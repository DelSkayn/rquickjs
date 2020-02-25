use crate::{context::Ctx, Error};
use rquickjs_sys as qjs;
use std::ffi::CStr;

mod module;
pub use module::Module;
mod string;
pub use string::String;
mod object;
pub use object::Object;
mod array;
pub use array::Array;
mod symbol;
pub use symbol::Symbol;
mod convert;
pub use convert::*;
mod rf;

/// Any javascript value
#[derive(Debug, Clone, PartialEq)]
pub enum Value<'js> {
    Symbol(Symbol<'js>),
    String(String<'js>),
    Object(Object<'js>),
    Array(Array<'js>),
    Int(i32),
    Bool(bool),
    Null,
    Undefined,
    Uninitialized,
    Float(f64),
}

/// Handle possible exceptions in JSValue's and turn them into errors
/// Will return the JSValue if it is not an exception
pub(crate) unsafe fn handle_exception<'js>(
    ctx: Ctx<'js>,
    js_val: qjs::JSValue,
) -> Result<qjs::JSValue, Error> {
    if js_val.tag != qjs::JS_TAG_EXCEPTION as i64 {
        return Ok(js_val);
    }
    Err(get_exception(ctx))
}

pub(crate) unsafe fn get_exception<'js>(ctx: Ctx<'js>) -> Error {
    let exception_val = qjs::JS_GetException(ctx.ctx);
    let is_error = qjs::JS_IsError(ctx.ctx, exception_val);
    let s = qjs::JS_ToCString(ctx.ctx, exception_val);
    if s.is_null() {
        return Error::Unknown;
    }
    let mut exception_text = CStr::from_ptr(s).to_string_lossy().into_owned();
    qjs::JS_FreeCString(ctx.ctx, s);
    if is_error == 1 {
        let s_stack = CStr::from_bytes_with_nul(b"stack\0").unwrap();
        let val = qjs::JS_GetPropertyStr(ctx.ctx, exception_val, s_stack.as_ptr());
        if !qjs::JS_IsUndefined(val) {
            let stack = qjs::JS_ToCString(ctx.ctx, val);
            let text = match CStr::from_ptr(stack).to_str() {
                Err(e) => return e.into(),
                Ok(x) => x,
            };
            exception_text = format!("{}\n{}", exception_text, text);
            qjs::JS_FreeCString(ctx.ctx, stack);
        }
    }
    qjs::JS_FreeValue(ctx.ctx, exception_val);
    Error::Exception(exception_text)
}

impl<'js> Value<'js> {
    // unsafe becuase the value must belong the context and the lifetime must be constrained
    // by its lifetime
    pub(crate) unsafe fn from_js_value(ctx: Ctx<'js>, v: qjs::JSValue) -> Result<Self, Error> {
        let v = handle_exception(ctx, v)?;
        //TODO test for overflow in down cast
        //Should probably not happen
        match v.tag as i32 {
            qjs::JS_TAG_INT => Ok(Value::Int(qjs::JS_VALUE_GET_INT!(v))),
            qjs::JS_TAG_BOOL => Ok(Value::Bool(qjs::JS_VALUE_GET_BOOL!(v) != 0)),
            qjs::JS_TAG_NULL => Ok(Value::Null),
            qjs::JS_TAG_UNDEFINED => Ok(Value::Undefined),
            qjs::JS_TAG_UNINITIALIZED => Ok(Value::Uninitialized),
            qjs::JS_TAG_FLOAT64 => Ok(Value::Float(qjs::JS_VALUE_GET_FLOAT64!(v))),
            qjs::JS_TAG_STRING => Ok(Value::String(String::from_js_value(ctx, v))),
            qjs::JS_TAG_SYMBOL => Ok(Value::Symbol(Symbol::from_js_value(ctx, v))),
            qjs::JS_TAG_OBJECT => {
                if qjs::JS_IsArray(ctx.ctx, v) == 1 {
                    Ok(Value::Array(Array::from_js_value(ctx, v)))
                } else {
                    Ok(Value::Object(Object::from_js_value(ctx, v)))
                }
            }
            qjs::JS_TAG_MODULE => {
                // Just to make sure things are properly cleaned up;
                Module::from_js_value(ctx, v);
                panic!("recieved module JSValue for Value, Value should not handle modules.")
            }
            _ => {
                // Can we possibly leak here?
                // We should have catched all the possible
                // types which are reference counted so it should be fine.
                panic!("got unmatched js value type tag")
            }
        }
    }

    pub(crate) fn as_js_value(&self) -> qjs::JSValue {
        match *self {
            Value::Int(ref x) => qjs::JSValue {
                u: qjs::JSValueUnion { int32: *x },
                tag: qjs::JS_TAG_INT as i64,
            },
            Value::Bool(ref x) => qjs::JSValue {
                u: qjs::JSValueUnion {
                    int32: if *x { 1 } else { 0 },
                },
                tag: qjs::JS_TAG_BOOL as i64,
            },
            Value::Null => qjs::JSValue {
                u: qjs::JSValueUnion { int32: 0 },
                tag: qjs::JS_TAG_NULL as i64,
            },
            Value::Undefined => qjs::JSValue {
                u: qjs::JSValueUnion { int32: 0 },
                tag: qjs::JS_TAG_UNDEFINED as i64,
            },
            Value::Uninitialized => qjs::JSValue {
                u: qjs::JSValueUnion { int32: 0 },
                tag: qjs::JS_TAG_UNINITIALIZED as i64,
            },
            Value::Float(ref x) => qjs::JSValue {
                u: qjs::JSValueUnion { float64: *x },
                tag: qjs::JS_TAG_FLOAT64 as i64,
            },
            Value::Symbol(ref x) => x.as_js_value(),
            Value::String(ref x) => x.as_js_value(),
            Value::Object(ref x) => x.as_js_value(),
            Value::Array(ref x) => x.as_js_value(),
        }
    }

    pub(crate) fn type_name(&self) -> &'static str {
        match *self {
            Value::Int(_) => "integer",
            Value::Bool(_) => "bool",
            Value::Null => "null",
            Value::Undefined => "undefined",
            Value::Uninitialized => "uninitialized",
            Value::Float(_) => "float",
            Value::Symbol(_) => "symbol",
            Value::String(_) => "string",
            Value::Object(_) => "object",
            Value::Array(_) => "array",
        }
    }

    // Returns wether a value can be used
    // as a key for a object without first
    // converting it to a string
    /*
    pub fn is_key(&self) -> bool {
        match *self {
            Value::Int => true,
            Value::String => true,
            _ => false,
        }
    }
    */
}
