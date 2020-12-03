mod array;
mod atom;
mod convert;
mod function;
mod js_ref;
mod module;
mod object;
mod string;
mod symbol;

use std::{panic, panic::UnwindSafe};

use crate::{qjs, Ctx, Error, Result};

pub use module::{AfterInit, BeforeInit, Module, ModuleDef};
#[cfg(feature = "exports")]
pub use module::{ExportEntriesIter, ExportNamesIter};

pub use array::Array;
pub use atom::*;
pub use convert::*;
pub use function::{
    Args, AsArguments, AsFunction, AsFunctionMut, Function, JsFn, JsFnMut, Method, This,
};
pub(crate) use js_ref::{JsRef, JsRefType};
pub use object::{Object, ObjectDef};
pub use string::String;
pub use symbol::Symbol;

/// The trait to get raw JS values from referenced JS types
pub trait AsJsValueRef<'js> {
    fn as_js_value_ref(&self) -> qjs::JSValue;
}

macro_rules! as_js_value_ref_impls {
    ($($t:ident,)*) => {
        $(
            impl<'js> AsJsValueRef<'js> for $t<'js> {
                fn as_js_value_ref(&self) -> qjs::JSValue {
                    self.0.as_js_value()
                }
            }
        )*
    };
}

as_js_value_ref_impls! {
    Function,
    Symbol,
    String,
    Object,
    Array,
}

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

    let exception = Value::from_js_value(ctx, exception_val).unwrap();
    Error::from_js(ctx, exception).unwrap()
}

impl<'js> Value<'js> {
    unsafe fn from_js_value_common(
        ctx: Ctx<'js>,
        tag: qjs::c_int,
        v: qjs::JSValue,
    ) -> Result<Self> {
        //TODO test for overflow in down cast
        //Should probably not happen
        match tag {
            qjs::JS_TAG_INT => Ok(Value::Int(qjs::JS_VALUE_GET_INT(v))),
            qjs::JS_TAG_BOOL => Ok(Value::Bool(qjs::JS_VALUE_GET_BOOL(v))),
            qjs::JS_TAG_NULL => Ok(Value::Null),
            qjs::JS_TAG_UNDEFINED => Ok(Value::Undefined),
            qjs::JS_TAG_UNINITIALIZED => Ok(Value::Uninitialized),
            qjs::JS_TAG_FLOAT64 => Ok(Value::Float(qjs::JS_VALUE_GET_FLOAT64(v))),

            qjs::JS_TAG_MODULE => {
                // Just to make sure things are properly cleaned up;
                Module::<AfterInit>::from_js_value(ctx, v);
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

    unsafe fn from_js_object(
        ctx: Ctx<'js>,
        val: qjs::JSValue,
        vref: JsRef<'js, Object<'js>>,
    ) -> Self {
        if qjs::JS_IsArray(ctx.ctx, val) == 1 {
            Array(vref).into_value()
        } else if qjs::JS_IsFunction(ctx.ctx, val) == 1 {
            Function(vref).into_value()
        } else {
            Object(vref).into_value()
        }
    }

    // unsafe becuase the value must belong the context and the lifetime must be constrained
    // by its lifetime
    pub(crate) unsafe fn from_js_value(ctx: Ctx<'js>, val: qjs::JSValue) -> Result<Self> {
        let val = handle_exception(ctx, val)?;
        //TODO test for overflow in down cast
        //Should probably not happen
        match qjs::JS_VALUE_GET_NORM_TAG(val) {
            qjs::JS_TAG_STRING => Ok(String(JsRef::from_js_value(ctx, val)).into_value()),
            qjs::JS_TAG_SYMBOL => Ok(Symbol(JsRef::from_js_value(ctx, val)).into_value()),
            qjs::JS_TAG_OBJECT => Ok(Self::from_js_object(
                ctx,
                val,
                JsRef::from_js_value(ctx, val),
            )),
            tag => Self::from_js_value_common(ctx, tag, val),
        }
    }

    #[allow(dead_code)]
    pub(crate) unsafe fn from_js_value_const(ctx: Ctx<'js>, val: qjs::JSValue) -> Result<Self> {
        let val = handle_exception(ctx, val)?;
        //TODO test for overflow in down cast
        //Should probably not happen
        match qjs::JS_VALUE_GET_NORM_TAG(val) {
            qjs::JS_TAG_STRING => Ok(String(JsRef::from_js_value_const(ctx, val)).into_value()),
            qjs::JS_TAG_SYMBOL => Ok(Symbol(JsRef::from_js_value_const(ctx, val)).into_value()),
            qjs::JS_TAG_OBJECT => Ok(Self::from_js_object(
                ctx,
                val,
                JsRef::from_js_value_const(ctx, val),
            )),
            tag => Self::from_js_value_common(ctx, tag, val),
        }
    }

    fn to_js_value_common(&self) -> qjs::JSValue {
        match self {
            Value::Int(x) => qjs::JS_MKVAL(qjs::JS_TAG_INT, *x),
            Value::Bool(x) => {
                if *x {
                    qjs::JS_TRUE
                } else {
                    qjs::JS_FALSE
                }
            }
            Value::Null => qjs::JS_NULL,
            Value::Undefined => qjs::JS_UNDEFINED,
            Value::Uninitialized => qjs::JS_UNINITIALIZED,
            Value::Float(x) => qjs::JS_NewFloat64(*x),
            _ => unreachable!(),
        }
    }

    pub(crate) fn as_js_value(&self) -> qjs::JSValue {
        match self {
            Value::Symbol(x) => x.0.as_js_value(),
            Value::String(x) => x.0.as_js_value(),
            Value::Object(x) => x.0.as_js_value(),
            Value::Array(x) => x.0.as_js_value(),
            Value::Function(x) => x.0.as_js_value(),
            other => other.to_js_value_common(),
        }
    }

    pub(crate) fn into_js_value(self) -> qjs::JSValue {
        match self {
            Value::Symbol(x) => x.0.into_js_value(),
            Value::String(x) => x.0.into_js_value(),
            Value::Object(x) => x.0.into_js_value(),
            Value::Array(x) => x.0.into_js_value(),
            Value::Function(x) => x.0.into_js_value(),
            other => other.to_js_value_common(),
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

macro_rules! conv_impls {
    ($($type:ident,)*) => {
        $(
            impl<'js> From<$type<'js>> for Value<'js> {
                fn from(value: $type<'js>) -> Self {
                    Value::$type(value)
                }
            }
        )*
    };
}

conv_impls! {
    Function,
    Symbol,
    String,
    Object,
    Array,
}
