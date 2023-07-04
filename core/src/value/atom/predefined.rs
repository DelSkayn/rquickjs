use crate::qjs;

/// A collection of atoms which are predefined in quickjs.
///
/// Using these over [`Atom::from_str`] can be more performant as these don't need to be looked up
/// in a hashmap.
#[derive(Clone, Copy, Eq, PartialEq, Hash, Debug)]
#[repr(u32)]
pub enum PredefinedAtom {
    /// "length"
    Length = qjs::JS_ATOM_length,
    /// "fileName",
    FileName = qjs::JS_ATOM_fileName,
    /// "LineNumber",
    LineNumber = qjs::JS_ATOM_lineNumber,
    /// "stack",
    Stack = qjs::JS_ATOM_stack,
    /// "name",
    Name = qjs::JS_ATOM_name,
    /// "toString",
    ToString = qjs::JS_ATOM_toString,
    /// "valueOf",
    ValueOf = qjs::JS_ATOM_valueOf,
    /// "prototype",
    Prototype = qjs::JS_ATOM_prototype,
    /// "constructor",
    Constructor = qjs::JS_ATOM_constructor,
    /// "configurable",
    Configurable = qjs::JS_ATOM_configurable,
    /// "writable",
    Writable = qjs::JS_ATOM_writable,
    /// "enumerable",
    Enumerable = qjs::JS_ATOM_enumerable,

    /// "Module"
    Module = qjs::JS_ATOM_Module,
    /// "then"
    Then = qjs::JS_ATOM_then,
    /// "resolve"
    Resolve = qjs::JS_ATOM_resolve,
    /// "reject"
    Reject = qjs::JS_ATOM_reject,
    /// "promise"
    Promise = qjs::JS_ATOM_promise,
    /// "proxy"
    Proxy = qjs::JS_ATOM_proxy,
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
    pub const fn to_str(self) -> &'static str {
        match self {
            PredefinedAtom::Length => "length",
            PredefinedAtom::FileName => "fileName",
            PredefinedAtom::LineNumber => "LineNumber",
            PredefinedAtom::Stack => "stack",
            PredefinedAtom::Name => "name",
            PredefinedAtom::ToString => "toString",
            PredefinedAtom::ValueOf => "valueOf",
            PredefinedAtom::Prototype => "prototype",
            PredefinedAtom::Constructor => "constructor",
            PredefinedAtom::Configurable => "configurable",
            PredefinedAtom::Writable => "writable",
            PredefinedAtom::Enumerable => "enumerable",
            PredefinedAtom::Module => "Module",
            PredefinedAtom::Then => "then",
            PredefinedAtom::Resolve => "resolve",
            PredefinedAtom::Reject => "reject",
            PredefinedAtom::Promise => "promise",
            PredefinedAtom::Proxy => "proxy",
            PredefinedAtom::Revoke => "revoke",
            PredefinedAtom::Async => "async",
            PredefinedAtom::Exec => "exec",
            PredefinedAtom::Groups => "groups",
            PredefinedAtom::Status => "status",
            PredefinedAtom::Reason => "reason",
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
