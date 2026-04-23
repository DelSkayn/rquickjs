use rquickjs::{class::Trace, CatchResultExt, Class, Context, JsLifetime, Object, Runtime};

#[derive(Trace, JsLifetime)]
#[rquickjs::class]
pub struct Outer<'js> {
    #[qjs(get, set)]
    inner: Class<'js, Inner>,
}

#[derive(Trace, JsLifetime, Default, Clone)]
#[rquickjs::class]
pub struct Inner {
    #[qjs(get, set)]
    value: String,
}

pub fn main() {
    let rt = Runtime::new().unwrap();
    let ctx = Context::full(&rt).unwrap();

    ctx.with(|ctx| {
        let inner = Class::instance(
            ctx.clone(),
            Inner {
                value: "initial".into(),
            },
        )
        .unwrap();
        let outer = Class::instance(ctx.clone(), Outer { inner }).unwrap();
        ctx.globals().set("o", outer.clone()).unwrap();

        ctx.eval::<(), _>(
            r#"
            if (o.inner.value !== "initial") throw new Error("get");
            o.inner.value = "changed";
            if (o.inner.value !== "changed") throw new Error("nested set");
        "#,
        )
        .catch(&ctx)
        .unwrap();

        // Rust side observes the nested mutation.
        assert_eq!(outer.borrow().inner.borrow().value, "changed");

        // Mutation from Rust is visible from JS.
        outer.borrow().inner.borrow_mut().value = "from rust".into();
        let v: String = outer
            .as_value()
            .as_object()
            .unwrap()
            .get::<_, Object>("inner")
            .unwrap()
            .get("value")
            .unwrap();
        assert_eq!(v, "from rust");
    });
}
