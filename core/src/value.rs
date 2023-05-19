mod array;
mod atom;
mod bigint;
mod convert;
mod function;
mod module;
mod object;
mod string;
mod symbol;

#[cfg(feature = "array-buffer")]
mod array_buffer;
#[cfg(feature = "array-buffer")]
mod typed_array;

use crate::{qjs, Ctx, Error, Result};

pub use module::{
    Declarations, Exports, Module, ModuleData, ModuleDataKind, ModuleDef, ModuleLoadFn,
    ModulesBuilder,
};

pub use array::Array;
pub use atom::*;
pub use bigint::BigInt;
pub use convert::*;
pub use function::{
    AsArguments, AsFunction, Func, Function, Method, MutFn, OnceFn, Opt, Rest, This,
};
pub use object::{Filter, Object, ObjectDef};
pub use string::String;
pub use symbol::Symbol;

#[cfg(feature = "array-buffer")]
pub use array_buffer::ArrayBuffer;
#[cfg(feature = "array-buffer")]
pub use typed_array::TypedArray;

#[cfg(feature = "futures")]
pub use function::Async;

use std::{fmt, mem, ops::Deref, result::Result as StdResult, str};

/// Any javascript value
pub struct Value<'js> {
    pub(crate) ctx: Ctx<'js>,
    pub(crate) value: qjs::JSValue,
}

impl<'js> Clone for Value<'js> {
    fn clone(&self) -> Self {
        let ctx = self.ctx;
        let value = unsafe { qjs::JS_DupValue(self.value) };
        Self { ctx, value }
    }
}

impl<'js> Drop for Value<'js> {
    fn drop(&mut self) {
        unsafe {
            qjs::JS_FreeValue(self.ctx.as_ptr(), self.value);
        }
    }
}

impl<'js> fmt::Debug for Value<'js> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let type_ = self.type_of();
        type_.fmt(f)?;
        use Type::*;
        match type_ {
            Bool | Int | Float => {
                '('.fmt(f)?;
                match type_ {
                    Bool => unsafe { self.get_bool() }.fmt(f)?,
                    Int => unsafe { self.get_int() }.fmt(f)?,
                    Float => unsafe { self.get_float() }.fmt(f)?,
                    _ => unreachable!(),
                }
                ')'.fmt(f)?;
            }
            String => {
                "(\"".fmt(f)?;
                unsafe { self.ref_string() }.to_string().fmt(f)?;
                "\")".fmt(f)?;
            }
            Symbol | Object | Array | Function => {
                '('.fmt(f)?;
                unsafe { self.get_ptr() }.fmt(f)?;
                ')'.fmt(f)?;
            }
            _ => (),
        }
        Ok(())
    }
}

impl<'js> PartialEq for Value<'js> {
    fn eq(&self, other: &Self) -> bool {
        let type_ = self.type_of();
        if type_ != other.type_of() {
            return false;
        }
        use Type::*;
        match type_ {
            Uninitialized | Undefined | Null => true,
            Bool => unsafe { self.get_bool() == other.get_bool() },
            Int => unsafe { self.get_int() == other.get_int() },
            Float => unsafe { self.get_float() == other.get_float() },
            _ => unsafe { self.get_ptr() == other.get_ptr() },
        }
    }
}

impl<'js> Value<'js> {
    // unsafe becuase the value must belong the context and the lifetime must be constrained by its lifetime
    #[inline]
    pub(crate) unsafe fn from_js_value(ctx: Ctx<'js>, value: qjs::JSValue) -> Self {
        Self { ctx, value }
    }

    #[inline]
    pub(crate) unsafe fn from_js_value_const(ctx: Ctx<'js>, value: qjs::JSValueConst) -> Self {
        let value = qjs::JS_DupValue(value);
        Self { ctx, value }
    }

    #[inline]
    pub(crate) fn as_js_value(&self) -> qjs::JSValueConst {
        self.value
    }

    #[inline]
    pub(crate) fn into_js_value(self) -> qjs::JSValue {
        let value = self.value;
        mem::forget(self);
        value
    }

    #[inline]
    pub fn new_uninitialized(ctx: Ctx<'js>) -> Self {
        let value = qjs::JS_UNINITIALIZED;
        Self { ctx, value }
    }

    #[inline]
    pub fn new_undefined(ctx: Ctx<'js>) -> Self {
        let value = qjs::JS_UNDEFINED;
        Self { ctx, value }
    }

    #[inline]
    pub fn new_null(ctx: Ctx<'js>) -> Self {
        let value = qjs::JS_NULL;
        Self { ctx, value }
    }

    /// Create new boolean value
    #[inline]
    pub fn new_bool(ctx: Ctx<'js>, value: bool) -> Self {
        let value = if value { qjs::JS_TRUE } else { qjs::JS_FALSE };
        Self { ctx, value }
    }

    // unsafe because no type checking
    #[inline]
    pub(crate) unsafe fn get_bool(&self) -> bool {
        qjs::JS_VALUE_GET_BOOL(self.value)
    }

    /// Try get bool from value
    pub fn as_bool(&self) -> Option<bool> {
        if self.is_bool() {
            Some(unsafe { self.get_bool() })
        } else {
            None
        }
    }

    /// Create new int value
    #[inline]
    pub fn new_int(ctx: Ctx<'js>, value: i32) -> Self {
        let value = qjs::JS_MKVAL(qjs::JS_TAG_INT, value);
        Self { ctx, value }
    }

    #[inline]
    pub(crate) unsafe fn get_int(&self) -> i32 {
        qjs::JS_VALUE_GET_INT(self.value)
    }

    /// Try get int from value
    pub fn as_int(&self) -> Option<i32> {
        if self.is_int() {
            Some(unsafe { self.get_int() })
        } else {
            None
        }
    }

    /// Create new float value
    #[inline]
    pub fn new_float(ctx: Ctx<'js>, value: f64) -> Self {
        let value = qjs::JS_NewFloat64(value);
        Self { ctx, value }
    }

    #[inline]
    pub(crate) unsafe fn get_float(&self) -> f64 {
        qjs::JS_VALUE_GET_FLOAT64(self.value)
    }

    /// Try get float from value
    pub fn as_float(&self) -> Option<f64> {
        if self.is_float() {
            Some(unsafe { self.get_float() })
        } else {
            None
        }
    }

    /// Create a new number value
    #[inline]
    pub fn new_number(ctx: Ctx<'js>, value: f64) -> Self {
        let int = value as i32;
        #[allow(clippy::float_cmp)]
        // This is safe and fast in that case
        let value = if value == int as f64 {
            qjs::JS_MKVAL(qjs::JS_TAG_INT, int)
        } else {
            qjs::JS_NewFloat64(value)
        };
        Self { ctx, value }
    }

    /// Try get any number from value
    pub fn as_number(&self) -> Option<f64> {
        if self.is_int() {
            Some(unsafe { self.get_int() as _ })
        } else if self.is_float() {
            Some(unsafe { self.get_float() })
        } else {
            None
        }
    }

    #[allow(unused)]
    #[inline]
    pub(crate) fn new_ptr(ctx: Ctx<'js>, tag: qjs::c_int, ptr: *mut qjs::c_void) -> Self {
        let value = qjs::JS_MKPTR(tag, ptr);
        Self { ctx, value }
    }

    #[allow(unused)]
    #[inline]
    pub(crate) fn new_ptr_const(ctx: Ctx<'js>, tag: qjs::c_int, ptr: *mut qjs::c_void) -> Self {
        let value = unsafe { qjs::JS_DupValue(qjs::JS_MKPTR(tag, ptr)) };
        Self { ctx, value }
    }

    #[inline]
    pub(crate) unsafe fn get_ptr(&self) -> *mut qjs::c_void {
        qjs::JS_VALUE_GET_PTR(self.value)
    }

    /// Check if the value is a bool
    #[inline]
    pub fn is_bool(&self) -> bool {
        qjs::JS_TAG_BOOL == unsafe { qjs::JS_VALUE_GET_TAG(self.value) }
    }

    /// Check if the value is an int
    #[inline]
    pub fn is_int(&self) -> bool {
        qjs::JS_TAG_INT == unsafe { qjs::JS_VALUE_GET_TAG(self.value) }
    }

    /// Check if the value is a float
    #[inline]
    pub fn is_float(&self) -> bool {
        qjs::JS_TAG_FLOAT64 == unsafe { qjs::JS_VALUE_GET_NORM_TAG(self.value) }
    }

    /// Check if the value is an any number
    #[inline]
    pub fn is_number(&self) -> bool {
        let tag = unsafe { qjs::JS_VALUE_GET_NORM_TAG(self.value) };
        qjs::JS_TAG_INT == tag || qjs::JS_TAG_FLOAT64 == tag
    }

    /// Check if the value is a string
    #[inline]
    pub fn is_string(&self) -> bool {
        qjs::JS_TAG_STRING == unsafe { qjs::JS_VALUE_GET_TAG(self.value) }
    }

    /// Check if the value is a symbol
    #[inline]
    pub fn is_symbol(&self) -> bool {
        qjs::JS_TAG_SYMBOL == unsafe { qjs::JS_VALUE_GET_TAG(self.value) }
    }

    /// Check if the value is an object
    #[inline]
    pub fn is_object(&self) -> bool {
        qjs::JS_TAG_OBJECT == unsafe { qjs::JS_VALUE_GET_TAG(self.value) }
    }

    /// Check if the value is a module
    #[inline]
    pub fn is_module(&self) -> bool {
        qjs::JS_TAG_MODULE == unsafe { qjs::JS_VALUE_GET_TAG(self.value) }
    }

    /// Check if the value is an array
    #[inline]
    pub fn is_array(&self) -> bool {
        0 != unsafe { qjs::JS_IsArray(self.ctx.as_ptr(), self.value) }
    }

    /// Check if the value is a function
    #[inline]
    pub fn is_function(&self) -> bool {
        0 != unsafe { qjs::JS_IsFunction(self.ctx.as_ptr(), self.value) }
    }

    /// Check if the value is an error
    #[inline]
    pub fn is_error(&self) -> bool {
        0 != unsafe { qjs::JS_IsError(self.ctx.as_ptr(), self.value) }
    }

    /// Reference as value
    #[inline]
    pub fn as_value(&self) -> &Self {
        self
    }

    /// Convert from value to specified type
    pub fn get<T: FromJs<'js>>(&self) -> Result<T> {
        T::from_js(self.ctx, self.clone())
    }
}

impl<'js> AsRef<Value<'js>> for Value<'js> {
    fn as_ref(&self) -> &Value<'js> {
        self
    }
}

macro_rules! type_impls {
    // type: name => tag
    ($($type:ident: $name:ident => $tag:ident,)*) => {
        /// The type of value
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        #[repr(i32)]
        pub enum Type {
            $($type,)*
            Unknown
        }

        impl Type {
            /// Returns true if the type is one of `uninitialized`, `undefined` or `null`
            pub const fn is_void(self) -> bool {
                use Type::*;
                matches!(self, Uninitialized | Undefined | Null)
            }

            /// Check the type for similarity
            pub const fn interpretable_as(self, other: Self) -> bool {
                use Type::*;

                let t = self as i32;
                let o = other as i32;

                o == t ||
                    (o == Float as i32 && t == Int as i32) ||
                    (o == Object as i32 && (t == Array as i32 ||
                                            t == Function as i32))
            }

            /// Returns string representation of type
            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Type::$type => stringify!($name),)*
                    Type::Unknown => "Unknown type",
                }
            }
        }

        impl AsRef<str> for Type {
            fn as_ref(&self) -> &str {
                self.as_str()
            }
        }

        impl fmt::Display for Type {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                self.as_str().fmt(f)
            }
        }

        impl str::FromStr for Type {
            type Err = ();

            fn from_str(s: &str) -> StdResult<Self, Self::Err> {
                Ok(match s {
                    $(stringify!($name) => Type::$type,)*
                    _ => return Err(()),
                })
            }
        }

        impl<'js> Value<'js> {
            /// Get the type of value
            pub fn type_of(&self) -> Type {
                let tag = unsafe { qjs::JS_VALUE_GET_NORM_TAG(self.value) };
                match tag {
                    $(qjs::$tag if type_impls!(@cond $type self) => Type::$type,)*
                    _ => Type::Unknown,
                }
            }

            /// Get the name of type
            pub fn type_name(&self) -> &'static str {
                self.type_of().as_str()
            }
        }
    };

    (@cond Array $self:expr) => { $self.is_array() };
    (@cond Function $self:expr) => { $self.is_function() };
    (@cond $type:ident $self:expr) => { true };
}

type_impls! {
    Uninitialized: uninitialized => JS_TAG_UNINITIALIZED,
    Undefined: undefined => JS_TAG_UNDEFINED,
    Null: null => JS_TAG_NULL,
    Bool: bool => JS_TAG_BOOL,
    Int: int => JS_TAG_INT,
    Float: float => JS_TAG_FLOAT64,
    String: string => JS_TAG_STRING,
    Symbol: symbol => JS_TAG_SYMBOL,
    Array: array => JS_TAG_OBJECT,
    Function: function => JS_TAG_OBJECT,
    Object: object => JS_TAG_OBJECT,
    Module: module => JS_TAG_MODULE,
    BigInt: big_int => JS_TAG_BIG_INT,
}

macro_rules! sub_types {
    ($($type:ident $as:ident $ref:ident $into:ident $from:ident,)*) => {
        $(
            impl<'js> sub_types!(@type $type) {
                /// Reference to value
                #[inline]
                pub fn as_value(&self) -> &Value<'js> {
                    &self.0
                }

                /// Convert into value
                #[inline]
                pub fn into_value(self) -> Value<'js> {
                    self.0
                }

                /// Convert from value
                pub fn from_value(value: Value<'js>) -> Result<Self> {
                    let type_ = value.type_of();
                    if type_.interpretable_as(Type::$type) {
                        Ok(sub_types!(@wrap $type value))
                    } else {
                        Err(Error::new_from_js(type_.as_str(), Type::$type.as_str()))
                    }
                }

                #[allow(unused)]
                pub(crate) unsafe fn from_js_value_const(ctx: Ctx<'js>, value: qjs::JSValueConst) -> Self {
                    sub_types!(@wrap $type Value::from_js_value_const(ctx, value))
                }

                #[allow(unused)]
                pub(crate) unsafe fn from_js_value(ctx: Ctx<'js>, value: qjs::JSValue) -> Self {
                    sub_types!(@wrap $type Value::from_js_value(ctx, value))
                }
            }

            impl<'js> Deref for sub_types!(@type $type) {
                type Target = Value<'js>;

                fn deref(&self) -> &Self::Target {
                    &self.0
                }
            }

            impl<'js> AsRef<Value<'js>> for sub_types!(@type $type) {
                fn as_ref(&self) -> &Value<'js> {
                    &self.0
                }
            }

            impl<'js> Value<'js> {
                /// Interprete as
                ///
                /// # Safety
                /// You should be sure that the value already is of required type before to do it.
                #[inline]
                pub unsafe fn $ref(&self) -> &sub_types!(@type $type) {
                    &*(self as *const _ as *const $type)
                }

                /// Try reinterprete as
                pub fn $as(&self) -> Option<&sub_types!(@type $type)> {
                    if self.type_of().interpretable_as(Type::$type) {
                        Some(unsafe { self.$ref() })
                    } else {
                        None
                    }
                }

                /// Try convert into
                pub fn $into(self) -> Option<sub_types!(@type $type)> {
                    if self.type_of().interpretable_as(Type::$type) {
                        Some(sub_types!(@wrap $type self))
                    } else {
                        None
                    }
                }

                /// Convert from
                pub fn $from(value: sub_types!(@type $type)) -> Self {
                    value.0
                }
            }

            impl<'js> From<sub_types!(@type $type)> for Value<'js> {
                fn from(value: sub_types!(@type $type)) -> Self {
                    value.0
                }
            }

            impl<'js> FromJs<'js> for sub_types!(@type $type) {
                fn from_js(_: Ctx<'js>, value: Value<'js>) -> Result<Self> {
                    Self::from_value(value)
                }
            }

            impl<'js> IntoJs<'js> for sub_types!(@type $type) {
                fn into_js(self, _: Ctx<'js>) -> Result<Value<'js>> {
                    Ok(self.0)
                }
            }

            impl<'js> IntoAtom<'js> for sub_types!(@type $type) {
                fn into_atom(self, ctx: Ctx<'js>) -> Atom<'js> {
                    Atom::from_value(ctx, &self.0)
                }
            }
        )*
    };

    (@type Module) => { EvaluatedModule<'js> };
    (@type $type:ident) => { $type<'js> };

    (@wrap Module $val:expr) => { Module($val, PhantomData) };
    (@wrap $type:ident $val:expr) => { $type($val) };
}

sub_types! {
    String as_string ref_string into_string from_string,
    Symbol as_symbol ref_symbol into_symbol from_symbol,
    Object as_object ref_object into_object from_object,
    Array as_array ref_array into_array from_array,
    Function as_function ref_function into_function from_function,
    BigInt as_big_int ref_big_int into_big_int from_big_int,
}

macro_rules! void_types {
    ($($(#[$meta:meta])* $type:ident $new:ident;)*) => {
        $(
            $(#[$meta])*
            #[derive(Debug, Copy, Clone, PartialEq, Eq)]
            pub struct $type;

            impl $type {
                /// Convert into value
                pub fn into_value<'js>(self, ctx: Ctx<'js>) -> Value<'js> {
                    Value::$new(ctx)
                }

                /// Convert from value
                pub fn from_value<'js>(value: Value<'js>) -> Result<Self> {
                    if value.type_of() == Type::$type {
                        Ok(Self)
                    } else {
                        Err(Error::new_from_js("value", Type::$type.as_str()))
                    }
                }
            }

            impl<'js> FromJs<'js> for $type {
                fn from_js(_: Ctx<'js>, value: Value<'js>) -> Result<Self> {
                    Self::from_value(value)
                }
            }

            impl<'js> IntoJs<'js> for $type {
                fn into_js(self, ctx: Ctx<'js>) -> Result<Value<'js>> {
                    Ok(self.into_value(ctx))
                }
            }
        )*
    };
}

void_types! {
    /// The placeholder which treated as uninitialized JS value
    Uninitialized new_uninitialized;

    /// The placeholder which treated as `undefined` value
    Undefined new_undefined;

    /// The placeholder which treated as `null` value
    Null new_null;
}

#[cfg(test)]
mod test {
    use crate::*;

    #[test]
    fn type_matches() {
        assert!(Type::Bool.interpretable_as(Type::Bool));

        assert!(Type::Object.interpretable_as(Type::Object));
        assert!(Type::Array.interpretable_as(Type::Object));
        assert!(Type::Function.interpretable_as(Type::Object));

        assert!(!Type::Object.interpretable_as(Type::Array));
        assert!(!Type::Object.interpretable_as(Type::Function));

        assert!(!Type::Bool.interpretable_as(Type::Int));
    }
}
