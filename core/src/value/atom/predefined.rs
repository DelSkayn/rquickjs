use crate::qjs;

/// A collection of atoms which are predefined in quickjs.
///
/// Using these over [`Atom::from_str`](crate::Atom::from_str) can be more performant as these don't need to be looked up
/// in a hashmap.
#[derive(Clone, Copy, Eq, PartialEq, Hash, Debug)]
#[repr(u32)]
#[allow(clippy::unnecessary_cast)]
pub enum PredefinedAtom {
    /// "null"
    Null = qjs::JS_ATOM_null as u32, // must be first
    /// "false"
    False = qjs::JS_ATOM_false as u32,
    /// "true"
    True = qjs::JS_ATOM_true as u32,
    /// "if"
    If = qjs::JS_ATOM_if as u32,
    /// "else"
    Else = qjs::JS_ATOM_else as u32,
    /// "return"
    Return = qjs::JS_ATOM_return as u32,
    /// "var"
    Var = qjs::JS_ATOM_var as u32,
    /// "this"
    This = qjs::JS_ATOM_this as u32,
    /// "delete"
    Delete = qjs::JS_ATOM_delete as u32,
    /// "void"
    Void = qjs::JS_ATOM_void as u32,
    /// "typeof"
    Typeof = qjs::JS_ATOM_typeof as u32,
    /// "new"
    New = qjs::JS_ATOM_new as u32,
    /// "in"
    In = qjs::JS_ATOM_in as u32,
    /// "instanceof"
    Instanceof = qjs::JS_ATOM_instanceof as u32,
    /// "do"
    Do = qjs::JS_ATOM_do as u32,
    /// "while"
    While = qjs::JS_ATOM_while as u32,
    /// "for"
    For = qjs::JS_ATOM_for as u32,
    /// "break"
    Break = qjs::JS_ATOM_break as u32,
    /// "continue"
    Continue = qjs::JS_ATOM_continue as u32,
    /// "switch"
    Switch = qjs::JS_ATOM_switch as u32,
    /// "case"
    Case = qjs::JS_ATOM_case as u32,
    /// "default"
    Default = qjs::JS_ATOM_default as u32,
    /// "throw"
    Throw = qjs::JS_ATOM_throw as u32,
    /// "try"
    Try = qjs::JS_ATOM_try as u32,
    /// "catch"
    Catch = qjs::JS_ATOM_catch as u32,
    /// "finally"
    Finally = qjs::JS_ATOM_finally as u32,
    /// "function"
    FunctionKw = qjs::JS_ATOM_function as u32,
    /// "debugger"
    Debugger = qjs::JS_ATOM_debugger as u32,
    /// "with"
    With = qjs::JS_ATOM_with as u32,
    /// "class"
    Class = qjs::JS_ATOM_class as u32,
    /// "const"
    Const = qjs::JS_ATOM_const as u32,
    /// "enum"
    Enum = qjs::JS_ATOM_enum as u32,
    /// "export"
    Export = qjs::JS_ATOM_export as u32,
    /// "extends"
    Extends = qjs::JS_ATOM_extends as u32,
    /// "import"
    Import = qjs::JS_ATOM_import as u32,
    /// "super"
    Super = qjs::JS_ATOM_super as u32,
    /// "implements"
    Implements = qjs::JS_ATOM_implements as u32,
    /// "interface"
    Interface = qjs::JS_ATOM_interface as u32,
    /// "let"
    Let = qjs::JS_ATOM_let as u32,
    /// "package"
    Package = qjs::JS_ATOM_package as u32,
    /// "private"
    Private = qjs::JS_ATOM_private as u32,
    /// "protected"
    Protected = qjs::JS_ATOM_protected as u32,
    /// "public"
    Public = qjs::JS_ATOM_public as u32,
    /// "static"
    Static = qjs::JS_ATOM_static as u32,
    /// "yield"
    Yield = qjs::JS_ATOM_yield as u32,
    /// "await"
    Await = qjs::JS_ATOM_await as u32,

    /// ""
    Empty = qjs::JS_ATOM_empty_string as u32,
    /// "length"
    Length = qjs::JS_ATOM_length as u32,
    /// "fileName"
    FileName = qjs::JS_ATOM_fileName as u32,
    /// "lineNumber"
    LineNumber = qjs::JS_ATOM_lineNumber as u32,
    /// "columnNumber
    ColumnNumber = qjs::JS_ATOM_columnNumber as u32,
    /// "message"
    Message = qjs::JS_ATOM_message as u32,
    /// "errors"
    Errors = qjs::JS_ATOM_errors as u32,
    /// "stack"
    Stack = qjs::JS_ATOM_stack as u32,
    /// "name"
    Name = qjs::JS_ATOM_name as u32,
    /// "toString"
    ToString = qjs::JS_ATOM_toString as u32,
    /// "toLocaleString"
    ToLocaleString = qjs::JS_ATOM_toLocaleString as u32,
    /// "valueOf"
    ValueOf = qjs::JS_ATOM_valueOf as u32,
    /// "eval"
    Eval = qjs::JS_ATOM_eval as u32,
    /// "prototype"
    Prototype = qjs::JS_ATOM_prototype as u32,
    /// "constructor"
    Constructor = qjs::JS_ATOM_constructor as u32,
    /// "configurable"
    Configurable = qjs::JS_ATOM_configurable as u32,
    /// "writable"
    Writable = qjs::JS_ATOM_writable as u32,
    /// "enumerable"
    Enumerable = qjs::JS_ATOM_enumerable as u32,
    /// "value"
    Value = qjs::JS_ATOM_value as u32,
    /// "get"
    Getter = qjs::JS_ATOM_get as u32,
    /// "set"
    Setter = qjs::JS_ATOM_set as u32,
    /// "of"
    Of = qjs::JS_ATOM_of as u32,
    /// "__proto__"
    UnderscoreProto = qjs::JS_ATOM___proto__ as u32,
    /// "undefined"
    Undefined = qjs::JS_ATOM_undefined as u32,
    /// "number"
    NumberLower = qjs::JS_ATOM_number as u32,
    /// "boolean"
    BooleanLower = qjs::JS_ATOM_boolean as u32,
    /// "string"
    StringLower = qjs::JS_ATOM_string as u32,
    /// "object"
    ObjectLower = qjs::JS_ATOM_object as u32,
    /// "symbol"
    SymbolLower = qjs::JS_ATOM_symbol as u32,
    /// "integer"
    Integer = qjs::JS_ATOM_integer as u32,
    /// "unknown"
    Unknown = qjs::JS_ATOM_unknown as u32,
    /// "arguments"
    ArgumentsLower = qjs::JS_ATOM_arguments as u32,
    /// "callee"
    Callee = qjs::JS_ATOM_callee as u32,
    /// "caller"
    Caller = qjs::JS_ATOM_caller as u32,
    /// "lastIndex"
    LastIndex = qjs::JS_ATOM_lastIndex as u32,
    /// "target"
    Target = qjs::JS_ATOM_target as u32,
    /// "index"
    Index = qjs::JS_ATOM_index as u32,
    /// "input"
    Input = qjs::JS_ATOM_input as u32,
    /// "defineProperties"
    DefineProperties = qjs::JS_ATOM_defineProperties as u32,
    /// "apply"
    Apply = qjs::JS_ATOM_apply as u32,
    /// "join"
    Join = qjs::JS_ATOM_join as u32,
    /// "concat"
    Concat = qjs::JS_ATOM_concat as u32,
    /// "split"
    Split = qjs::JS_ATOM_split as u32,
    /// "construct"
    Construct = qjs::JS_ATOM_construct as u32,
    /// "getPrototypeOf"
    GetPrototypeOf = qjs::JS_ATOM_getPrototypeOf as u32,
    /// "setPrototypeOf"
    SetPrototypeOf = qjs::JS_ATOM_setPrototypeOf as u32,
    /// "isExtensible"
    IsExtensible = qjs::JS_ATOM_isExtensible as u32,
    /// "preventExtensions"
    PreventExtensions = qjs::JS_ATOM_preventExtensions as u32,
    /// "has"
    Has = qjs::JS_ATOM_has as u32,
    /// "deleteProperty"
    DeleteProperty = qjs::JS_ATOM_deleteProperty as u32,
    /// "defineProperty"
    DefineProperty = qjs::JS_ATOM_defineProperty as u32,
    /// "getOwnPropertyDescriptor"
    GetOwnPropertyDescriptor = qjs::JS_ATOM_getOwnPropertyDescriptor as u32,
    /// "ownKeys"
    OwnKeys = qjs::JS_ATOM_ownKeys as u32,
    /// "add"
    Add = qjs::JS_ATOM_add as u32,
    /// "done"
    Done = qjs::JS_ATOM_done as u32,
    /// "next"
    Next = qjs::JS_ATOM_next as u32,
    /// "values"
    Values = qjs::JS_ATOM_values as u32,
    /// "source"
    Source = qjs::JS_ATOM_source as u32,
    /// "flags"
    Flags = qjs::JS_ATOM_flags as u32,
    /// "global"
    Global = qjs::JS_ATOM_global as u32,
    /// "unicode"
    Unicode = qjs::JS_ATOM_unicode as u32,
    /// "raw"
    Raw = qjs::JS_ATOM_raw as u32,
    /// "new.target"
    NewTarget = qjs::JS_ATOM_new_target as u32,
    /// "this.active_func"
    ThisActiveFunc = qjs::JS_ATOM_this_active_func as u32,
    /// "\<home_object\>"
    HomeObject = qjs::JS_ATOM_home_object as u32,
    /// "\<computed_field\>"
    ComputedField = qjs::JS_ATOM_computed_field as u32,
    /// "\<static_computed_field\>"
    StaticComputedField = qjs::JS_ATOM_static_computed_field as u32, // must come after computed_fields
    /// "\<class_fields_init\>"
    ClassFieldsInit = qjs::JS_ATOM_class_fields_init as u32,
    /// "\<brand\>"
    Brand = qjs::JS_ATOM_brand as u32,
    /// "#constructor"
    HashConstructor = qjs::JS_ATOM_hash_constructor as u32,
    /// "as"
    As = qjs::JS_ATOM_as as u32,
    /// "from"
    From = qjs::JS_ATOM_from as u32,
    /// "meta"
    Meta = qjs::JS_ATOM_meta as u32,
    /// "*default*"
    StarDefault = qjs::JS_ATOM__default_ as u32,
    /// "*"
    Star = qjs::JS_ATOM__star_ as u32,
    /// "Module"
    Module = qjs::JS_ATOM_Module as u32,
    /// "then"
    Then = qjs::JS_ATOM_then as u32,
    /// "resolve"
    Resolve = qjs::JS_ATOM_resolve as u32,
    /// "reject"
    Reject = qjs::JS_ATOM_reject as u32,
    /// "promise"
    PromiseLower = qjs::JS_ATOM_promise as u32,
    /// "proxy"
    ProxyLower = qjs::JS_ATOM_proxy as u32,
    /// "revoke"
    Revoke = qjs::JS_ATOM_revoke as u32,
    /// "async"
    Async = qjs::JS_ATOM_async as u32,
    /// "exec"
    Exec = qjs::JS_ATOM_exec as u32,
    /// "groups"
    Groups = qjs::JS_ATOM_groups as u32,
    /// "status"
    Status = qjs::JS_ATOM_status as u32,
    /// "reason"
    Reason = qjs::JS_ATOM_reason as u32,
    /// "globalThis"
    GlobalThis = qjs::JS_ATOM_globalThis as u32,
    /// "bigint"
    Bigint = qjs::JS_ATOM_bigint as u32,
    /// "bigfloat"
    Bigfloat = qjs::JS_ATOM_bigfloat as u32,
    /// "bigdecimal"
    Bigdecimal = qjs::JS_ATOM_bigdecimal as u32,
    /// "roundingMode"
    RoundingMode = qjs::JS_ATOM_roundingMode as u32,
    /// "maximumSignificantDigits"
    MaximumSignificantDigits = qjs::JS_ATOM_maximumSignificantDigits as u32,
    /// "maximumFractionDigits"
    MaximumFractionDigits = qjs::JS_ATOM_maximumFractionDigits as u32,
    /// "toJSON"
    ToJSON = qjs::JS_ATOM_toJSON as u32,
    /// "Object"
    Object = qjs::JS_ATOM_Object as u32,
    /// "Array"
    Array = qjs::JS_ATOM_Array as u32,
    /// "Error"
    Error = qjs::JS_ATOM_Error as u32,
    /// "Number"
    Number = qjs::JS_ATOM_Number as u32,
    /// "String"
    String = qjs::JS_ATOM_String as u32,
    /// "Boolean"
    Boolean = qjs::JS_ATOM_Boolean as u32,
    /// "Symbol"
    Symbol = qjs::JS_ATOM_Symbol as u32,
    /// "Arguments"
    Arguments = qjs::JS_ATOM_Arguments as u32,
    /// "Math"
    Math = qjs::JS_ATOM_Math as u32,
    /// "JSON"
    JSON = qjs::JS_ATOM_JSON as u32,
    /// "Date"
    Date = qjs::JS_ATOM_Date as u32,
    /// "Function"
    Function = qjs::JS_ATOM_Function as u32,
    /// "GeneratorFunction"
    GeneratorFunction = qjs::JS_ATOM_GeneratorFunction as u32,
    /// "ForInIterator"
    ForInIterator = qjs::JS_ATOM_ForInIterator as u32,
    /// "RegExp"
    RegExp = qjs::JS_ATOM_RegExp as u32,
    /// "ArrayBuffer"
    ArrayBuffer = qjs::JS_ATOM_ArrayBuffer as u32,
    /// "SharedArrayBuffer"
    SharedArrayBuffer = qjs::JS_ATOM_SharedArrayBuffer as u32,
    /// "Uint8ClampedArray"
    Uint8ClampedArray = qjs::JS_ATOM_Uint8ClampedArray as u32,
    /// "Int8Array"
    Int8Array = qjs::JS_ATOM_Int8Array as u32,
    /// "Uint8Array"
    Uint8Array = qjs::JS_ATOM_Uint8Array as u32,
    /// "Int16Array"
    Int16Array = qjs::JS_ATOM_Int16Array as u32,
    /// "Uint16Array"
    Uint16Array = qjs::JS_ATOM_Uint16Array as u32,
    /// "Int32Array"
    Int32Array = qjs::JS_ATOM_Int32Array as u32,
    /// "Uint32Array"
    Uint32Array = qjs::JS_ATOM_Uint32Array as u32,
    /// "BigInt64Array"
    BigInt64Array = qjs::JS_ATOM_BigInt64Array as u32,
    /// "BigUint64Array"
    BigUint64Array = qjs::JS_ATOM_BigUint64Array as u32,
    /// "Float32Array"
    Float32Array = qjs::JS_ATOM_Float32Array as u32,
    /// "Float64Array"
    Float64Array = qjs::JS_ATOM_Float64Array as u32,
    /// "DataView"
    DataView = qjs::JS_ATOM_DataView as u32,
    /// "BigInt"
    BigInt = qjs::JS_ATOM_BigInt as u32,
    /// "BigFloat"
    BigFloat = qjs::JS_ATOM_BigFloat as u32,
    /// "BigFloatEnv"
    BigFloatEnv = qjs::JS_ATOM_BigFloatEnv as u32,
    /// "BigDecimal"
    BigDecimal = qjs::JS_ATOM_BigDecimal as u32,
    /// "OperatorSet"
    OperatorSet = qjs::JS_ATOM_OperatorSet as u32,
    /// "Operators"
    Operators = qjs::JS_ATOM_Operators as u32,
    /// "Map"
    Map = qjs::JS_ATOM_Map as u32,
    /// "Set"
    Set = qjs::JS_ATOM_Set as u32,
    /// "WeakMap"
    WeakMap = qjs::JS_ATOM_WeakMap as u32,
    /// "WeakSet"
    WeakSet = qjs::JS_ATOM_WeakSet as u32,
    /// "Map Iterator"
    MapIterator = qjs::JS_ATOM_Map_Iterator as u32,
    /// "Set Iterator"
    SetIterator = qjs::JS_ATOM_Set_Iterator as u32,
    /// "Array Iterator"
    ArrayIterator = qjs::JS_ATOM_Array_Iterator as u32,
    /// "String Iterator"
    StringIterator = qjs::JS_ATOM_String_Iterator as u32,
    /// "RegExp String Iterator"
    RegExpStringIterator = qjs::JS_ATOM_RegExp_String_Iterator as u32,
    /// "Generator"
    Generator = qjs::JS_ATOM_Generator as u32,
    /// "Proxy"
    Proxy = qjs::JS_ATOM_Proxy as u32,
    /// "Promise"
    Promise = qjs::JS_ATOM_Promise as u32,
    /// "PromiseResolveFunction"
    PromiseResolveFunction = qjs::JS_ATOM_PromiseResolveFunction as u32,
    /// "PromiseRejectFunction"
    PromiseRejectFunction = qjs::JS_ATOM_PromiseRejectFunction as u32,
    /// "AsyncFunction"
    AsyncFunction = qjs::JS_ATOM_AsyncFunction as u32,
    /// "AsyncFunctionResolve"
    AsyncFunctionResolve = qjs::JS_ATOM_AsyncFunctionResolve as u32,
    /// "AsyncFunctionReject"
    AsyncFunctionReject = qjs::JS_ATOM_AsyncFunctionReject as u32,
    /// "AsyncGeneratorFunction"
    AsyncGeneratorFunction = qjs::JS_ATOM_AsyncGeneratorFunction as u32,
    /// "AsyncGenerator"
    AsyncGenerator = qjs::JS_ATOM_AsyncGenerator as u32,
    /// "EvalError"
    EvalError = qjs::JS_ATOM_EvalError as u32,
    /// "RangeError"
    RangeError = qjs::JS_ATOM_RangeError as u32,
    /// "ReferenceError"
    ReferenceError = qjs::JS_ATOM_ReferenceError as u32,
    /// "SyntaxError"
    SyntaxError = qjs::JS_ATOM_SyntaxError as u32,
    /// "TypeError"
    TypeError = qjs::JS_ATOM_TypeError as u32,
    /// "URIError"
    URIError = qjs::JS_ATOM_URIError as u32,
    /// "InternalError"
    InternalError = qjs::JS_ATOM_InternalError as u32,
    /// "Symbol.asyncIterator"
    SymbolAsyncIterator = qjs::JS_ATOM_Symbol_asyncIterator as u32,
    /// "Symbol.iterator"
    SymbolIterator = qjs::JS_ATOM_Symbol_iterator as u32,
    /// "Symbol.match"
    SymbolMatch = qjs::JS_ATOM_Symbol_match as u32,
    /// "Symbol.matchAll"
    SymbolMatchAll = qjs::JS_ATOM_Symbol_matchAll as u32,
    /// "Symbol.replace"
    SymbolReplace = qjs::JS_ATOM_Symbol_replace as u32,
    /// "Symbol.search"
    SymbolSearch = qjs::JS_ATOM_Symbol_search as u32,
    /// "Symbol.split"
    SymbolSplit = qjs::JS_ATOM_Symbol_split as u32,
    /// "Symbol.toStringTag"
    SymbolToStringTag = qjs::JS_ATOM_Symbol_toStringTag as u32,
    /// "Symbol.isConcatSpreadable"
    SymbolIsConcatSpreadable = qjs::JS_ATOM_Symbol_isConcatSpreadable as u32,
    /// "Symbol.hasInstance"
    SymbolHasInstance = qjs::JS_ATOM_Symbol_hasInstance as u32,
    /// "Symbol.species"
    SymbolSpecies = qjs::JS_ATOM_Symbol_species as u32,
    /// "Symbol.unscopables"
    SymbolUnscopables = qjs::JS_ATOM_Symbol_unscopables as u32,
}

impl PredefinedAtom {
    pub const fn is_symbol(self) -> bool {
        matches!(
            self,
            PredefinedAtom::SymbolAsyncIterator
                | PredefinedAtom::SymbolIterator
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
            PredefinedAtom::ColumnNumber => "columnNumber",
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
            PredefinedAtom::SymbolAsyncIterator => "Symbol.asyncIterator",
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
            PredefinedAtom::SymbolAsyncIterator,
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
                let atom = predef.into_atom(&ctx).unwrap();
                assert_eq!(atom.to_string().unwrap().as_str(), predef.to_str());

                // the string of a symbol doesn't convert to the same atom.
                if predef.is_symbol() {
                    continue;
                }

                let from_str = Atom::from_str(ctx.clone(), predef.to_str()).unwrap();
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
