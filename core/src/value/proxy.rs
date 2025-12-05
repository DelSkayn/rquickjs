//! Module for types dealing with JS proxies.

use crate::{qjs, value::Constructor, Ctx, IntoJs, Object, Result};

mod handler;
pub use handler::{ProxyHandler, ProxyProperty, ProxyReceiver, ProxyTarget};

/// Rust representation of a JavaScript proxy.
#[derive(Debug, PartialEq, Clone, Hash, Eq)]
#[repr(transparent)]
pub struct Proxy<'js>(pub(crate) Object<'js>);

impl<'js> Proxy<'js> {
    /// Create a new JavaScript proxy
    pub fn new(
        ctx: Ctx<'js>,
        target: impl IntoJs<'js>,
        handler: ProxyHandler<'js>,
    ) -> Result<Self> {
        // Until we have https://github.com/quickjs-ng/quickjs/issues/1261
        let constructor: Constructor = ctx.globals().get("Proxy")?;
        let proxy = constructor.construct((target, handler))?;
        Ok(Self(proxy))
    }

    /// Get the target of the proxy
    pub fn target(&self) -> Result<Object<'js>> {
        unsafe {
            let target = qjs::JS_GetProxyTarget(self.0.ctx.as_ptr(), self.0.as_js_value());
            let target = self.0.ctx.handle_exception(target)?;
            Ok(Object::from_js_value(self.0.ctx.clone(), target))
        }
    }

    /// Get the handler of the proxy
    pub fn handler(&self) -> Result<Object<'js>> {
        unsafe {
            let handler = qjs::JS_GetProxyHandler(self.0.ctx.as_ptr(), self.0.as_js_value());
            let handler = self.0.ctx.handle_exception(handler)?;
            Ok(Object::from_js_value(self.0.ctx.clone(), handler))
        }
    }
}

#[cfg(test)]
mod test {
    use crate::{
        class::{JsClass, Readable, Trace, Tracer},
        test_with, Class, Error, Function, JsLifetime, Value,
    };

    use super::*;

    #[test]
    fn from_javascript() {
        test_with(|ctx| {
            let proxy: Proxy = ctx
                .eval(r#"new Proxy({ a: 1 }, { get: () => 2 })"#)
                .unwrap();
            let target = proxy.target().unwrap();
            let handler = proxy.handler().unwrap();
            let a: i32 = target.get("a").unwrap();
            assert_eq!(a, 1);
            let _: Function = handler.get("get").unwrap();
        });
    }

    #[test]
    fn from_rust() {
        test_with(|ctx| {
            let handler = ProxyHandler::new(ctx.clone())
                .unwrap()
                .with_getter(|target, property, _receiver| {
                    if property.to_string().unwrap() == "a" {
                        let value: Value<'_> = target.0.get("a")?;
                        Ok(value)
                    } else {
                        Err(Error::Unknown)
                    }
                })
                .unwrap();
            let target = Object::new(ctx.clone()).unwrap();
            target.set("a", 1).unwrap();
            let proxy = Proxy::new(ctx.clone(), target, handler).unwrap();
            ctx.globals().set("proxy", proxy).unwrap();
            let a: i32 = ctx.eval("proxy.a").unwrap();
            assert_eq!(a, 1);
        });
    }

    #[test]
    fn class_proxy() {
        pub struct MyClass {
            a: i32,
        }

        impl MyClass {
            pub fn new(a: i32) -> Self {
                Self { a }
            }
        }

        impl<'js> Trace<'js> for MyClass {
            fn trace<'a>(&self, _tracer: Tracer<'a, 'js>) {}
        }

        unsafe impl<'js> JsLifetime<'js> for MyClass {
            type Changed<'to> = MyClass;
        }

        impl<'js> JsClass<'js> for MyClass {
            const NAME: &'static str = "MyClass";

            type Mutable = Readable;

            fn constructor(_ctx: &Ctx<'js>) -> Result<Option<Constructor<'js>>> {
                Ok(None)
            }
        }

        test_with(|ctx| {
            let handler = ProxyHandler::new(ctx.clone())
                .unwrap()
                .with_getter(|target, property, _receiver| {
                    if property.to_string().unwrap() == "a" {
                        let target = target.0.into_class::<MyClass>().unwrap();
                        let value = target.borrow().a;
                        Ok(value)
                    } else {
                        Err(Error::Unknown)
                    }
                })
                .unwrap();
            let target = Class::instance(ctx.clone(), MyClass::new(1)).unwrap();
            let proxy = Proxy::new(ctx.clone(), target, handler).unwrap();
            ctx.globals().set("proxy", proxy).unwrap();
            let a: i32 = ctx.eval("proxy.a").unwrap();
            assert_eq!(a, 1);
        });
    }
}
