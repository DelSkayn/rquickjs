use std::cell::Cell;

use super::JsClass;

pub enum BorrowError {
    NotWritable,
    AlreadyWriting,
    AlreadyReading,
}

pub unsafe trait Mutability {
    #[doc(hidden)]
    type Cell<T>;
}

/// A marker type used for t
pub enum Readable {}

unsafe impl Mutability for Readable {
    #[doc(hidden)]
    type Cell<T> = T;
}

pub enum Writable {}

pub struct WritableCell<T> {
    count: Cell<usize>,
    value: T,
}

unsafe impl Mutability for Writable {
    #[doc(hidden)]
    type Cell<T> = WritableCell<T>;
}

pub struct JsCell<T: JsClass> {
    cell: <T::Mutable as Mutability>::Cell<T>,
}
