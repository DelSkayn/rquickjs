use super::FromJs;
use crate::{Array, Ctx, Error, Function, Object, Result, String, Value};
use std::string::String as StdString;

impl<'js> FromJs<'js> for Value<'js> {
    fn from_js(_: Ctx<'js>, value: Value<'js>) -> Result<Self> {
        Ok(value)
    }
}

impl<'js> FromJs<'js> for StdString {
    fn from_js(ctx: Ctx<'js>, value: Value<'js>) -> Result<Self> {
        let type_name = value.type_name();
        let res = ctx
            .coerce_string(value)
            .map_err(|e| {
                if let Error::Exception(text) = e {
                    Error::FromJsConversion {
                        from: type_name,
                        to: "string",
                        message: Some(text),
                    }
                } else {
                    e
                }
            })?
            .to_str()?
            .to_string();
        Ok(res)
    }
}

impl<'js> FromJs<'js> for i32 {
    fn from_js(ctx: Ctx<'js>, value: Value<'js>) -> Result<Self> {
        let type_name = value.type_name();
        ctx.coerce_i32(value).map_err(|e| {
            if let Error::Exception(text) = e {
                Error::FromJsConversion {
                    from: type_name,
                    to: "i32",
                    message: Some(text),
                }
            } else {
                e
            }
        })
    }
}

impl<'js> FromJs<'js> for u64 {
    fn from_js(ctx: Ctx<'js>, value: Value<'js>) -> Result<Self> {
        let type_name = value.type_name();
        ctx.coerce_u64(value).map_err(|e| {
            if let Error::Exception(text) = e {
                Error::FromJsConversion {
                    from: type_name,
                    to: "u32",
                    message: Some(text),
                }
            } else {
                e
            }
        })
    }
}

impl<'js> FromJs<'js> for f64 {
    fn from_js(ctx: Ctx<'js>, value: Value<'js>) -> Result<Self> {
        let type_name = value.type_name();
        ctx.coerce_f64(value).map_err(|e| {
            if let Error::Exception(text) = e {
                Error::FromJsConversion {
                    from: type_name,
                    to: "f64",
                    message: Some(text),
                }
            } else {
                e
            }
        })
    }
}

impl<'js> FromJs<'js> for bool {
    fn from_js(ctx: Ctx<'js>, value: Value<'js>) -> Result<Self> {
        let type_name = value.type_name();
        ctx.coerce_bool(value).map_err(|e| {
            if let Error::Exception(text) = e {
                Error::FromJsConversion {
                    from: type_name,
                    to: "bool",
                    message: Some(text),
                }
            } else {
                e
            }
        })
    }
}

impl<'js> FromJs<'js> for String<'js> {
    fn from_js(_: Ctx<'js>, value: Value<'js>) -> Result<Self> {
        match value {
            Value::String(x) => Ok(x),
            x => Err(Error::FromJsConversion {
                from: x.type_name(),
                to: "string",
                message: None,
            }),
        }
    }
}

impl<'js> FromJs<'js> for Object<'js> {
    fn from_js(_: Ctx<'js>, value: Value<'js>) -> Result<Self> {
        match value {
            Value::Object(x) => Ok(x),
            x => Err(Error::FromJsConversion {
                from: x.type_name(),
                to: "object",
                message: None,
            }),
        }
    }
}

impl<'js> FromJs<'js> for Array<'js> {
    fn from_js(_: Ctx<'js>, value: Value<'js>) -> Result<Self> {
        match value {
            Value::Object(x) => Ok(Array::from_object(x)),
            Value::Array(x) => Ok(x),
            x => Err(Error::FromJsConversion {
                from: x.type_name(),
                to: "array",
                message: None,
            }),
        }
    }
}

impl<'js> FromJs<'js> for Function<'js> {
    fn from_js(_: Ctx<'js>, value: Value<'js>) -> Result<Self> {
        match value {
            Value::Function(x) => Ok(x),
            x => Err(Error::FromJsConversion {
                from: x.type_name(),
                to: "function",
                message: None,
            }),
        }
    }
}

impl<'js> FromJs<'js> for () {
    fn from_js(_: Ctx<'js>, _: Value<'js>) -> Result<Self> {
        Ok(())
    }
}
