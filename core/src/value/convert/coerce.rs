use crate::{convert::Coerced, qjs, Ctx, FromJs, Result, StdString, String, Value};
use core::{
    mem::MaybeUninit,
    ops::{Deref, DerefMut},
};

impl<T> AsRef<T> for Coerced<T> {
    fn as_ref(&self) -> &T {
        &self.0
    }
}

impl<T> AsMut<T> for Coerced<T> {
    fn as_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

impl<T> Deref for Coerced<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> DerefMut for Coerced<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// Coerce a value to a string in the same way JavaScript would coerce values.
impl<'js> FromJs<'js> for Coerced<String<'js>> {
    fn from_js(ctx: &Ctx<'js>, value: Value<'js>) -> Result<Self> {
        Ok(Coerced(unsafe {
            let result = qjs::JS_ToString(ctx.as_ptr(), value.as_js_value());
            ctx.handle_exception(result)?;
            // result should be a string now
            // String itself will check for the tag when debug_assertions are enabled
            // but is should always be string
            String::from_js_value(ctx.clone(), result)
        }))
    }
}

/// Coerce a value to a string in the same way JavaScript would coerce values.
impl<'js> FromJs<'js> for Coerced<StdString> {
    fn from_js(ctx: &Ctx<'js>, value: Value<'js>) -> Result<Self> {
        <Coerced<String>>::from_js(ctx, value)
            .and_then(|string| string.to_string())
            .map(Coerced)
    }
}

macro_rules! coerce_impls {
	  ($($(#[$meta:meta])* $type:ident $func:ident,)*) => {
		    $(
            $(#[$meta])*
            impl<'js> FromJs<'js> for Coerced<$type> {
                fn from_js(ctx: &Ctx<'js>, value: Value<'js>) -> Result<Self> {
                    let mut result = MaybeUninit::uninit();
                    Ok(Coerced(unsafe {
                        if 0 > qjs::$func(ctx.as_ptr(), result.as_mut_ptr(), value.as_js_value()) {
                            return Err(ctx.raise_exception());
                        }
                        result.assume_init()
                    }))
                }
            }
        )*
	  };
}

coerce_impls! {
    /// Coerce a value to a `i32` in the same way JavaScript would coerce values
    i32 JS_ToInt32,
    /// Coerce a value to a `i64` in the same way JavaScript would coerce values
    i64 JS_ToInt64Ext,
    /// Coerce a value to a `u64` in the same way JavaScript would coerce values
    u64 JS_ToIndex,
    /// Coerce a value to a `f64` in the same way JavaScript would coerce values
    f64 JS_ToFloat64,
}

/// Coerce a value to a `bool` in the same way JavaScript would coerce values
impl<'js> FromJs<'js> for Coerced<bool> {
    fn from_js(ctx: &Ctx<'js>, value: Value<'js>) -> Result<Self> {
        Ok(Coerced(unsafe {
            let res = qjs::JS_ToBool(ctx.as_ptr(), value.as_js_value());
            if 0 > res {
                return Err(ctx.raise_exception());
            }
            res == 1
        }))
    }
}

#[cfg(feature = "serde")]
#[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "serde")))]
impl<T: serde::Serialize> serde::Serialize for Coerced<T> {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> core::result::Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

#[cfg(feature = "serde")]
#[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "serde")))]
impl<'de, T: serde::Deserialize<'de>> serde::Deserialize<'de> for Coerced<T> {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> core::result::Result<Self, D::Error> {
        T::deserialize(deserializer).map(Coerced)
    }
}

#[cfg(all(test, feature = "serde"))]
mod tests {
    use super::Coerced;
    use serde_json::{json, Value};

    #[test]
    fn serialize_is_transparent() {
        assert_eq!(serde_json::to_value(Coerced(42_i32)).unwrap(), json!(42));
        assert_eq!(serde_json::to_value(Coerced(-1.5_f64)).unwrap(), json!(-1.5));
        assert_eq!(serde_json::to_value(Coerced("abc")).unwrap(), json!("abc"));
        assert_eq!(serde_json::to_value(Coerced(true)).unwrap(), json!(true));
        assert_eq!(serde_json::to_value(Coerced(())).unwrap(), Value::Null);
    }

    #[test]
    fn serialize_in_containers() {
        assert_eq!(
            serde_json::to_value(vec![Coerced(1_u8), Coerced(2), Coerced(3)]).unwrap(),
            json!([1, 2, 3])
        );
        assert_eq!(
            serde_json::to_value((Coerced(1_i32), Coerced("a"))).unwrap(),
            json!([1, "a"])
        );
        assert_eq!(
            serde_json::to_value(Some(Coerced(9_i32))).unwrap(),
            json!(9)
        );
        assert_eq!(
            serde_json::to_value(None::<Coerced<i32>>).unwrap(),
            Value::Null
        );

        #[derive(serde::Serialize)]
        struct Data {
            id: Coerced<u32>,
            name: Coerced<&'static str>,
        }

        assert_eq!(
            serde_json::to_value(Data {
                id: Coerced(7),
                name: Coerced("rquickjs"),
            }).unwrap(),
            json!({ "id": 7, "name": "rquickjs" })
        );
    }

    #[test]
    fn deserialize_is_transparent() {
        assert_eq!(serde_json::from_str::<Coerced<i32>>("42").unwrap().0, 42);
        assert_eq!(
            serde_json::from_str::<Coerced<String>>("\"abc\"")
                .unwrap()
                .0,
            "abc"
        );
        assert!(serde_json::from_str::<Coerced<bool>>("true").unwrap().0);
        assert_eq!(
            serde_json::from_str::<Coerced<Vec<u8>>>("[1,2,3]")
                .unwrap()
                .0,
            vec![1, 2, 3]
        );

        // Deserializing borrowed data keeps working through the wrapper.
        let borrowed: Coerced<&str> = serde_json::from_str("\"borrowed\"").unwrap();
        assert_eq!(borrowed.0, "borrowed");
    }

    #[test]
    fn deserialize_is_still_typed() {
        // The wrapper must not add any coercion of its own: a JSON string is
        // still not accepted where an integer is expected.
        assert!(serde_json::from_str::<Coerced<i32>>("\"42\"").is_err());
        assert!(serde_json::from_str::<Coerced<String>>("42").is_err());
    }

    #[test]
    fn roundtrip() {
        #[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize)]
        struct Data {
            id: Coerced<u32>,
            name: Coerced<String>,
            tags: Vec<Coerced<String>>,
        }

        let data = Data {
            id: Coerced(7),
            name: Coerced("rquickjs".to_owned()),
            tags: vec![Coerced("js".to_owned()), Coerced("serde".to_owned())],
        };

        let json = serde_json::to_string(&data).unwrap();
        assert_eq!(json, r#"{"id":7,"name":"rquickjs","tags":["js","serde"]}"#);

        let decoded: Data = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, data);
    }

    #[test]
    fn coerced_js_values_serialize() {
        crate::test_with(|ctx| {
            // A coerced JS number is already a Rust string, so it serializes as
            // a JSON string rather than a JSON number.
            let value = ctx.eval::<Coerced<String>, _>("123").unwrap();
            assert_eq!(serde_json::to_string(&value).unwrap(), "\"123\"");

            // A deserialized wrapper can be handed back to JavaScript.
            let value: Coerced<i32> = serde_json::from_str("42").unwrap();
            ctx.globals().set("value", value.0).unwrap();
            assert_eq!(ctx.eval::<i32, _>("value").unwrap(), 42);
        });
    }
}
