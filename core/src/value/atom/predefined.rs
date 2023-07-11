use crate::qjs;

/// A collection of atoms which are predefined in quickjs.
///
/// Using these over [`Atom::from_str`] can be more performant as these don't need to be looked up
/// in a hashmap.
#[derive(Clone, Copy, Eq, PartialEq, Hash, Debug)]
#[repr(u32)]
pub enum PredefinedAtom {
    /// "null"
    Null = qjs::JS_ATOM_null, /* must be first */
    /// "false"
    False = qjs::JS_ATOM_false,
    /// "true"
    True = qjs::JS_ATOM_true,
    /// "if"
    If = qjs::JS_ATOM_if,
    /// "else"
    Else = qjs::JS_ATOM_else,
    /// "return"
    Return = qjs::JS_ATOM_return,
    /// "var"
    Var = qjs::JS_ATOM_var,
    /// "this"
    This = qjs::JS_ATOM_this,
    /// "delete"
    Delete = qjs::JS_ATOM_delete,
    /// "void"
    Void = qjs::JS_ATOM_void,
    /// "typeof"
    Typeof = qjs::JS_ATOM_typeof,
    /// "new"
    New = qjs::JS_ATOM_new,
    /// "in"
    In = qjs::JS_ATOM_in,
    /// "instanceof"
    Instanceof = qjs::JS_ATOM_instanceof,
    /// "do"
    Do = qjs::JS_ATOM_do,
    /// "while"
    While = qjs::JS_ATOM_while,
    /// "for"
    For = qjs::JS_ATOM_for,
    /// "break"
    Break = qjs::JS_ATOM_break,
    /// "continue"
    Continue = qjs::JS_ATOM_continue,
    /// "switch"
    Switch = qjs::JS_ATOM_switch,
    /// "case"
    Case = qjs::JS_ATOM_case,
    /// "default"
    Default = qjs::JS_ATOM_default,
    /// "throw"
    Throw = qjs::JS_ATOM_throw,
    /// "try"
    Try = qjs::JS_ATOM_try,
    /// "catch"
    Catch = qjs::JS_ATOM_catch,
    /// "finally"
    Finally = qjs::JS_ATOM_finally,
    /// "function"
    FunctionKw = qjs::JS_ATOM_function,
    /// "debugger"
    Debugger = qjs::JS_ATOM_debugger,
    /// "with"
    With = qjs::JS_ATOM_with,
    /// "class"
    Class = qjs::JS_ATOM_class,
    /// "const"
    Const = qjs::JS_ATOM_const,
    /// "enum"
    Enum = qjs::JS_ATOM_enum,
    /// "export"
    Export = qjs::JS_ATOM_export,
    /// "extends"
    Extends = qjs::JS_ATOM_extends,
    /// "import"
    Import = qjs::JS_ATOM_import,
    /// "super"
    Super = qjs::JS_ATOM_super,
    /// "implements"
    Implements = qjs::JS_ATOM_implements,
    /// "interface"
    Interface = qjs::JS_ATOM_interface,
    /// "let"
    Let = qjs::JS_ATOM_let,
    /// "package"
    Package = qjs::JS_ATOM_package,
    /// "private"
    Private = qjs::JS_ATOM_private,
    /// "protected"
    Protected = qjs::JS_ATOM_protected,
    /// "public"
    Public = qjs::JS_ATOM_public,
    /// "static"
    Static = qjs::JS_ATOM_static,
    /// "yield"
    Yield = qjs::JS_ATOM_yield,
    /// "await"
    Await = qjs::JS_ATOM_await,

    /// ""
    Empty = qjs::JS_ATOM_empty_string,
    /// "length"
    Length = qjs::JS_ATOM_length,
    /// "fileName"
    FileName = qjs::JS_ATOM_fileName,
    /// "lineNumber"
    LineNumber = qjs::JS_ATOM_lineNumber,
    /// "message"
    Message = qjs::JS_ATOM_message,
    /// "errors"
    Errors = qjs::JS_ATOM_errors,
    /// "stack"
    Stack = qjs::JS_ATOM_stack,
    /// "name"
    Name = qjs::JS_ATOM_name,
    /// "toString"
    ToString = qjs::JS_ATOM_toString,
    /// "toLocaleString"
    ToLocaleString = qjs::JS_ATOM_toLocaleString,
    /// "valueOf"
    ValueOf = qjs::JS_ATOM_valueOf,
    /// "eval"
    Eval = qjs::JS_ATOM_eval,
    /// "prototype"
    Prototype = qjs::JS_ATOM_prototype,
    /// "constructor"
    Constructor = qjs::JS_ATOM_constructor,
    /// "configurable"
    Configurable = qjs::JS_ATOM_configurable,
    /// "writable"
    Writable = qjs::JS_ATOM_writable,
    /// "enumerable"
    Enumerable = qjs::JS_ATOM_enumerable,
    /// "value"
    Value = qjs::JS_ATOM_value,
    /// "get"
    Getter = qjs::JS_ATOM_get,
    /// "set"
    Setter = qjs::JS_ATOM_set,
    /// "of"
    Of = qjs::JS_ATOM_of,
    /// "__proto__"
    UnderscoreProto = qjs::JS_ATOM___proto__,
    /// "undefined"
    Undefined = qjs::JS_ATOM_undefined,
    /// "number"
    NumberLower = qjs::JS_ATOM_number,
    /// "boolean"
    BooleanLower = qjs::JS_ATOM_boolean,
    /// "string"
    StringLower = qjs::JS_ATOM_string,
    /// "object"
    ObjectLower = qjs::JS_ATOM_object,
    /// "symbol"
    SymbolLower = qjs::JS_ATOM_symbol,
    /// "integer"
    Integer = qjs::JS_ATOM_integer,
    /// "unknown"
    Unknown = qjs::JS_ATOM_unknown,
    /// "arguments"
    ArgumentsLower = qjs::JS_ATOM_arguments,
    /// "callee"
    Callee = qjs::JS_ATOM_callee,
    /// "caller"
    Caller = qjs::JS_ATOM_caller,
    /// "lastIndex"
    LastIndex = qjs::JS_ATOM_lastIndex,
    /// "target"
    Target = qjs::JS_ATOM_target,
    /// "index"
    Index = qjs::JS_ATOM_index,
    /// "input"
    Input = qjs::JS_ATOM_input,
    /// "defineProperties"
    DefineProperties = qjs::JS_ATOM_defineProperties,
    /// "apply"
    Apply = qjs::JS_ATOM_apply,
    /// "join"
    Join = qjs::JS_ATOM_join,
    /// "concat"
    Concat = qjs::JS_ATOM_concat,
    /// "split"
    Split = qjs::JS_ATOM_split,
    /// "construct"
    Construct = qjs::JS_ATOM_construct,
    /// "getPrototypeOf"
    GetPrototypeOf = qjs::JS_ATOM_getPrototypeOf,
    /// "setPrototypeOf"
    SetPrototypeOf = qjs::JS_ATOM_setPrototypeOf,
    /// "isExtensible"
    IsExtensible = qjs::JS_ATOM_isExtensible,
    /// "preventExtensions"
    PreventExtensions = qjs::JS_ATOM_preventExtensions,
    /// "has"
    Has = qjs::JS_ATOM_has,
    /// "deleteProperty"
    DeleteProperty = qjs::JS_ATOM_deleteProperty,
    /// "defineProperty"
    DefineProperty = qjs::JS_ATOM_defineProperty,
    /// "getOwnPropertyDescriptor"
    GetOwnPropertyDescriptor = qjs::JS_ATOM_getOwnPropertyDescriptor,
    /// "ownKeys"
    OwnKeys = qjs::JS_ATOM_ownKeys,
    /// "add"
    Add = qjs::JS_ATOM_add,
    /// "done"
    Done = qjs::JS_ATOM_done,
    /// "next"
    Next = qjs::JS_ATOM_next,
    /// "values"
    Values = qjs::JS_ATOM_values,
    /// "source"
    Source = qjs::JS_ATOM_source,
    /// "flags"
    Flags = qjs::JS_ATOM_flags,
    /// "global"
    Global = qjs::JS_ATOM_global,
    /// "unicode"
    Unicode = qjs::JS_ATOM_unicode,
    /// "raw"
    Raw = qjs::JS_ATOM_raw,
    /// "new.target"
    NewTarget = qjs::JS_ATOM_new_target,
    /// "this.active_func"
    ThisActiveFunc = qjs::JS_ATOM_this_active_func,
    /// "<home_object>"
    HomeObject = qjs::JS_ATOM_home_object,
    /// "<computed_field>"
    ComputedField = qjs::JS_ATOM_computed_field,
    /// "<static_computed_field>"
    StaticComputedField = qjs::JS_ATOM_static_computed_field, /* must come after computed_fields */
    /// "<class_fields_init>"
    ClassFieldsInit = qjs::JS_ATOM_class_fields_init,
    /// "<brand>"
    Brand = qjs::JS_ATOM_brand,
    /// "#constructor"
    HashConstructor = qjs::JS_ATOM_hash_constructor,
    /// "as"
    As = qjs::JS_ATOM_as,
    /// "from"
    From = qjs::JS_ATOM_from,
    /// "meta"
    Meta = qjs::JS_ATOM_meta,
    /// "*default*"
    StarDefault = qjs::JS_ATOM__default_,
    /// "*"
    Star = qjs::JS_ATOM__star_,
    /// "Module"
    Module = qjs::JS_ATOM_Module,
    /// "then"
    Then = qjs::JS_ATOM_then,
    /// "resolve"
    Resolve = qjs::JS_ATOM_resolve,
    /// "reject"
    Reject = qjs::JS_ATOM_reject,
    /// "promise"
    PromiseLower = qjs::JS_ATOM_promise,
    /// "proxy"
    ProxyLower = qjs::JS_ATOM_proxy,
    /// "revoke"
    Revoke = qjs::JS_ATOM_revoke,
    /// "async"
    Async = qjs::JS_ATOM_async,
    /// "exec"
    Exec = qjs::JS_ATOM_exec,
    /// "groups"
    Groups = qjs::JS_ATOM_groups,
    /// "status"
    Status = qjs::JS_ATOM_status,
    /// "reason"
    Reason = qjs::JS_ATOM_reason,
    /// "globalThis"
    GlobalThis = qjs::JS_ATOM_globalThis,
    /// "bigint"
    Bigint = qjs::JS_ATOM_bigint,
    /// "bigfloat"
    Bigfloat = qjs::JS_ATOM_bigfloat,
    /// "bigdecimal"
    Bigdecimal = qjs::JS_ATOM_bigdecimal,
    /// "roundingMode"
    RoundingMode = qjs::JS_ATOM_roundingMode,
    /// "maximumSignificantDigits"
    MaximumSignificantDigits = qjs::JS_ATOM_maximumSignificantDigits,
    /// "maximumFractionDigits"
    MaximumFractionDigits = qjs::JS_ATOM_maximumFractionDigits,
    /// "toJSON"
    ToJSON = qjs::JS_ATOM_toJSON,
    /// "Object"
    Object = qjs::JS_ATOM_Object,
    /// "Array"
    Array = qjs::JS_ATOM_Array,
    /// "Error"
    Error = qjs::JS_ATOM_Error,
    /// "Number"
    Number = qjs::JS_ATOM_Number,
    /// "String"
    String = qjs::JS_ATOM_String,
    /// "Boolean"
    Boolean = qjs::JS_ATOM_Boolean,
    /// "Symbol"
    Symbol = qjs::JS_ATOM_Symbol,
    /// "Arguments"
    Arguments = qjs::JS_ATOM_Arguments,
    /// "Math"
    Math = qjs::JS_ATOM_Math,
    /// "JSON"
    JSON = qjs::JS_ATOM_JSON,
    /// "Date"
    Date = qjs::JS_ATOM_Date,
    /// "Function"
    Function = qjs::JS_ATOM_Function,
    /// "GeneratorFunction"
    GeneratorFunction = qjs::JS_ATOM_GeneratorFunction,
    /// "ForInIterator"
    ForInIterator = qjs::JS_ATOM_ForInIterator,
    /// "RegExp"
    RegExp = qjs::JS_ATOM_RegExp,
    /// "ArrayBuffer"
    ArrayBuffer = qjs::JS_ATOM_ArrayBuffer,
    /// "SharedArrayBuffer"
    SharedArrayBuffer = qjs::JS_ATOM_SharedArrayBuffer,
    /* must keep same order as class IDs for typed arrays */
    /// "Uint8ClampedArray"
    Uint8ClampedArray = qjs::JS_ATOM_Uint8ClampedArray,
    /// "Int8Array"
    Int8Array = qjs::JS_ATOM_Int8Array,
    /// "Uint8Array"
    Uint8Array = qjs::JS_ATOM_Uint8Array,
    /// "Int16Array"
    Int16Array = qjs::JS_ATOM_Int16Array,
    /// "Uint16Array"
    Uint16Array = qjs::JS_ATOM_Uint16Array,
    /// "Int32Array"
    Int32Array = qjs::JS_ATOM_Int32Array,
    /// "Uint32Array"
    Uint32Array = qjs::JS_ATOM_Uint32Array,
    /// "BigInt64Array"
    BigInt64Array = qjs::JS_ATOM_BigInt64Array,
    /// "BigUint64Array"
    BigUint64Array = qjs::JS_ATOM_BigUint64Array,
    /// "Float32Array"
    Float32Array = qjs::JS_ATOM_Float32Array,
    /// "Float64Array"
    Float64Array = qjs::JS_ATOM_Float64Array,
    /// "DataView"
    DataView = qjs::JS_ATOM_DataView,
    /// "BigInt"
    BigInt = qjs::JS_ATOM_BigInt,
    /// "BigFloat"
    BigFloat = qjs::JS_ATOM_BigFloat,
    /// "BigFloatEnv"
    BigFloatEnv = qjs::JS_ATOM_BigFloatEnv,
    /// "BigDecimal"
    BigDecimal = qjs::JS_ATOM_BigDecimal,
    /// "OperatorSet"
    OperatorSet = qjs::JS_ATOM_OperatorSet,
    /// "Operators"
    Operators = qjs::JS_ATOM_Operators,
    /// "Map"
    Map = qjs::JS_ATOM_Map,
    /// "Set"
    Set = qjs::JS_ATOM_Set, /* Map + 1 */
    /// "WeakMap"
    WeakMap = qjs::JS_ATOM_WeakMap, /* Map + 2 */
    /// "WeakSet"
    WeakSet = qjs::JS_ATOM_WeakSet, /* Map + 3 */
    /// "Map Iterator"
    MapIterator = qjs::JS_ATOM_Map_Iterator,
    /// "Set Iterator"
    SetIterator = qjs::JS_ATOM_Set_Iterator,
    /// "Array Iterator"
    ArrayIterator = qjs::JS_ATOM_Array_Iterator,
    /// "String Iterator"
    StringIterator = qjs::JS_ATOM_String_Iterator,
    /// "RegExp String Iterator"
    RegExpStringIterator = qjs::JS_ATOM_RegExp_String_Iterator,
    /// "Generator"
    Generator = qjs::JS_ATOM_Generator,
    /// "Proxy"
    Proxy = qjs::JS_ATOM_Proxy,
    /// "Promise"
    Promise = qjs::JS_ATOM_Promise,
    /// "PromiseResolveFunction"
    PromiseResolveFunction = qjs::JS_ATOM_PromiseResolveFunction,
    /// "PromiseRejectFunction"
    PromiseRejectFunction = qjs::JS_ATOM_PromiseRejectFunction,
    /// "AsyncFunction"
    AsyncFunction = qjs::JS_ATOM_AsyncFunction,
    /// "AsyncFunctionResolve"
    AsyncFunctionResolve = qjs::JS_ATOM_AsyncFunctionResolve,
    /// "AsyncFunctionReject"
    AsyncFunctionReject = qjs::JS_ATOM_AsyncFunctionReject,
    /// "AsyncGeneratorFunction"
    AsyncGeneratorFunction = qjs::JS_ATOM_AsyncGeneratorFunction,
    /// "AsyncGenerator"
    AsyncGenerator = qjs::JS_ATOM_AsyncGenerator,
    /// "EvalError"
    EvalError = qjs::JS_ATOM_EvalError,
    /// "RangeError"
    RangeError = qjs::JS_ATOM_RangeError,
    /// "ReferenceError"
    ReferenceError = qjs::JS_ATOM_ReferenceError,
    /// "SyntaxError"
    SyntaxError = qjs::JS_ATOM_SyntaxError,
    /// "TypeError"
    TypeError = qjs::JS_ATOM_TypeError,
    /// "URIError"
    URIError = qjs::JS_ATOM_URIError,
    /// "InternalError"
    InternalError = qjs::JS_ATOM_InternalError,
    /// "Symbol.iterator"
    SymbolIterator = qjs::JS_ATOM_Symbol_iterator,
    /// "Symbol.match"
    SymbolMatch = qjs::JS_ATOM_Symbol_match,
    /// "Symbol.matchAll"
    SymbolMatchAll = qjs::JS_ATOM_Symbol_matchAll,
    /// "Symbol.replace"
    SymbolReplace = qjs::JS_ATOM_Symbol_replace,
    /// "Symbol.search"
    SymbolSearch = qjs::JS_ATOM_Symbol_search,
    /// "Symbol.split"
    SymbolSplit = qjs::JS_ATOM_Symbol_split,
    /// "Symbol.toStringTag"
    SymbolToStringTag = qjs::JS_ATOM_Symbol_toStringTag,
    /// "Symbol.isConcatSpreadable"
    SymbolIsConcatSpreadable = qjs::JS_ATOM_Symbol_isConcatSpreadable,
    /// "Symbol.hasInstance"
    SymbolHasInstance = qjs::JS_ATOM_Symbol_hasInstance,
    /// "Symbol.species"
    SymbolSpecies = qjs::JS_ATOM_Symbol_species,
    /// "Symbol.unscopables"
    SymbolUnscopables = qjs::JS_ATOM_Symbol_unscopables,
}

impl PredefinedAtom {
    pub const fn is_symbol(self) -> bool {
        matches!(
            self,
            PredefinedAtom::SymbolIterator
                | PredefinedAtom::SymbolMatch
                | PredefinedAtom::SymbolMatchAll
                | PredefinedAtom::SymbolReplace
                | PredefinedAtom::SymbolSearch
                | PredefinedAtom::SymbolSplit
                | PredefinedAtom::SymbolToStringTag
                | PredefinedAtom::SymbolIsConcatSpreadable
                | PredefinedAtom::SymbolHasInstance
                | PredefinedAtom::SymbolSpecies
                | PredefinedAtom::SymbolUnscopables
        )
    }

    pub const fn to_str(self) -> &'static str {
        match self {
            PredefinedAtom::Null => "null",
            PredefinedAtom::False => "false",
            PredefinedAtom::True => "true",
            PredefinedAtom::If => "if",
            PredefinedAtom::Else => "else",
            PredefinedAtom::Return => "return",
            PredefinedAtom::Var => "var",
            PredefinedAtom::This => "this",
            PredefinedAtom::Delete => "delete",
            PredefinedAtom::Void => "void",
            PredefinedAtom::Typeof => "typeof",
            PredefinedAtom::New => "new",
            PredefinedAtom::In => "in",
            PredefinedAtom::Instanceof => "instanceof",
            PredefinedAtom::Do => "do",
            PredefinedAtom::While => "while",
            PredefinedAtom::For => "for",
            PredefinedAtom::Break => "break",
            PredefinedAtom::Continue => "continue",
            PredefinedAtom::Switch => "switch",
            PredefinedAtom::Case => "case",
            PredefinedAtom::Default => "default",
            PredefinedAtom::Throw => "throw",
            PredefinedAtom::Try => "try",
            PredefinedAtom::Catch => "catch",
            PredefinedAtom::Finally => "finally",
            PredefinedAtom::FunctionKw => "function",
            PredefinedAtom::Debugger => "debugger",
            PredefinedAtom::With => "with",
            PredefinedAtom::Class => "class",
            PredefinedAtom::Const => "const",
            PredefinedAtom::Enum => "enum",
            PredefinedAtom::Export => "export",
            PredefinedAtom::Extends => "extends",
            PredefinedAtom::Import => "import",
            PredefinedAtom::Super => "super",
            PredefinedAtom::Implements => "implements",
            PredefinedAtom::Interface => "interface",
            PredefinedAtom::Let => "let",
            PredefinedAtom::Package => "package",
            PredefinedAtom::Private => "private",
            PredefinedAtom::Protected => "protected",
            PredefinedAtom::Public => "public",
            PredefinedAtom::Static => "static",
            PredefinedAtom::Yield => "yield",
            PredefinedAtom::Await => "await",
            PredefinedAtom::Empty => "",
            PredefinedAtom::Length => "length",
            PredefinedAtom::FileName => "fileName",
            PredefinedAtom::LineNumber => "lineNumber",
            PredefinedAtom::Message => "message",
            PredefinedAtom::Errors => "errors",
            PredefinedAtom::Stack => "stack",
            PredefinedAtom::Name => "name",
            PredefinedAtom::ToString => "toString",
            PredefinedAtom::ToLocaleString => "toLocaleString",
            PredefinedAtom::ValueOf => "valueOf",
            PredefinedAtom::Eval => "eval",
            PredefinedAtom::Prototype => "prototype",
            PredefinedAtom::Constructor => "constructor",
            PredefinedAtom::Configurable => "configurable",
            PredefinedAtom::Writable => "writable",
            PredefinedAtom::Enumerable => "enumerable",
            PredefinedAtom::Value => "value",
            PredefinedAtom::Getter => "get",
            PredefinedAtom::Setter => "set",
            PredefinedAtom::Of => "of",
            PredefinedAtom::UnderscoreProto => "__proto__",
            PredefinedAtom::Undefined => "undefined",
            PredefinedAtom::NumberLower => "number",
            PredefinedAtom::BooleanLower => "boolean",
            PredefinedAtom::StringLower => "string",
            PredefinedAtom::ObjectLower => "object",
            PredefinedAtom::SymbolLower => "symbol",
            PredefinedAtom::Integer => "integer",
            PredefinedAtom::Unknown => "unknown",
            PredefinedAtom::ArgumentsLower => "arguments",
            PredefinedAtom::Callee => "callee",
            PredefinedAtom::Caller => "caller",
            PredefinedAtom::LastIndex => "lastIndex",
            PredefinedAtom::Target => "target",
            PredefinedAtom::Index => "index",
            PredefinedAtom::Input => "input",
            PredefinedAtom::DefineProperties => "defineProperties",
            PredefinedAtom::Apply => "apply",
            PredefinedAtom::Join => "join",
            PredefinedAtom::Concat => "concat",
            PredefinedAtom::Split => "split",
            PredefinedAtom::Construct => "construct",
            PredefinedAtom::GetPrototypeOf => "getPrototypeOf",
            PredefinedAtom::SetPrototypeOf => "setPrototypeOf",
            PredefinedAtom::IsExtensible => "isExtensible",
            PredefinedAtom::PreventExtensions => "preventExtensions",
            PredefinedAtom::Has => "has",
            PredefinedAtom::DeleteProperty => "deleteProperty",
            PredefinedAtom::DefineProperty => "defineProperty",
            PredefinedAtom::GetOwnPropertyDescriptor => "getOwnPropertyDescriptor",
            PredefinedAtom::OwnKeys => "ownKeys",
            PredefinedAtom::Add => "add",
            PredefinedAtom::Done => "done",
            PredefinedAtom::Next => "next",
            PredefinedAtom::Values => "values",
            PredefinedAtom::Source => "source",
            PredefinedAtom::Flags => "flags",
            PredefinedAtom::Global => "global",
            PredefinedAtom::Unicode => "unicode",
            PredefinedAtom::Raw => "raw",
            PredefinedAtom::NewTarget => "new.target",
            PredefinedAtom::ThisActiveFunc => "this.active_func",
            PredefinedAtom::HomeObject => "<home_object>",
            PredefinedAtom::ComputedField => "<computed_field>",
            PredefinedAtom::StaticComputedField => "<static_computed_field>",
            PredefinedAtom::ClassFieldsInit => "<class_fields_init>",
            PredefinedAtom::Brand => "<brand>",
            PredefinedAtom::HashConstructor => "#constructor",
            PredefinedAtom::As => "as",
            PredefinedAtom::From => "from",
            PredefinedAtom::Meta => "meta",
            PredefinedAtom::StarDefault => "*default*",
            PredefinedAtom::Star => "*",
            PredefinedAtom::Module => "Module",
            PredefinedAtom::Then => "then",
            PredefinedAtom::Resolve => "resolve",
            PredefinedAtom::Reject => "reject",
            PredefinedAtom::PromiseLower => "promise",
            PredefinedAtom::ProxyLower => "proxy",
            PredefinedAtom::Revoke => "revoke",
            PredefinedAtom::Async => "async",
            PredefinedAtom::Exec => "exec",
            PredefinedAtom::Groups => "groups",
            PredefinedAtom::Status => "status",
            PredefinedAtom::Reason => "reason",
            PredefinedAtom::GlobalThis => "globalThis",
            PredefinedAtom::Bigint => "bigint",
            PredefinedAtom::Bigfloat => "bigfloat",
            PredefinedAtom::Bigdecimal => "bigdecimal",
            PredefinedAtom::RoundingMode => "roundingMode",
            PredefinedAtom::MaximumSignificantDigits => "maximumSignificantDigits",
            PredefinedAtom::MaximumFractionDigits => "maximumFractionDigits",
            PredefinedAtom::ToJSON => "toJSON",
            PredefinedAtom::Object => "Object",
            PredefinedAtom::Array => "Array",
            PredefinedAtom::Error => "Error",
            PredefinedAtom::Number => "Number",
            PredefinedAtom::String => "String",
            PredefinedAtom::Boolean => "Boolean",
            PredefinedAtom::Symbol => "Symbol",
            PredefinedAtom::Arguments => "Arguments",
            PredefinedAtom::Math => "Math",
            PredefinedAtom::JSON => "JSON",
            PredefinedAtom::Date => "Date",
            PredefinedAtom::Function => "Function",
            PredefinedAtom::GeneratorFunction => "GeneratorFunction",
            PredefinedAtom::ForInIterator => "ForInIterator",
            PredefinedAtom::RegExp => "RegExp",
            PredefinedAtom::ArrayBuffer => "ArrayBuffer",
            PredefinedAtom::SharedArrayBuffer => "SharedArrayBuffer",
            PredefinedAtom::Uint8ClampedArray => "Uint8ClampedArray",
            PredefinedAtom::Int8Array => "Int8Array",
            PredefinedAtom::Uint8Array => "Uint8Array",
            PredefinedAtom::Int16Array => "Int16Array",
            PredefinedAtom::Uint16Array => "Uint16Array",
            PredefinedAtom::Int32Array => "Int32Array",
            PredefinedAtom::Uint32Array => "Uint32Array",
            PredefinedAtom::BigInt64Array => "BigInt64Array",
            PredefinedAtom::BigUint64Array => "BigUint64Array",
            PredefinedAtom::Float32Array => "Float32Array",
            PredefinedAtom::Float64Array => "Float64Array",
            PredefinedAtom::DataView => "DataView",
            PredefinedAtom::BigInt => "BigInt",
            PredefinedAtom::BigFloat => "BigFloat",
            PredefinedAtom::BigFloatEnv => "BigFloatEnv",
            PredefinedAtom::BigDecimal => "BigDecimal",
            PredefinedAtom::OperatorSet => "OperatorSet",
            PredefinedAtom::Operators => "Operators",
            PredefinedAtom::Map => "Map",
            PredefinedAtom::Set => "Set",
            PredefinedAtom::WeakMap => "WeakMap",
            PredefinedAtom::WeakSet => "WeakSet",
            PredefinedAtom::MapIterator => "Map Iterator",
            PredefinedAtom::SetIterator => "Set Iterator",
            PredefinedAtom::ArrayIterator => "Array Iterator",
            PredefinedAtom::StringIterator => "String Iterator",
            PredefinedAtom::RegExpStringIterator => "RegExp String Iterator",
            PredefinedAtom::Generator => "Generator",
            PredefinedAtom::Proxy => "Proxy",
            PredefinedAtom::Promise => "Promise",
            PredefinedAtom::PromiseResolveFunction => "PromiseResolveFunction",
            PredefinedAtom::PromiseRejectFunction => "PromiseRejectFunction",
            PredefinedAtom::AsyncFunction => "AsyncFunction",
            PredefinedAtom::AsyncFunctionResolve => "AsyncFunctionResolve",
            PredefinedAtom::AsyncFunctionReject => "AsyncFunctionReject",
            PredefinedAtom::AsyncGeneratorFunction => "AsyncGeneratorFunction",
            PredefinedAtom::AsyncGenerator => "AsyncGenerator",
            PredefinedAtom::EvalError => "EvalError",
            PredefinedAtom::RangeError => "RangeError",
            PredefinedAtom::ReferenceError => "ReferenceError",
            PredefinedAtom::SyntaxError => "SyntaxError",
            PredefinedAtom::TypeError => "TypeError",
            PredefinedAtom::URIError => "URIError",
            PredefinedAtom::InternalError => "InternalError",
            PredefinedAtom::SymbolIterator => "Symbol.iterator",
            PredefinedAtom::SymbolMatch => "Symbol.match",
            PredefinedAtom::SymbolMatchAll => "Symbol.matchAll",
            PredefinedAtom::SymbolReplace => "Symbol.replace",
            PredefinedAtom::SymbolSearch => "Symbol.search",
            PredefinedAtom::SymbolSplit => "Symbol.split",
            PredefinedAtom::SymbolToStringTag => "Symbol.toStringTag",
            PredefinedAtom::SymbolIsConcatSpreadable => "Symbol.isConcatSpreadable",
            PredefinedAtom::SymbolHasInstance => "Symbol.hasInstance",
            PredefinedAtom::SymbolSpecies => "Symbol.species",
            PredefinedAtom::SymbolUnscopables => "Symbol.unscopables",
        }
    }
}

#[cfg(test)]
mod test {
    use crate::{Atom, Context, IntoAtom, Runtime};

    use super::PredefinedAtom;
    #[test]
    fn string_correct() {
        static ALL_PREDEFS: &[PredefinedAtom] = &[
            PredefinedAtom::Null,
            PredefinedAtom::False,
            PredefinedAtom::True,
            PredefinedAtom::If,
            PredefinedAtom::Else,
            PredefinedAtom::Return,
            PredefinedAtom::Var,
            PredefinedAtom::This,
            PredefinedAtom::Delete,
            PredefinedAtom::Void,
            PredefinedAtom::Typeof,
            PredefinedAtom::New,
            PredefinedAtom::In,
            PredefinedAtom::Instanceof,
            PredefinedAtom::Do,
            PredefinedAtom::While,
            PredefinedAtom::For,
            PredefinedAtom::Break,
            PredefinedAtom::Continue,
            PredefinedAtom::Switch,
            PredefinedAtom::Case,
            PredefinedAtom::Default,
            PredefinedAtom::Throw,
            PredefinedAtom::Try,
            PredefinedAtom::Catch,
            PredefinedAtom::Finally,
            PredefinedAtom::FunctionKw,
            PredefinedAtom::Debugger,
            PredefinedAtom::With,
            PredefinedAtom::Class,
            PredefinedAtom::Const,
            PredefinedAtom::Enum,
            PredefinedAtom::Export,
            PredefinedAtom::Extends,
            PredefinedAtom::Import,
            PredefinedAtom::Super,
            PredefinedAtom::Implements,
            PredefinedAtom::Interface,
            PredefinedAtom::Let,
            PredefinedAtom::Package,
            PredefinedAtom::Private,
            PredefinedAtom::Protected,
            PredefinedAtom::Public,
            PredefinedAtom::Static,
            PredefinedAtom::Yield,
            PredefinedAtom::Await,
            PredefinedAtom::Empty,
            PredefinedAtom::Length,
            PredefinedAtom::FileName,
            PredefinedAtom::LineNumber,
            PredefinedAtom::Message,
            PredefinedAtom::Errors,
            PredefinedAtom::Stack,
            PredefinedAtom::Name,
            PredefinedAtom::ToString,
            PredefinedAtom::ToLocaleString,
            PredefinedAtom::ValueOf,
            PredefinedAtom::Eval,
            PredefinedAtom::Prototype,
            PredefinedAtom::Constructor,
            PredefinedAtom::Configurable,
            PredefinedAtom::Writable,
            PredefinedAtom::Enumerable,
            PredefinedAtom::Value,
            PredefinedAtom::Getter,
            PredefinedAtom::Setter,
            PredefinedAtom::Of,
            PredefinedAtom::UnderscoreProto,
            PredefinedAtom::Undefined,
            PredefinedAtom::NumberLower,
            PredefinedAtom::BooleanLower,
            PredefinedAtom::StringLower,
            PredefinedAtom::ObjectLower,
            PredefinedAtom::SymbolLower,
            PredefinedAtom::Integer,
            PredefinedAtom::Unknown,
            PredefinedAtom::ArgumentsLower,
            PredefinedAtom::Callee,
            PredefinedAtom::Caller,
            PredefinedAtom::LastIndex,
            PredefinedAtom::Target,
            PredefinedAtom::Index,
            PredefinedAtom::Input,
            PredefinedAtom::DefineProperties,
            PredefinedAtom::Apply,
            PredefinedAtom::Join,
            PredefinedAtom::Concat,
            PredefinedAtom::Split,
            PredefinedAtom::Construct,
            PredefinedAtom::GetPrototypeOf,
            PredefinedAtom::SetPrototypeOf,
            PredefinedAtom::IsExtensible,
            PredefinedAtom::PreventExtensions,
            PredefinedAtom::Has,
            PredefinedAtom::DeleteProperty,
            PredefinedAtom::DefineProperty,
            PredefinedAtom::GetOwnPropertyDescriptor,
            PredefinedAtom::OwnKeys,
            PredefinedAtom::Add,
            PredefinedAtom::Done,
            PredefinedAtom::Next,
            PredefinedAtom::Values,
            PredefinedAtom::Source,
            PredefinedAtom::Flags,
            PredefinedAtom::Global,
            PredefinedAtom::Unicode,
            PredefinedAtom::Raw,
            PredefinedAtom::NewTarget,
            PredefinedAtom::ThisActiveFunc,
            PredefinedAtom::HomeObject,
            PredefinedAtom::ComputedField,
            PredefinedAtom::StaticComputedField,
            PredefinedAtom::ClassFieldsInit,
            PredefinedAtom::Brand,
            PredefinedAtom::HashConstructor,
            PredefinedAtom::As,
            PredefinedAtom::From,
            PredefinedAtom::Meta,
            PredefinedAtom::StarDefault,
            PredefinedAtom::Star,
            PredefinedAtom::Module,
            PredefinedAtom::Then,
            PredefinedAtom::Resolve,
            PredefinedAtom::Reject,
            PredefinedAtom::PromiseLower,
            PredefinedAtom::ProxyLower,
            PredefinedAtom::Revoke,
            PredefinedAtom::Async,
            PredefinedAtom::Exec,
            PredefinedAtom::Groups,
            PredefinedAtom::Status,
            PredefinedAtom::Reason,
            PredefinedAtom::GlobalThis,
            PredefinedAtom::Bigint,
            PredefinedAtom::Bigfloat,
            PredefinedAtom::Bigdecimal,
            PredefinedAtom::RoundingMode,
            PredefinedAtom::MaximumSignificantDigits,
            PredefinedAtom::MaximumFractionDigits,
            PredefinedAtom::ToJSON,
            PredefinedAtom::Object,
            PredefinedAtom::Array,
            PredefinedAtom::Error,
            PredefinedAtom::Number,
            PredefinedAtom::String,
            PredefinedAtom::Boolean,
            PredefinedAtom::Symbol,
            PredefinedAtom::Arguments,
            PredefinedAtom::Math,
            PredefinedAtom::JSON,
            PredefinedAtom::Date,
            PredefinedAtom::Function,
            PredefinedAtom::GeneratorFunction,
            PredefinedAtom::ForInIterator,
            PredefinedAtom::RegExp,
            PredefinedAtom::ArrayBuffer,
            PredefinedAtom::SharedArrayBuffer,
            PredefinedAtom::Uint8ClampedArray,
            PredefinedAtom::Int8Array,
            PredefinedAtom::Uint8Array,
            PredefinedAtom::Int16Array,
            PredefinedAtom::Uint16Array,
            PredefinedAtom::Int32Array,
            PredefinedAtom::Uint32Array,
            PredefinedAtom::BigInt64Array,
            PredefinedAtom::BigUint64Array,
            PredefinedAtom::Float32Array,
            PredefinedAtom::Float64Array,
            PredefinedAtom::DataView,
            PredefinedAtom::BigInt,
            PredefinedAtom::BigFloat,
            PredefinedAtom::BigFloatEnv,
            PredefinedAtom::BigDecimal,
            PredefinedAtom::OperatorSet,
            PredefinedAtom::Operators,
            PredefinedAtom::Map,
            PredefinedAtom::Set,
            PredefinedAtom::WeakMap,
            PredefinedAtom::WeakSet,
            PredefinedAtom::MapIterator,
            PredefinedAtom::SetIterator,
            PredefinedAtom::ArrayIterator,
            PredefinedAtom::StringIterator,
            PredefinedAtom::RegExpStringIterator,
            PredefinedAtom::Generator,
            PredefinedAtom::Proxy,
            PredefinedAtom::Promise,
            PredefinedAtom::PromiseResolveFunction,
            PredefinedAtom::PromiseRejectFunction,
            PredefinedAtom::AsyncFunction,
            PredefinedAtom::AsyncFunctionResolve,
            PredefinedAtom::AsyncFunctionReject,
            PredefinedAtom::AsyncGeneratorFunction,
            PredefinedAtom::AsyncGenerator,
            PredefinedAtom::EvalError,
            PredefinedAtom::RangeError,
            PredefinedAtom::ReferenceError,
            PredefinedAtom::SyntaxError,
            PredefinedAtom::TypeError,
            PredefinedAtom::URIError,
            PredefinedAtom::InternalError,
            PredefinedAtom::SymbolIterator,
            PredefinedAtom::SymbolMatch,
            PredefinedAtom::SymbolMatchAll,
            PredefinedAtom::SymbolReplace,
            PredefinedAtom::SymbolSearch,
            PredefinedAtom::SymbolSplit,
            PredefinedAtom::SymbolToStringTag,
            PredefinedAtom::SymbolIsConcatSpreadable,
            PredefinedAtom::SymbolHasInstance,
            PredefinedAtom::SymbolSpecies,
            PredefinedAtom::SymbolUnscopables,
        ];

        let rt = Runtime::new().unwrap();
        let context = Context::full(&rt).unwrap();
        context.with(|ctx| {
            for predef in ALL_PREDEFS {
                let atom = predef.into_atom(ctx).unwrap();
                assert_eq!(atom.to_string().unwrap().as_str(), predef.to_str());

                // the string of a symbol doesn't convert to the same atom.
                if predef.is_symbol() {
                    continue;
                }

                let from_str = Atom::from_str(ctx, predef.to_str()).unwrap();
                assert_eq!(
                    atom,
                    from_str,
                    "Atom `{}` from str and from redefined not equal",
                    predef.to_str()
                )
            }
        })
    }
}
