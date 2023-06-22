use std::{
    cell::{Cell, UnsafeCell},
    marker::PhantomData,
    ops::{Deref, DerefMut},
};

use super::JsClass;

#[derive(Debug)]
pub enum BorrowError {
    NotWritable,
    AlreadyBorrowed,
}

pub unsafe trait Mutability {
    #[doc(hidden)]
    type Cell<T>;

    #[doc(hidden)]
    type BorrowMut<'a, T>: Deref<Target = T> + DerefMut
    where
        T: 'a;

    #[doc(hidden)]
    type Borrow<'a, T>: Deref<Target = T>
    where
        T: 'a;

    fn new_cell<T>(t: T) -> Self::Cell<T>;

    #[doc(hidden)]
    fn borrow<'a, T>(cell: &'a Self::Cell<T>) -> Self::Borrow<'a, T>;

    #[doc(hidden)]
    fn borrow_mut<'a, T>(cell: &'a Self::Cell<T>) -> Self::BorrowMut<'a, T>;

    #[doc(hidden)]
    fn try_borrow<'a, T>(cell: &'a Self::Cell<T>) -> Result<Self::Borrow<'a, T>, BorrowError>;

    #[doc(hidden)]
    fn try_borrow_mut<'a, T>(
        cell: &'a Self::Cell<T>,
    ) -> Result<Self::BorrowMut<'a, T>, BorrowError>;
}

/// A marker type used for t
pub enum Readable {}

unsafe impl Mutability for Readable {
    type Cell<T> = T;

    type BorrowMut<'a, T> = &'a mut T
        where T: 'a;

    type Borrow<'a, T> = &'a T
    where T: 'a;

    fn new_cell<T>(t: T) -> Self::Cell<T> {
        t
    }

    fn borrow<'a, T>(cell: &'a Self::Cell<T>) -> Self::Borrow<'a, T> {
        cell
    }

    fn borrow_mut<'a, T>(_cell: &'a Self::Cell<T>) -> Self::BorrowMut<'a, T> {
        panic!("tried to borrow a js class mutably which is not writable")
    }

    fn try_borrow<'a, T>(cell: &'a Self::Cell<T>) -> Result<Self::Borrow<'a, T>, BorrowError> {
        Ok(cell)
    }

    fn try_borrow_mut<'a, T>(
        _cell: &'a Self::Cell<T>,
    ) -> Result<Self::BorrowMut<'a, T>, BorrowError> {
        Err(BorrowError::NotWritable)
    }
}

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

    type BorrowMut<'a, T> = WriteBorrowMut<'a,T>
    where
        T: 'a ;

    type Borrow<'a, T> = WriteBorrow<'a,T>
    where
        T: 'a;

    fn new_cell<T>(t: T) -> Self::Cell<T> {
        WritableCell {
            count: Cell::new(0),
            value: UnsafeCell::new(t),
        }
    }

    fn borrow<'a, T>(cell: &'a Self::Cell<T>) -> Self::Borrow<'a, T> {
        Self::try_borrow(cell).unwrap()
    }

    fn borrow_mut<'a, T>(cell: &'a Self::Cell<T>) -> Self::BorrowMut<'a, T> {
        Self::try_borrow_mut(cell).unwrap()
    }

    fn try_borrow<'a, T>(cell: &'a Self::Cell<T>) -> Result<Self::Borrow<'a, T>, BorrowError> {
        let count = cell.count.get();
        if count != usize::MAX {
            cell.count.set(count + 1);
            Ok(WriteBorrow {
                cell,
                _marker: PhantomData,
            })
        } else {
            Err(BorrowError::AlreadyBorrowed)
        }
    }

    fn try_borrow_mut<'a, T>(
        cell: &'a Self::Cell<T>,
    ) -> Result<Self::BorrowMut<'a, T>, BorrowError> {
        let count = cell.count.get();
        if count != 0 {
            cell.count.set(usize::MAX);
            Ok(WriteBorrowMut {
                cell,
                _marker: PhantomData,
            })
        } else {
            Err(BorrowError::AlreadyBorrowed)
        }
    }
}

pub struct JsCell<T: JsClass> {
    cell: <T::Mutable as Mutability>::Cell<T>,
}

impl<T: JsClass> JsCell<T> {
    pub fn new(t: T) -> Self {
        JsCell {
            cell: <T::Mutable as Mutability>::new_cell(t),
        }
    }

    pub fn borrow(&self) -> Borrow<T> {
        Borrow(<T::Mutable as Mutability>::borrow(&self.cell))
    }

    pub fn try_borrow(&self) -> Result<Borrow<T>, BorrowError> {
        <T::Mutable as Mutability>::try_borrow(&self.cell).map(Borrow)
    }

    pub fn borrow_mut(&self) -> BorrowMut<T> {
        BorrowMut(<T::Mutable as Mutability>::borrow_mut(&self.cell))
    }

    pub fn try_borrow_mut(&self) -> Result<BorrowMut<T>, BorrowError> {
        <T::Mutable as Mutability>::try_borrow_mut(&self.cell).map(BorrowMut)
    }
}

pub struct Borrow<'a, T: JsClass + 'a>(<T::Mutable as Mutability>::Borrow<'a, T>);

pub struct BorrowMut<'a, T: JsClass + 'a>(<T::Mutable as Mutability>::BorrowMut<'a, T>);
