use std::{
    any::{Any, TypeId},
    cell::{Cell, UnsafeCell},
    collections::HashMap,
    hash::{BuildHasherDefault, Hasher},
    mem::ManuallyDrop,
    ops::Deref,
};

/// A trait for userdata which is stored in the runtime.
///
/// # Safety
/// For safe implementation of this trait none of its default implemented functions must be
/// overwritten and the type Static must be the correct type.
///
/// The static type must be the original type with the `'js` lifetime changed to `'static`.
///
/// All rquickjs javascript value have a `'js` lifetime, this lifetime is managed by rquickjs to
/// ensure that rquickjs values are used correctly. You can derive some lifetimes from this
/// lifetimes but only lifetimes on rquickjs structs can be soundly changed by this trait.
///
/// If changing a values type to its `UserData::Static` type would cause any borrow, non-rquickjs
/// struct with a '`js` struct to no have a different lifetime then the implementation is unsound.
///
/// ## Example
/// Below is a correctly implemented UserData, the `'js` on `Function` is directly derived from a
/// `Ctx<'js>`.
/// ```
/// # use rquickjs::{Function, runtime::UserData};
///
/// struct MyUserData<'js>{
///     function: Option<Function<'js>>
/// }
///
/// unsafe impl<'js> UserData<'js> for MyUserData<'js>{
///     // The self type with the lifetime changed to static.
///     type Static = MyUserData<'static>;
/// }
/// ```
///
/// The example below is __unsound__ as it changes the `&'js` borrow to static.
///
/// ```no_run
/// # use rquickjs::{Function, runtime::UserData};
///
/// struct MyUserData<'js>{
///     // This is unsound!
///     // The &'js lifetime here is not a lifetime on a rquickjs struct.
///     function: &'js Function<'js>
/// }
///
/// unsafe impl<'js> UserData<'js> for MyUserData<'js>{
///     // The self type with the lifetime changed to static.
///     type Static = MyUserData<'static>;
/// }
/// ```
///
///
pub unsafe trait UserData<'js> {
    type Static: 'static + Any;

    unsafe fn to_static(this: Self) -> Self::Static
    where
        Self: Sized,
    {
        assert_eq!(
            std::mem::size_of::<Self>(),
            std::mem::size_of::<Self::Static>(),
            "Invalid implementation of UserData, size_of::<Self>() != size_of::<Self::Static>()"
        );
        assert_eq!(
            std::mem::align_of::<Self>(),
            std::mem::align_of::<Self::Static>(),
            "Invalid implementation of UserData, align_of::<Self>() != align_of::<Self::Static>()"
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

    unsafe fn from_static_box(this: Box<Self::Static>) -> Box<Self>
    where
        Self: Sized,
    {
        assert_eq!(
            std::mem::size_of::<Self>(),
            std::mem::size_of::<Self::Static>(),
            "Invalid implementation of UserData, size_of::<Self>() != size_of::<Self::Static>()"
        );
        assert_eq!(
            std::mem::align_of::<Self>(),
            std::mem::align_of::<Self::Static>(),
            "Invalid implementation of UserData, align_of::<Self>() != align_of::<Self::Static>()"
        );

        Box::from_raw(Box::into_raw(this) as *mut Self)
    }

    unsafe fn from_static_ref<'a>(this: &'a Self::Static) -> &'a Self
    where
        Self: Sized,
    {
        std::mem::transmute(this)
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
    pub fn insert<'js, U>(&self, data: U) -> Result<Option<Box<U>>, U>
    where
        U: UserData<'js>,
    {
        if self.count.get() > 0 {
            return Err(data);
        }
        let user_static = unsafe { U::to_static(data) };
        let id = TypeId::of::<U::Static>();
        let r = unsafe { (*self.map.get()).insert(id, Box::new(user_static)) }.map(|x| {
            let r = x
                .downcast()
                .expect("type confusion! userdata not stored under the right type id");
            unsafe { U::from_static_box(r) }
        });
        Ok(r)
    }

    pub fn remove<'js, U>(&self) -> Result<Option<Box<U>>, ()>
    where
        U: UserData<'js>,
    {
        if self.count.get() > 0 {
            return Err(());
        }
        let id = TypeId::of::<U::Static>();
        let r = unsafe { (*self.map.get()).remove(&id) }.map(|x| {
            let r = x
                .downcast()
                .expect("type confusion! userdata not stored under the right type id");
            unsafe { U::from_static_box(r) }
        });
        Ok(r)
    }

    pub fn get<'js, U: UserData<'js>>(&self) -> Option<UserDataGuard<U>> {
        let id = TypeId::of::<U::Static>();
        self.count.set(self.count.get() + 1);
        unsafe { (*self.map.get()).get(&id) }.map(|x| {
            let u = x
                .downcast_ref()
                .expect("type confusion! userdata not stored under the right type id");

            let r = unsafe { U::from_static_ref(u) };
            UserDataGuard { map: self, r }
        })
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
