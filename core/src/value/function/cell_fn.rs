use core::fmt;
use std::cell::{Cell, RefCell};

#[derive(Debug)]
pub enum CellFnError {
    /// The cell fn is already being borrowed, can only happen when a mutable closure is called recursively.
    AlreadyBorrowed,
    /// The cell fn was alread used, only returned after an owned closure was already called once.
    AlreadyUsed,
}

impl fmt::Display for CellFnError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            CellFnError::AlreadyBorrowed => {
                write!(
                    f,
                    "Function that can only be called mutably was already borrowed"
                )
            }
            CellFnError::AlreadyUsed => {
                write!(
                    f,
                    "Function that could only be called once was already called"
                )
            }
        }
    }
}

/// A trait for making closures with different borrowing rules callable throught the same
/// interface..
pub trait CellFn<'js, P, R> {
    fn call(&self, params: P) -> Result<R, CellFnError>;
}

/// Helper type for creating a function from a closure which implements FnMut
///
/// When called through [`CellFn`] will try to borrow the internal [`RefCell`], if this is not
/// possible it will return an error.
pub struct Mut<T>(pub RefCell<T>);

/// Helper type for creating a function from a closure which implements FnMut
///
/// When called through [`CellFn`] will take the internal value leaving it empty. If the internal
/// value was already empty it will return a error.
pub struct Once<T>(pub Cell<Option<T>>);

macro_rules! impl_cell_fn{
    ($($t:ident),*) => {

        #[allow(non_snake_case)]
        impl<'js,Func,R$(,$t)*> CellFn<'js, ($($t,)*), R> for Func
        where
            Func: Fn($($t,)*) -> R,
        {
            fn call(&self, ($($t,)*): ($($t,)*)) -> Result<R, CellFnError>{
                Ok((self)($($t,)*))
            }
        }

        #[allow(non_snake_case)]
        impl<'js,Func,R$(,$t)*> CellFn<'js, ($($t,)*), R> for Mut<Func>
        where
            Func: FnMut($($t,)*) -> R,
        {
            fn call(&self, ($($t,)*): ($($t,)*)) -> Result<R, CellFnError>{
                let mut lock = self.0.try_borrow_mut().map_err(|_| CellFnError::AlreadyBorrowed)?;
                Ok((lock)($($t,)*))
            }
        }

        #[allow(non_snake_case)]
        impl<'js,Func,R$(,$t)*> CellFn<'js, ($($t,)*), R> for Once<Func>
        where
            Func: FnOnce($($t,)*) -> R,
        {
            fn call(&self, ($($t,)*): ($($t,)*)) -> Result<R, CellFnError>{
                let f = self.0.take().ok_or(CellFnError::AlreadyUsed)?;
                Ok((f)($($t,)*))
            }
        }
    };
}

impl_cell_fn!();
impl_cell_fn!(A);
impl_cell_fn!(A, B);
impl_cell_fn!(A, B, C);
impl_cell_fn!(A, B, C, D);
impl_cell_fn!(A, B, C, D, E);
impl_cell_fn!(A, B, C, D, E, F);
impl_cell_fn!(A, B, C, D, E, F, G);
