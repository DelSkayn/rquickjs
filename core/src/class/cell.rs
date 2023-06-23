use std::{
    cell::{Cell, UnsafeCell},
    error::Error,
    fmt,
    marker::PhantomData,
    mem::ManuallyDrop,
    ops::{Deref, DerefMut},
};

use crate::{FromJs, IntoJs};

use super::{Class, JsClass};

#[derive(Debug)]
pub enum BorrowError {
    /// The class was not writable
    NotWritable,
    /// The class was already borrowed in a way that prevents borrowing again.
    AlreadyBorrowed,
}

impl fmt::Display for BorrowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            BorrowError::NotWritable => write!(f, "tried to borrow a class which is not writable"),
            BorrowError::AlreadyBorrowed => {
                write!(f, "can't borrow a class as it is already borrowed")
            }
        }
    }
}

impl Error for BorrowError {}

pub unsafe trait Mutability {
    #[doc(hidden)]
    type Cell<T>;

    fn new_cell<T>(t: T) -> Self::Cell<T>;

    #[doc(hidden)]
    unsafe fn borrow<'a, T>(cell: &'a Self::Cell<T>) -> Result<(), BorrowError>;

    #[doc(hidden)]
    unsafe fn unborrow<'a, T>(cell: &'a Self::Cell<T>);

    #[doc(hidden)]
    unsafe fn borrow_mut<'a, T>(cell: &'a Self::Cell<T>) -> Result<(), BorrowError>;

    #[doc(hidden)]
    unsafe fn unborrow_mut<'a, T>(cell: &'a Self::Cell<T>);

    unsafe fn deref<'a, T>(cell: &'a Self::Cell<T>) -> &'a T;

    unsafe fn deref_mut<'a, T>(cell: &'a Self::Cell<T>) -> &'a mut T;
}

/// A marker type used for marking the mutability of a class.
/// When a class has `Readable` as it Mutable type you can only borrow it immutable.
pub enum Readable {}

unsafe impl Mutability for Readable {
    type Cell<T> = T;

    fn new_cell<T>(t: T) -> Self::Cell<T> {
        t
    }

    unsafe fn borrow<'a, T>(_cell: &'a Self::Cell<T>) -> Result<(), BorrowError> {
        Ok(())
    }

    unsafe fn unborrow<'a, T>(_cell: &'a Self::Cell<T>) {}

    unsafe fn borrow_mut<'a, T>(_cell: &'a Self::Cell<T>) -> Result<(), BorrowError> {
        Err(BorrowError::NotWritable)
    }

    unsafe fn unborrow_mut<'a, T>(_cell: &'a Self::Cell<T>) {}

    unsafe fn deref<'a, T>(cell: &'a Self::Cell<T>) -> &'a T {
        cell
    }

    unsafe fn deref_mut<'a, T>(_cell: &'a Self::Cell<T>) -> &'a mut T {
        unreachable!()
    }
}

/// A marker type used for marking the mutability of a class.
/// When a class has `Writable` as it Mutable type you can borrow it both mutability and immutable.
pub enum Writable {}

pub struct WritableCell<T> {
    count: Cell<usize>,
    value: UnsafeCell<T>,
}

#[doc(hidden)]
pub struct WriteBorrow<'a, T> {
    cell: &'a WritableCell<T>,
    _marker: PhantomData<&'a T>,
}

impl<'a, T> Deref for WriteBorrow<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &(*self.cell.value.get()) }
    }
}

impl<'a, T> Drop for WriteBorrow<'a, T> {
    fn drop(&mut self) {
        self.cell.count.set(self.cell.count.get() - 1);
    }
}

#[doc(hidden)]
pub struct WriteBorrowMut<'a, T> {
    cell: &'a WritableCell<T>,
    _marker: PhantomData<&'a T>,
}

impl<'a, T> Deref for WriteBorrowMut<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &(*self.cell.value.get()) }
    }
}

impl<'a, T> DerefMut for WriteBorrowMut<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut (*self.cell.value.get()) }
    }
}

impl<'a, T> Drop for WriteBorrowMut<'a, T> {
    fn drop(&mut self) {
        self.cell.count.set(0);
    }
}

unsafe impl Mutability for Writable {
    type Cell<T> = WritableCell<T>;

    fn new_cell<T>(t: T) -> Self::Cell<T> {
        WritableCell {
            count: Cell::new(0),
            value: UnsafeCell::new(t),
        }
    }

    unsafe fn borrow<'a, T>(cell: &'a Self::Cell<T>) -> Result<(), BorrowError> {
        let count = cell.count.get();
        if count == usize::MAX {
            return Err(BorrowError::AlreadyBorrowed);
        }
        cell.count.set(count + 1);
        Ok(())
    }

    unsafe fn unborrow<'a, T>(cell: &'a Self::Cell<T>) {
        cell.count.set(cell.count.get() - 1);
    }

    unsafe fn borrow_mut<'a, T>(cell: &'a Self::Cell<T>) -> Result<(), BorrowError> {
        let count = cell.count.get();
        if count != 0 {
            return Err(BorrowError::AlreadyBorrowed);
        }
        cell.count.set(usize::MAX);
        Ok(())
    }

    unsafe fn unborrow_mut<'a, T>(cell: &'a Self::Cell<T>) {
        cell.count.set(0);
    }

    unsafe fn deref<'a, T>(cell: &'a Self::Cell<T>) -> &'a T {
        &*cell.value.get()
    }

    unsafe fn deref_mut<'a, T>(cell: &'a Self::Cell<T>) -> &'a mut T {
        &mut *cell.value.get()
    }
}

/// A cell type for rust classes passed to javascript.
///
/// Implements a RefCell like borrow checking.
pub struct JsCell<T: JsClass> {
    cell: <T::Mutable as Mutability>::Cell<T>,
}

impl<T: JsClass> JsCell<T> {
    /// Create a new JsCell
    pub fn new(t: T) -> Self {
        JsCell {
            cell: <T::Mutable as Mutability>::new_cell(t),
        }
    }

    /// Borrow the contained value immutable.
    ///
    /// # Panic
    /// Panics if the value is already borrowed mutably
    pub fn borrow(&self) -> Borrow<T> {
        unsafe {
            <T::Mutable as Mutability>::borrow(&self.cell).unwrap();
            Borrow(&self.cell)
        }
    }

    /// Try to borrow the contained value immutable.
    pub fn try_borrow(&self) -> Result<Borrow<T>, BorrowError> {
        unsafe {
            <T::Mutable as Mutability>::borrow(&self.cell)?;
            Ok(Borrow(&self.cell))
        }
    }

    /// Borrow the contained value mutably.
    ///
    /// # Panic
    /// Panics if the value is already borrowed mutably or the class can't be borrowed mutably.
    pub fn borrow_mut(&self) -> BorrowMut<T> {
        unsafe {
            <T::Mutable as Mutability>::borrow_mut(&self.cell).unwrap();
            BorrowMut(&self.cell)
        }
    }

    /// Try to borrow the contained value mutably.
    pub fn try_borrow_mut(&self) -> Result<BorrowMut<T>, BorrowError> {
        unsafe {
            <T::Mutable as Mutability>::borrow_mut(&self.cell)?;
            Ok(BorrowMut(&self.cell))
        }
    }
}

/// A borrow guard for a borrowed class.
pub struct Borrow<'a, T: JsClass + 'a>(&'a <T::Mutable as Mutability>::Cell<T>);

impl<'a, T: JsClass + 'a> Drop for Borrow<'a, T> {
    fn drop(&mut self) {
        unsafe { <T::Mutable as Mutability>::unborrow(self.0) }
    }
}

impl<'a, T: JsClass> Deref for Borrow<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { <T::Mutable as Mutability>::deref(self.0) }
    }
}

/// A borrow guard for a mutably borrowed class.
pub struct BorrowMut<'a, T: JsClass + 'a>(&'a <T::Mutable as Mutability>::Cell<T>);

impl<'a, T: JsClass + 'a> Drop for BorrowMut<'a, T> {
    fn drop(&mut self) {
        unsafe { <T::Mutable as Mutability>::unborrow_mut(self.0) }
    }
}

impl<'a, T: JsClass> Deref for BorrowMut<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { <T::Mutable as Mutability>::deref(self.0) }
    }
}

impl<'a, T: JsClass> DerefMut for BorrowMut<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { <T::Mutable as Mutability>::deref_mut(self.0) }
    }
}

/// An owned borrow of a class object which keeps the borrow alive for the duration of the objects lifetime.
pub struct OwnedBorrow<'js, T: JsClass + 'js>(ManuallyDrop<Class<'js, T>>);

impl<'js, T: JsClass + 'js> OwnedBorrow<'js, T> {
    /// Borrow a class owned.
    ///
    /// # Panic
    /// Panics if the class cannot be borrowed
    pub fn from_class(class: Class<'js, T>) -> Self {
        Self::try_from_class(class).unwrap()
    }

    /// Try to borrow a class owned.
    pub fn try_from_class(class: Class<'js, T>) -> Result<Self, BorrowError> {
        unsafe {
            <T::Mutable as Mutability>::borrow(&class.as_class().cell)?;
        }
        Ok(OwnedBorrow(ManuallyDrop::new(class)))
    }

    /// Turn the owned borrow back into the class releasing the borrow.
    pub fn into_inner(mut self) -> Class<'js, T> {
        unsafe { <T::Mutable as Mutability>::unborrow(&self.0.as_class().cell) };
        let res = unsafe { ManuallyDrop::take(&mut self.0) };
        std::mem::forget(self);
        res
    }
}

impl<'js, T: JsClass + 'js> Drop for OwnedBorrow<'js, T> {
    fn drop(&mut self) {
        unsafe {
            <T::Mutable as Mutability>::unborrow(&self.0.as_class().cell);
            ManuallyDrop::drop(&mut self.0)
        }
    }
}

impl<'js, T: JsClass + 'js> Deref for OwnedBorrow<'js, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        unsafe { <T::Mutable as Mutability>::deref(&self.0.as_class().cell) }
    }
}

impl<'js, T: JsClass> FromJs<'js> for OwnedBorrow<'js, T> {
    fn from_js(ctx: crate::Ctx<'js>, value: crate::Value<'js>) -> crate::Result<Self> {
        let cls = Class::from_js(ctx, value)?;
        Ok(OwnedBorrow::try_from_class(cls)?)
    }
}

impl<'js, T: JsClass> IntoJs<'js> for OwnedBorrow<'js, T> {
    fn into_js(self, ctx: crate::Ctx<'js>) -> crate::Result<crate::Value<'js>> {
        self.into_inner().into_js(ctx)
    }
}

/// An owned mutable borrow of a class object which keeps the borrow alive for the duration of the objects lifetime.
pub struct OwnedBorrowMut<'js, T: JsClass + 'js>(ManuallyDrop<Class<'js, T>>);

impl<'js, T: JsClass + 'js> OwnedBorrowMut<'js, T> {
    /// Borrow a class mutably owned.
    ///
    /// # Panic
    /// Panics if the class cannot be borrowed
    pub fn from_class(class: Class<'js, T>) -> Self {
        Self::try_from_class(class).unwrap()
    }

    /// Try to borrow a class mutably owned.
    pub fn try_from_class(class: Class<'js, T>) -> Result<Self, BorrowError> {
        unsafe {
            <T::Mutable as Mutability>::borrow_mut(&class.as_class().cell)?;
        }
        Ok(OwnedBorrowMut(ManuallyDrop::new(class)))
    }

    /// Turn the owned borrow back into the class releasing the borrow.
    pub fn into_inner(mut self) -> Class<'js, T> {
        unsafe { <T::Mutable as Mutability>::unborrow_mut(&self.0.as_class().cell) };
        let res = unsafe { ManuallyDrop::take(&mut self.0) };
        std::mem::forget(self);
        res
    }
}

impl<'js, T: JsClass + 'js> Drop for OwnedBorrowMut<'js, T> {
    fn drop(&mut self) {
        unsafe {
            <T::Mutable as Mutability>::unborrow_mut(&self.0.as_class().cell);
            ManuallyDrop::drop(&mut self.0)
        }
    }
}

impl<'js, T: JsClass + 'js> Deref for OwnedBorrowMut<'js, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        unsafe { <T::Mutable as Mutability>::deref(&self.0.as_class().cell) }
    }
}

impl<'js, T: JsClass + 'js> DerefMut for OwnedBorrowMut<'js, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { <T::Mutable as Mutability>::deref_mut(&self.0.as_class().cell) }
    }
}

impl<'js, T: JsClass> FromJs<'js> for OwnedBorrowMut<'js, T> {
    fn from_js(ctx: crate::Ctx<'js>, value: crate::Value<'js>) -> crate::Result<Self> {
        let cls = Class::from_js(ctx, value)?;
        Ok(OwnedBorrowMut::try_from_class(cls)?)
    }
}

impl<'js, T: JsClass> IntoJs<'js> for OwnedBorrowMut<'js, T> {
    fn into_js(self, ctx: crate::Ctx<'js>) -> crate::Result<crate::Value<'js>> {
        self.into_inner().into_js(ctx)
    }
}
