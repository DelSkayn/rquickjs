mod id;
pub use id::ClassId;

mod cell;
pub use cell::{JsCell, Mutability, Readable, Writable};

pub trait JsClass {
    /// The name the constructor has in javascript
    const NAME: &'static str;

    /// Can the type be mutated while a javascript value.
    type Mutable: Mutability;

    /// A unique id for the class.
    fn class_id() -> &'static ClassId;
}
