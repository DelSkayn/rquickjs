use crate::{context::Ctx, Error, Result};
use rquickjs_sys as qjs;
use std::panic::{self, UnwindSafe};

mod module;
#[cfg(feature = "exports")]
pub use module::ExportList;
pub use module::Module;

mod string;
pub use string::String;
mod object;
pub use object::Object;
mod array;
pub use array::Array;
mod symbol;
pub use symbol::Symbol;
pub mod function;
pub use function::Function;
mod convert;
pub use convert::*;
mod atom;
pub use atom::*;
pub(crate) mod rf;
use rf::{JsObjectRef, JsStringRef, JsSymbolRef};
mod multi;
pub use multi::{ArgsValue, ArgsValueJs, RestValues, ValueIter};

/// The `FromIterator` trait to use with `Ctx`
pub trait FromIteratorJs<'js, A>: Sized {
    type Item;

    fn from_iter_js<T>(ctx: Ctx<'js>, iter: T) -> Result<Self>
    where
        T: IntoIterator<Item = A>;
}

/// The `Iterator` trait extension to support `Ctx`
pub trait IteratorJs<'js, A> {
    fn collect_js<B>(self, ctx: Ctx<'js>) -> Result<B>
    where
        B: FromIteratorJs<'js, A>;
}

impl<'js, T, A> IteratorJs<'js, A> for T
where
    T: Iterator<Item = A>,
{
    fn collect_js<B>(self, ctx: Ctx<'js>) -> Result<B>
    where
        B: FromIteratorJs<'js, A>,
    {
        B::from_iter_js(ctx, self)
    }
}

pub(crate) fn handle_panic<F: FnOnce() -> qjs::JSValue + UnwindSafe>(
    ctx: *mut qjs::JSContext,
    f: F,
) -> qjs::JSValue {
    unsafe {
        match panic::catch_unwind(f) {
            Ok(x) => x,
            Err(e) => {
                Ctx::from_ptr(ctx).get_opaque().panic = Some(e);
                qjs::JS_Throw(ctx, qjs::JS_MKVAL(qjs::JS_TAG_EXCEPTION, 0))
            }
        }
    }
}

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
///
/// # Safety
/// Assumes to have ownership of the JSValue
pub(crate) unsafe fn handle_exception<'js>(
    ctx: Ctx<'js>,
    js_val: qjs::JSValue,
) -> Result<qjs::JSValue> {
    if qjs::JS_VALUE_GET_NORM_TAG(js_val) != qjs::JS_TAG_EXCEPTION {
        return Ok(js_val);
    }
    Err(get_exception(ctx))
}

pub(crate) unsafe fn get_exception<'js>(ctx: Ctx<'js>) -> Error {
    let exception_val = qjs::JS_GetException(ctx.ctx);
    if let Some(x) = ctx.get_opaque().panic.take() {
        panic::resume_unwind(x);
    }

    let atom_stack = Atom::from_str(ctx, "stack");
    let atom_file_name = Atom::from_str(ctx, "fileName");
    let atom_line_number = Atom::from_str(ctx, "lineNumber");
    let atom_message = Atom::from_str(ctx, "message");
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
    pub(crate) unsafe fn from_js_value(ctx: Ctx<'js>, v: qjs::JSValue) -> Result<Self> {
        let v = handle_exception(ctx, v)?;
        //TODO test for overflow in down cast
        //Should probably not happen
        match qjs::JS_VALUE_GET_NORM_TAG(v) {
            qjs::JS_TAG_INT => Ok(Value::Int(qjs::JS_VALUE_GET_INT(v))),
            qjs::JS_TAG_BOOL => Ok(Value::Bool(qjs::JS_VALUE_GET_BOOL(v))),
            qjs::JS_TAG_NULL => Ok(Value::Null),
            qjs::JS_TAG_UNDEFINED => Ok(Value::Undefined),
            qjs::JS_TAG_UNINITIALIZED => Ok(Value::Uninitialized),
            qjs::JS_TAG_FLOAT64 => Ok(Value::Float(qjs::JS_VALUE_GET_FLOAT64(v))),
            qjs::JS_TAG_STRING => Ok(Value::String(String(JsStringRef::from_js_value(ctx, v)))),
            qjs::JS_TAG_SYMBOL => Ok(Value::Symbol(Symbol(JsSymbolRef::from_js_value(ctx, v)))),
            qjs::JS_TAG_OBJECT => {
                let val = JsObjectRef::from_js_value(ctx, v);
                if qjs::JS_IsArray(ctx.ctx, v) == 1 {
                    Ok(Value::Array(Array(val)))
                } else if qjs::JS_IsFunction(ctx.ctx, v) == 1 {
                    Ok(Value::Function(Function(val)))
                } else {
                    Ok(Value::Object(Object(val)))
                }
            }
            #[cfg(feature = "exports")]
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

    #[allow(dead_code)]
    pub(crate) unsafe fn from_js_value_const(ctx: Ctx<'js>, v: qjs::JSValue) -> Result<Self> {
        let v = handle_exception(ctx, v)?;
        //TODO test for overflow in down cast
        //Should probably not happen
        match qjs::JS_VALUE_GET_NORM_TAG(v) {
            qjs::JS_TAG_INT => Ok(Value::Int(qjs::JS_VALUE_GET_INT(v))),
            qjs::JS_TAG_BOOL => Ok(Value::Bool(qjs::JS_VALUE_GET_BOOL(v))),
            qjs::JS_TAG_NULL => Ok(Value::Null),
            qjs::JS_TAG_UNDEFINED => Ok(Value::Undefined),
            qjs::JS_TAG_UNINITIALIZED => Ok(Value::Uninitialized),
            qjs::JS_TAG_FLOAT64 => Ok(Value::Float(qjs::JS_VALUE_GET_FLOAT64(v))),
            qjs::JS_TAG_STRING => Ok(Value::String(String(JsStringRef::from_js_value_const(
                ctx, v,
            )))),
            qjs::JS_TAG_SYMBOL => Ok(Value::Symbol(Symbol(JsSymbolRef::from_js_value_const(
                ctx, v,
            )))),
            qjs::JS_TAG_OBJECT => {
                let val = JsObjectRef::from_js_value_const(ctx, v);
                if qjs::JS_IsArray(ctx.ctx, v) == 1 {
                    Ok(Value::Array(Array(val)))
                } else if qjs::JS_IsFunction(ctx.ctx, v) == 1 {
                    Ok(Value::Function(Function(val)))
                } else {
                    Ok(Value::Object(Object(val)))
                }
            }
            #[cfg(feature = "exports")]
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
        match self {
            Value::Int(x) => qjs::JS_MKVAL(qjs::JS_TAG_INT, *x),
            Value::Bool(x) => qjs::JS_MKVAL(qjs::JS_TAG_BOOL, if *x { 1 } else { 0 }),
            Value::Null => qjs::JS_MKVAL(qjs::JS_TAG_NULL, 0),
            Value::Undefined => qjs::JS_MKVAL(qjs::JS_TAG_UNDEFINED, 0),
            Value::Uninitialized => qjs::JS_MKVAL(qjs::JS_TAG_UNINITIALIZED, 0),
            Value::Float(x) => qjs::JS_NewFloat64(*x),
            Value::Symbol(x) => x.0.as_js_value(),
            Value::String(x) => x.0.as_js_value(),
            Value::Object(x) => x.0.as_js_value(),
            Value::Array(x) => x.0.as_js_value(),
            Value::Function(x) => x.0.as_js_value(),
        }
    }

    pub(crate) fn into_js_value(self) -> qjs::JSValue {
        match self {
            Value::Int(x) => qjs::JS_MKVAL(qjs::JS_TAG_INT, x),
            Value::Bool(x) => qjs::JS_MKVAL(qjs::JS_TAG_BOOL, if x { 1 } else { 0 }),
            Value::Null => qjs::JS_MKVAL(qjs::JS_TAG_NULL, 0),
            Value::Undefined => qjs::JS_MKVAL(qjs::JS_TAG_UNDEFINED, 0),
            Value::Uninitialized => qjs::JS_MKVAL(qjs::JS_TAG_UNINITIALIZED, 0),
            Value::Float(x) => qjs::JS_NewFloat64(x),
            Value::Symbol(x) => x.0.into_js_value(),
            Value::String(x) => x.0.into_js_value(),
            Value::Object(x) => x.0.into_js_value(),
            Value::Array(x) => x.0.into_js_value(),
            Value::Function(x) => x.0.into_js_value(),
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
