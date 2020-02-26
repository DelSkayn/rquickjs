use super::ToJs;
use crate::{Array, Ctx, Object, Result, String, Value};
use std::string::String as StdString;

impl<'js> ToJs<'js> for Value<'js> {
    fn to_js(self, _: Ctx<'js>) -> Result<Value<'js>> {
        Ok(self)
    }
}

impl<'js> ToJs<'js> for String<'js> {
    fn to_js(self, _: Ctx<'js>) -> Result<Value<'js>> {
        Ok(Value::String(self))
    }
}

impl<'js> ToJs<'js> for Object<'js> {
    fn to_js(self, _: Ctx<'js>) -> Result<Value<'js>> {
        Ok(Value::Object(self))
    }
}

impl<'js> ToJs<'js> for Array<'js> {
    fn to_js(self, _: Ctx<'js>) -> Result<Value<'js>> {
        Ok(Value::Array(self))
    }
}

impl<'js> ToJs<'js> for StdString {
    fn to_js(self, ctx: Ctx<'js>) -> Result<Value<'js>> {
        let s = String::from_str(ctx, self.as_str())?;
        Ok(Value::String(s))
    }
}

impl<'js, 'a> ToJs<'js> for &'a str {
    fn to_js(self, ctx: Ctx<'js>) -> Result<Value<'js>> {
        let s = String::from_str(ctx, self)?;
        Ok(Value::String(s))
    }
}

impl<'js> ToJs<'js> for i32 {
    fn to_js(self, _: Ctx<'js>) -> Result<Value<'js>> {
        Ok(Value::Int(self))
    }
}
