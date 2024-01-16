use crate::{qjs, Ctx, Error, Result};
use std::{fmt, hash::Hash, mem, ops::Deref, result::Result as StdResult, str};

pub mod array;
pub mod atom;
mod bigint;
pub mod convert;
mod exception;
pub mod function;
pub mod module;
pub mod object;
mod string;
mod symbol;

pub use array::Array;
pub use atom::Atom;
pub use bigint::BigInt;
pub use convert::{Coerced, FromAtom, FromIteratorJs, FromJs, IntoAtom, IntoJs, IteratorJs};
pub use exception::Exception;
pub use function::{Constructor, Function};
pub use module::Module;
pub use object::{Filter, Object};
pub use string::String;
pub use symbol::Symbol;

#[cfg(feature = "array-buffer")]
pub mod array_buffer;
#[cfg(feature = "array-buffer")]
pub mod typed_array;

#[cfg(feature = "array-buffer")]
pub use array_buffer::ArrayBuffer;
#[cfg(feature = "array-buffer")]
pub use typed_array::TypedArray;

/// Any JavaScript value
pub struct Value<'js> {
    pub(crate) ctx: Ctx<'js>,
    pub(crate) value: qjs::JSValue,
}

impl<'js> PartialEq for Value<'js> {
    fn eq(&self, other: &Self) -> bool {
        let tag = unsafe { qjs::JS_VALUE_GET_TAG(self.value) };
        let tag_other = unsafe { qjs::JS_VALUE_GET_TAG(other.value) };

        let bits = unsafe { qjs::JS_VALUE_GET_FLOAT64(self.value).to_bits() };
        let bits_other = unsafe { qjs::JS_VALUE_GET_FLOAT64(other.value).to_bits() };

        tag == tag_other && bits == bits_other
    }
}

impl<'js> Eq for Value<'js> {}

impl<'js> Hash for Value<'js> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        let tag = unsafe { qjs::JS_VALUE_GET_TAG(self.value) };
        let bits = unsafe { qjs::JS_VALUE_GET_FLOAT64(self.value).to_bits() };
        state.write_i32(tag);
        state.write_u64(bits)
    }
}

impl<'js> Clone for Value<'js> {
    fn clone(&self) -> Self {
        let ctx = self.ctx.clone();
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
                write!(f, "(")?;
                match type_ {
                    Bool => unsafe { self.get_bool() }.fmt(f)?,
                    Int => unsafe { self.get_int() }.fmt(f)?,
                    Float => unsafe { self.get_float() }.fmt(f)?,
                    _ => unreachable!(),
                }
                write!(f, ")")?;
            }
            String => {
                write!(f, "(")?;
                unsafe { self.ref_string() }.to_string().fmt(f)?;
                write!(f, ")")?;
            }
            Symbol | Object | Array | Function | Constructor => {
                write!(f, "(")?;
                unsafe { self.get_ptr() }.fmt(f)?;
                write!(f, ")")?;
            }
            Exception => {
                writeln!(f, "(")?;
                self.as_exception().unwrap().fmt(f)?;
                writeln!(f, ")")?;
            }
            Null => "null".fmt(f)?,
            Undefined => "undefined".fmt(f)?,
            Uninitialized => "uninitialized".fmt(f)?,
            Module => "module".fmt(f)?,
            BigInt => "BigInt".fmt(f)?,
            Unknown => "unknown".fmt(f)?,
        }
        Ok(())
    }
}

impl<'js> Value<'js> {
    // unsafe because the value must belong the context and the lifetime must be constrained by its lifetime
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
        unsafe { qjs::JS_FreeContext(self.ctx.as_ptr()) };
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

    /// Returns the Ctx object associated with this value.
    #[inline]
    pub fn ctx(&self) -> &Ctx<'js> {
        &self.ctx
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

    /// Returns if the value is the JavaScript null value.
    #[inline]
    pub fn is_null(&self) -> bool {
        let tag = unsafe { qjs::JS_VALUE_GET_NORM_TAG(self.value) };
        qjs::JS_TAG_NULL == tag
    }

    /// Returns if the value is the JavaScript undefined value.
    #[inline]
    pub fn is_undefined(&self) -> bool {
        let tag = unsafe { qjs::JS_VALUE_GET_NORM_TAG(self.value) };
        qjs::JS_TAG_UNDEFINED == tag
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

    /// Check if the value is a constructor function
    #[inline]
    pub fn is_constructor(&self) -> bool {
        0 != unsafe { qjs::JS_IsConstructor(self.ctx.as_ptr(), self.value) }
    }

    /// Check if the value is an exception
    #[inline]
    pub fn is_exception(&self) -> bool {
        unsafe { qjs::JS_IsException(self.value) }
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

    #[inline]
    pub(crate) fn into_value(self) -> Self {
        self
    }

    /// Convert from value to specified type
    pub fn get<T: FromJs<'js>>(&self) -> Result<T> {
        T::from_js(self.ctx(), self.clone())
    }

    /// Returns the raw C library JavaScript value.
    pub fn as_raw(&self) -> qjs::JSValue {
        self.value
    }

    /// Create a value from the C library JavaScript value.
    ///
    /// # Safety
    /// The value cannot be from an unrelated runtime and the value must be owned.
    /// QuickJS JavaScript values are reference counted. The drop implementation of this type
    /// decrements the reference count so the value must have count which won't be decremented
    /// elsewhere. Use [`qjs::JS_DupValue`] to increment the reference count of the value.
    pub unsafe fn from_raw(ctx: Ctx<'js>, value: qjs::JSValue) -> Self {
        Self::from_js_value(ctx, value)
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
        /// The type of JavaScript value
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        #[repr(u8)]
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

                if (self as u8) == (other as u8){
                    return true
                }
                match other{
                    Float => matches!(self, Int),
                    Object => matches!(self, Array | Function | Constructor | Exception),
                    Function => matches!(self, Constructor),
                    _ => false
                }
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
    (@cond Constructor $self:expr) => { $self.is_constructor() };
    (@cond Function $self:expr) => { $self.is_function() };
    (@cond Exception $self:expr) => { $self.is_error() };
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
    Constructor: constructor => JS_TAG_OBJECT,
    Function: function => JS_TAG_OBJECT,
    Exception: exception => JS_TAG_OBJECT,
    Object: object => JS_TAG_OBJECT,
    Module: module => JS_TAG_MODULE,
    BigInt: big_int => JS_TAG_BIG_INT,
}

macro_rules! sub_types {
    ($( $head:ident$(->$sub_type:ident)* $as:ident $ref:ident $into:ident $try_into:ident $from:ident,)*) => {
        $(
            impl<'js> $head<'js> {
                /// Reference to value
                #[inline]
                pub fn as_value(&self) -> &Value<'js> {
                    &self.0.as_value()
                }

                /// Convert into value
                #[inline]
                pub fn into_value(self) -> Value<'js> {
                    self.0.into_value()
                }

                /// Returns the underlying super type.
                pub fn into_inner(self) -> sub_types!(@head_ty $($sub_type),*) {
                    self.0
                }
                /// Returns a reference to the underlying super type.
                pub fn as_inner(&self) -> & sub_types!(@head_ty $($sub_type),*) {
                    &self.0
                }

                /// Returns the [`Ctx`] object associated with this value
                pub fn ctx(&self) -> &Ctx<'js>{
                    self.0.ctx()
                }

                /// Convert from value
                pub fn from_value(value: Value<'js>) -> Result<Self> {
                    let type_ = value.type_of();
                    if type_.interpretable_as(Type::$head) {
                        Ok(sub_types!(@wrap $head$(->$sub_type)*  value))
                    } else {
                        Err(Error::new_from_js(type_.as_str(), Type::$head.as_str()))
                    }
                }

                #[allow(unused)]
                pub(crate) unsafe fn from_js_value_const(ctx: Ctx<'js>, value: qjs::JSValueConst) -> Self {
                    sub_types!(@wrap $head$(->$sub_type)* Value::from_js_value_const(ctx, value))
                }

                #[allow(unused)]
                pub(crate) unsafe fn from_js_value(ctx: Ctx<'js>, value: qjs::JSValue) -> Self {
                    sub_types!(@wrap $head$(->$sub_type)* Value::from_js_value(ctx, value))
                }

                #[allow(unused)]
                pub(crate) fn into_js_value(self) -> qjs::JSValue{
                    self.0.into_js_value()
                }

                #[allow(unused)]
                pub(crate) fn as_js_value(&self) -> qjs::JSValueConst{
                    self.0.as_js_value()
                }
            }

            impl<'js> Deref for $head<'js> {
                type Target = sub_types!(@head_ty $($sub_type),*);

                fn deref(&self) -> &Self::Target {
                    &self.0
                }
            }

            sub_types!(@imp_as_ref $head$(,$sub_type)*);

            impl<'js> Value<'js> {
                #[doc = concat!("Interpret as [`",stringify!($head),"`]")]
                ///
                /// # Safety
                /// You should be sure that the value already is of required type before to do it.
                #[inline]
                pub unsafe fn $ref(&self) -> &$head<'js> {
                    &*(self as *const _ as *const $head)
                }

                #[doc = concat!("Try reinterpret as [`",stringify!($head),"`]")]
                pub fn $as(&self) -> Option<&$head<'js>> {
                    if self.type_of().interpretable_as(Type::$head) {
                        Some(unsafe { self.$ref() })
                    } else {
                        None
                    }
                }

                #[doc = concat!("Try convert into [`",stringify!($head),"`]")]
                pub fn $into(self) -> Option<$head<'js>> {
                    if self.type_of().interpretable_as(Type::$head) {
                        Some(sub_types!(@wrap $head$(->$sub_type)* self))
                    } else {
                        None
                    }
                }

                #[doc = concat!("Try convert into [`",stringify!($head),"`] returning self if the conversion fails.")]
                pub fn $try_into(self) -> std::result::Result<$head<'js>, Value<'js>> {
                    if self.type_of().interpretable_as(Type::$head) {
                        Ok(sub_types!(@wrap $head$(->$sub_type)* self))
                    } else {
                        Err(self)
                    }
                }

                #[doc = concat!("Convert from [`",stringify!($head),"`]")]
                pub fn $from(value: $head<'js>) -> Self {
                    value.into_value()
                }
            }

            impl<'js> From<$head<'js>> for Value<'js> {
                fn from(value: $head<'js>) -> Self {
                    value.into_value()
                }
            }

            impl<'js> FromJs<'js> for $head<'js> {
                fn from_js(_: &Ctx<'js>, value: Value<'js>) -> Result<Self> {
                    Self::from_value(value)
                }
            }

            impl<'js> IntoJs<'js> for $head<'js> {
                fn into_js(self, _ctx: &Ctx<'js>) -> Result<Value<'js>> {
                    Ok(self.into_value())
                }
            }

            impl<'js> IntoAtom<'js> for $head<'js>{
                fn into_atom(self, ctx: &Ctx<'js>) -> Result<Atom<'js>> {
                    Atom::from_value(ctx.clone(), &self.into_value())
                }
            }
        )*
    };

    (@type $type:ident) => { $type<'js> };

    (@head $head:ident $(rem:ident)*)  => { $head };
    (@head_ty $head:ident$(,$rem:ident)*)  => { $head<'js> };

    (@wrap $type:ident$(->$rem:ident)+ $val:expr) => { $type(sub_types!(@wrap $($rem)->* $val)) };
    (@wrap Value $val:expr) => { $val };

    /*
    (@into_inner $head:ident->$inner:ident$(->$rem:ident)*) => {
        impl<'js> $head<'js>{
            pub fn into_inner($
        }
    }*/

    (@imp_as_ref $type:ident,Value) => {
        impl<'js> AsRef<Value<'js>> for $type<'js> {
            fn as_ref(&self) -> &Value<'js> {
                &self.0
            }
        }
    };
    (@imp_as_ref $type:ident,$inner:ident$(,$rem:ident)*) => {
        impl<'js> AsRef<$inner<'js>> for $type<'js> {
            fn as_ref(&self) -> &$inner<'js> {
                &self.0
            }
        }

        impl<'js> AsRef<Value<'js>> for $type<'js> {
            fn as_ref(&self) -> &Value<'js> {
                self.0.as_ref()
            }
        }
    };
}

sub_types! {
    String->Value as_string ref_string into_string try_into_string from_string,
    Symbol->Value as_symbol ref_symbol into_symbol try_into_symbol from_symbol,
    Object->Value as_object ref_object into_object try_into_object from_object,
    Function->Object->Value as_function ref_function into_function try_into_function from_function,
    Constructor->Function->Object->Value as_constructor ref_constructor into_constructor try_into_constructor from_constructor,
    Array->Object->Value as_array ref_array into_array try_into_array from_array,
    Exception->Object->Value as_exception ref_exception into_exception try_into_exception from_exception,
    BigInt->Value as_big_int ref_big_int into_big_int try_into_big_int from_big_int,
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
                fn from_js(_: &Ctx<'js>, value: Value<'js>) -> Result<Self> {
                    Self::from_value(value)
                }
            }

            impl<'js> IntoJs<'js> for $type {
                fn into_js(self, ctx: &Ctx<'js>) -> Result<Value<'js>> {
                    Ok(self.into_value(ctx.clone()))
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
