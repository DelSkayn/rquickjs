mod args;
pub use args::Args;
mod ffi;

use crate::{Object, Result, Value};

pub trait JsFunction {
    fn call<'a, 'js>(arguments: Args<'a, 'js>) -> Result<Value<'js>>;
}

pub struct Function<'js>(pub(crate) Value<'js>);
