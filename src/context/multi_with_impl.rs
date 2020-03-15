use super::{Context, Ctx, MultiWith};
use std::mem;

impl<'js> MultiWith<'js> for (&'js Context, &'js Context) {
    type Arg = (Ctx<'js>, Ctx<'js>);

    fn with<R, F: FnOnce(Self::Arg) -> R>(self, f: F) -> R {
        if self.0.get_runtime_ptr() != self.1.get_runtime_ptr() {
            panic!("Tried to use contexts of different runtimes with eachother");
        }
        let guard = self.0.rt.lock();
        self.0.reset_stack();
        let ctx_0 = Ctx::new(self.0);
        let ctx_1 = Ctx::new(self.1);
        let res = f((ctx_0, ctx_1));
        mem::drop(guard);
        res
    }
}

impl<'js> MultiWith<'js> for (&'js Context, &'js Context, &'js Context) {
    type Arg = (Ctx<'js>, Ctx<'js>, Ctx<'js>);

    fn with<R, F: FnOnce(Self::Arg) -> R>(self, f: F) -> R {
        if self.0.get_runtime_ptr() != self.1.get_runtime_ptr() {
            panic!("Tried to use contexts of different runtimes with eachother");
        }
        if self.0.get_runtime_ptr() != self.2.get_runtime_ptr() {
            panic!("Tried to use contexts of different runtimes with eachother");
        }
        let guard = self.0.rt.lock();
        self.0.reset_stack();
        let ctx_0 = Ctx::new(self.0);
        let ctx_1 = Ctx::new(self.1);
        let ctx_2 = Ctx::new(self.2);
        let res = f((ctx_0, ctx_1, ctx_2));
        mem::drop(guard);
        res
    }
}

impl<'js> MultiWith<'js> for (&'js Context, &'js Context, &'js Context, &'js Context) {
    type Arg = (Ctx<'js>, Ctx<'js>, Ctx<'js>, Ctx<'js>);

    fn with<R, F: FnOnce(Self::Arg) -> R>(self, f: F) -> R {
        if self.0.get_runtime_ptr() != self.1.get_runtime_ptr() {
            panic!("Tried to use contexts of different runtimes with eachother");
        }
        if self.0.get_runtime_ptr() != self.2.get_runtime_ptr() {
            panic!("Tried to use contexts of different runtimes with eachother");
        }
        if self.0.get_runtime_ptr() != self.3.get_runtime_ptr() {
            panic!("Tried to use contexts of different runtimes with eachother");
        }
        let guard = self.0.rt.lock();
        self.0.reset_stack();
        let ctx_0 = Ctx::new(self.0);
        let ctx_1 = Ctx::new(self.1);
        let ctx_2 = Ctx::new(self.2);
        let ctx_3 = Ctx::new(self.3);
        let res = f((ctx_0, ctx_1, ctx_2, ctx_3));
        mem::drop(guard);
        res
    }
}
