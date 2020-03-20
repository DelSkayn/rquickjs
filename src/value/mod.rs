use crate::{context::Ctx, Error};
use rquickjs_sys as qjs;
//use std::ffi::CStr;

mod module;
pub use module::{ExportList, Module};
mod string;
pub use string::String;
mod object;
pub use object::Object;
mod array;
pub use array::Array;
mod symbol;
pub use symbol::Symbol;
mod function;
pub use function::Function;
mod convert;
pub use convert::*;
mod atom;
pub use atom::*;
mod rf;

/// Any javascript value.
#[derive(Debug, Clone, PartialEq)]
pub enum Value<'js> {
    Function(Function<'js>),
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
    let atom_stack = Atom::from_str(ctx, "stack");
    let atom_file_name = Atom::from_str(ctx, "fileName");
    let atom_line_number = Atom::from_str(ctx, "lineNumber");
    let atom_message = Atom::from_str(ctx, "message");

    let exception_val = qjs::JS_GetException(ctx.ctx);
    // Dont know if is this is always correct
    // TODO test exceptions
    let message = Value::from_js_value(
        ctx,
        qjs::JS_GetProperty(ctx.ctx, exception_val, atom_message.atom),
    )
    .unwrap();
    let stack = Value::from_js_value(
        ctx,
        qjs::JS_GetProperty(ctx.ctx, exception_val, atom_stack.atom),
    )
    .unwrap();
    let file = Value::from_js_value(
        ctx,
        qjs::JS_GetProperty(ctx.ctx, exception_val, atom_file_name.atom),
    )
    .unwrap();
    let line = Value::from_js_value(
        ctx,
        qjs::JS_GetProperty(ctx.ctx, exception_val, atom_line_number.atom),
    )
    .unwrap();
    qjs::JS_FreeValue(ctx.ctx, exception_val);
    Error::Exception {
        message: FromJs::from_js(ctx, message).unwrap(),
        file: FromJs::from_js(ctx, file).unwrap_or_else(|_| "unknown".to_string()),
        line: f64::from_js(ctx, line).unwrap() as u32,
        stack: FromJs::from_js(ctx, stack).unwrap(),
    }
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
                } else if qjs::JS_IsFunction(ctx.ctx, v) == 1 {
                    Ok(Value::Function(Function::from_js_value(ctx, v)))
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
            Value::Float(ref x) => unsafe { qjs::JS_NewFloat64(*x) },
            Value::Symbol(ref x) => x.as_js_value(),
            Value::String(ref x) => x.as_js_value(),
            Value::Object(ref x) => x.as_js_value(),
            Value::Array(ref x) => x.as_js_value(),
            Value::Function(ref x) => x.as_js_value(),
        }
    }

    #[doc(hidden)]
    pub fn type_name(&self) -> &'static str {
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
            Value::Function(_) => "function",
        }
    }
}
