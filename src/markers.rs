use std::cell::Cell;
use std::marker::PhantomData;

// Super nice trick taken from the rlua library.
// Can be used to pin a lifetime so that all functions which use
// that lifetime can only us that single lifetime and not one which
// is variant over that lifetime
pub type Invariant<'a> = PhantomData<Cell<&'a ()>>;
