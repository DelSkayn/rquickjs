pub struct This<T>(pub T);

pub struct Opt<T>(pub Option<T>);

pub struct Rest<T>(pub Vec<T>);

pub struct Null<T>(pub Option<T>);

/// A type to flatten tuples into another tuple.
///
/// ToArgs is only implemented for tuples with a length of up to 8.
/// If you need more arguments you can use this type to extend arguments with upto 8 additional
/// arguments recursivily.
pub struct Flat<T>(pub T);

pub struct Exhaustive;
