pub use std::rc::Weak;
use std::{
    cell::{RefCell, RefMut},
    rc::Rc,
};

#[repr(transparent)]
pub struct Mut<T: ?Sized>(RefCell<T>);

pub type Lock<'a, T> = RefMut<'a, T>;

pub type Ref<T> = Rc<T>;

impl<T> Mut<T> {
    pub fn new(inner: T) -> Self {
        Mut(RefCell::new(inner))
    }

    pub fn lock(&self) -> Lock<T> {
        self.0.borrow_mut()
    }

    pub fn try_lock(&self) -> Option<Lock<T>> {
        self.0.try_borrow_mut().ok()
    }

    pub async fn async_lock(&self) -> Lock<T> {
        self.lock()
    }
}
