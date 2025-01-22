use core::fmt;
use std::{
    any::{Any, TypeId},
    cell::{Cell, UnsafeCell},
    collections::HashMap,
    hash::{BuildHasherDefault, Hasher},
    mem::ManuallyDrop,
    ops::Deref,
};

use crate::JsLifetime;

unsafe fn to_static<'js, T>(this: T) -> T::Changed<'static>
where
    T: JsLifetime<'js> + Sized,
{
    assert_eq!(
        std::mem::size_of::<T>(),
        std::mem::size_of::<T::Changed<'static>>(),
        "Invalid implementation of JsLifetime, size_of::<T>() != size_of::<T::Changed<'static>>()"
    );
    assert_eq!(
        std::mem::align_of::<T>(),
        std::mem::align_of::<T::Changed<'static>>(),
        "Invalid implementation of JsLifetime, align_of::<T>() != align_of::<T::Changed<'static>>()"
    );

    // a super unsafe way to cast between types, This is necessary because normal transmute will
    // complain that Self and Self::Static are not related so might not have the same size.
    union Trans<A, B> {
        from: ManuallyDrop<A>,
        to: ManuallyDrop<B>,
    }

    ManuallyDrop::into_inner(
        (Trans {
            from: ManuallyDrop::new(this),
        })
        .to,
    )
}

unsafe fn from_static_box<'js, T>(this: Box<T::Changed<'static>>) -> Box<T>
where
    T: JsLifetime<'js> + Sized,
{
    assert_eq!(
        std::mem::size_of::<T>(),
        std::mem::size_of::<T::Changed<'static>>(),
        "Invalid implementation of JsLifetime, size_of::<T>() != size_of::<T::Changed<'static>>()"
    );
    assert_eq!(
        std::mem::align_of::<T>(),
        std::mem::align_of::<T::Changed<'static>>(),
        "Invalid implementation of JsLifetime, align_of::<T>() != align_of::<T::Changed<'static>>()"
    );

    Box::from_raw(Box::into_raw(this) as *mut T)
}

unsafe fn from_static_ref<'a, 'js, T>(this: &'a T::Changed<'static>) -> &'a T
where
    T: JsLifetime<'js> + Sized,
{
    std::mem::transmute(this)
}

pub struct UserDataError<T>(pub T);

impl<T> fmt::Display for UserDataError<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "tried to mutate the user data store while it was being referenced"
        )
    }
}

impl<T> fmt::Debug for UserDataError<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

#[derive(Default)]
struct IdHasher(u64);

impl Hasher for IdHasher {
    fn write(&mut self, _: &[u8]) {
        unreachable!("TypeId calls write_u64");
    }

    fn write_u64(&mut self, id: u64) {
        self.0 = id;
    }

    fn finish(&self) -> u64 {
        self.0
    }
}

/// Typeid hashmap taken from axum.
#[derive(Default)]
pub(crate) struct UserDataMap {
    map: UnsafeCell<HashMap<TypeId, Box<dyn Any>, BuildHasherDefault<IdHasher>>>,
    count: Cell<usize>,
}

impl UserDataMap {
    pub fn insert<'js, U>(&self, data: U) -> Result<Option<Box<U>>, UserDataError<U>>
    where
        U: JsLifetime<'js>,
        U::Changed<'static>: Any,
    {
        if self.count.get() > 0 {
            return Err(UserDataError(data));
        }
        let user_static = unsafe { to_static(data) };
        let id = TypeId::of::<U::Changed<'static>>();
        let r = unsafe { (*self.map.get()).insert(id, Box::new(user_static)) }.map(|x| {
            let r = x
                .downcast()
                .expect("type confusion! userdata not stored under the right type id");
            unsafe { from_static_box(r) }
        });
        Ok(r)
    }

    pub fn remove<'js, U>(&self) -> Result<Option<Box<U>>, UserDataError<()>>
    where
        U: JsLifetime<'js>,
        U::Changed<'static>: Any,
    {
        if self.count.get() > 0 {
            return Err(UserDataError(()));
        }
        let id = TypeId::of::<U::Changed<'static>>();
        let r = unsafe { (*self.map.get()).remove(&id) }.map(|x| {
            let r = x
                .downcast()
                .expect("type confusion! userdata not stored under the right type id");
            unsafe { from_static_box(r) }
        });
        Ok(r)
    }

    pub fn get<'js, U>(&self) -> Option<UserDataGuard<U>>
    where
        U: JsLifetime<'js>,
        U::Changed<'static>: Any,
    {
        let id = TypeId::of::<U::Changed<'static>>();
        unsafe { (*self.map.get()).get(&id) }.map(|x| {
            self.count.set(self.count.get() + 1);
            let u = x
                .downcast_ref()
                .expect("type confusion! userdata not stored under the right type id");

            let r = unsafe { from_static_ref(u) };
            UserDataGuard { map: self, r }
        })
    }

    pub fn clear(&mut self) {
        self.map.get_mut().clear()
    }
}

/// Guard for user data to avoid inserting new userdata while exisiting userdata is being
/// referenced.
pub struct UserDataGuard<'a, U> {
    map: &'a UserDataMap,
    r: &'a U,
}

impl<'a, U> Deref for UserDataGuard<'a, U> {
    type Target = U;

    fn deref(&self) -> &Self::Target {
        self.r
    }
}

impl<'a, U> Drop for UserDataGuard<'a, U> {
    fn drop(&mut self) {
        self.map.count.set(self.map.count.get() - 1)
    }
}
