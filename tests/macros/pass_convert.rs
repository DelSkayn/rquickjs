use rquickjs::{class::Trace, CatchResultExt, Context, FromJs, IntoJs, JsLifetime, Runtime};

#[derive(Debug, PartialEq, Eq, FromJs, IntoJs)]
#[qjs(rename_all = "camelCase")]
struct RenamedFields {
    some_value: u32,
    #[qjs(rename = "labelText")]
    label_text: String,
}

#[derive(Debug, PartialEq, Eq, FromJs, IntoJs)]
struct Pair(u32, String);

#[derive(Clone, Debug, PartialEq, Eq, Trace, JsLifetime)]
#[rquickjs::class]
struct JsClassShape {
    value: u32,
}

pub fn main() {
    let rt = Runtime::new().unwrap();
    let ctx = Context::full(&rt).unwrap();

    ctx.with(|ctx| {
        ctx.globals()
            .set(
                "renamed",
                RenamedFields {
                    some_value: 1,
                    label_text: "alpha".into(),
                },
            )
            .unwrap();

        ctx.eval::<(), _>(
            r#"
            if (renamed.someValue !== 1) {
                throw new Error("someValue");
            }
            if (renamed.labelText !== "alpha") {
                throw new Error("labelText");
            }
        "#,
        )
        .catch(&ctx)
        .unwrap();

        let renamed: RenamedFields = ctx
            .eval(r#"({ someValue: 2, labelText: "beta" })"#)
            .unwrap();
        assert_eq!(
            renamed,
            RenamedFields {
                some_value: 2,
                label_text: "beta".into(),
            }
        );

        ctx.globals().set("pair", Pair(3, "gamma".into())).unwrap();

        ctx.eval::<(), _>(
            r#"
            if (pair[0] !== 3) {
                throw new Error("pair0");
            }
            if (pair[1] !== "gamma") {
                throw new Error("pair1");
            }
        "#,
        )
        .catch(&ctx)
        .unwrap();

        let pair: Pair = ctx.eval(r#"[4, "delta"]"#).unwrap();
        assert_eq!(pair, Pair(4, "delta".into()));

        ctx.globals()
            .set("classValue", JsClassShape { value: 7 })
            .unwrap();

        let class_value: JsClassShape = ctx.eval("classValue").unwrap();
        assert_eq!(class_value.value, 7);

        let class_from_literal = ctx.eval::<JsClassShape, _>(r#"({ value: 8 })"#);
        assert!(class_from_literal.is_err());
    });
}
