#![allow(dead_code)]

/// Vendored version of relative-path that supports no_std
/// Taken from on:
///   https://github.com/udoprog/relative-path/blob/7a3a0fb6472e13b43103b28eea725f11dc6e1f11/relative-path/src/lib.rs
///
/// Go back to upstream dependency when https://github.com/udoprog/relative-path/pull/64 is merged
/// and a new version is released.
use alloc::{
    borrow::{Borrow, Cow, ToOwned},
    boxed::Box,
    rc::Rc,
    string::String,
    sync::Arc,
};
use core::{
    cmp, error, fmt,
    hash::{Hash, Hasher},
    iter::FromIterator,
    mem, ops, str,
};

#[cfg(feature = "std")]
use std::path;

const STEM_SEP: char = '.';
const CURRENT_STR: &str = ".";
const PARENT_STR: &str = "..";

const SEP: char = '/';

fn split_file_at_dot(input: &str) -> (Option<&str>, Option<&str>) {
    if input == PARENT_STR {
        return (Some(input), None);
    }

    let mut iter = input.rsplitn(2, STEM_SEP);

    let after = iter.next();
    let before = iter.next();

    if before == Some("") {
        (Some(input), None)
    } else {
        (before, after)
    }
}

// Iterate through `iter` while it matches `prefix`; return `None` if `prefix`
// is not a prefix of `iter`, otherwise return `Some(iter_after_prefix)` giving
// `iter` after having exhausted `prefix`.
fn iter_after<'a, 'b, I, J>(mut iter: I, mut prefix: J) -> Option<I>
where
    I: Iterator<Item = Component<'a>> + Clone,
    J: Iterator<Item = Component<'b>>,
{
    loop {
        let mut iter_next = iter.clone();
        match (iter_next.next(), prefix.next()) {
            (Some(x), Some(y)) if x == y => (),
            (Some(_) | None, Some(_)) => return None,
            (Some(_) | None, None) => return Some(iter),
        }
        iter = iter_next;
    }
}

/// A single path component.
///
/// Accessed using the [`RelativePath::components`] iterator.
///
/// # Examples
///
#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum Component<'a> {
    /// The current directory `.`.
    CurDir,
    /// The parent directory `..`.
    ParentDir,
    /// A normal path component as a string.
    Normal(&'a str),
}

impl<'a> Component<'a> {
    /// Extracts the underlying [`str`] slice.
    ///
    /// [`str`]: prim@str
    ///
    /// # Examples
    ///
    #[must_use]
    pub fn as_str(self) -> &'a str {
        use self::Component::{CurDir, Normal, ParentDir};

        match self {
            CurDir => CURRENT_STR,
            ParentDir => PARENT_STR,
            Normal(name) => name,
        }
    }
}

/// [`AsRef<RelativePath>`] implementation for [`Component`].
///
/// # Examples
///
impl AsRef<RelativePath> for Component<'_> {
    #[inline]
    fn as_ref(&self) -> &RelativePath {
        self.as_str().as_ref()
    }
}

/// Traverse the given components and apply to the provided stack.
///
/// This takes '.', and '..' into account. Where '.' doesn't change the stack, and '..' pops the
/// last item or further adds parent components.
#[inline(always)]
fn relative_traversal<'a, C>(buf: &mut RelativePathBuf, components: C)
where
    C: IntoIterator<Item = Component<'a>>,
{
    use self::Component::{CurDir, Normal, ParentDir};

    for c in components {
        match c {
            CurDir => (),
            ParentDir => match buf.components().next_back() {
                Some(Component::ParentDir) | None => {
                    buf.push(PARENT_STR);
                }
                _ => {
                    buf.pop();
                }
            },
            Normal(name) => {
                buf.push(name);
            }
        }
    }
}

/// Iterator over all the components in a relative path.
#[derive(Clone)]
pub struct Components<'a> {
    source: &'a str,
}

impl<'a> Iterator for Components<'a> {
    type Item = Component<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        self.source = self.source.trim_start_matches(SEP);

        let slice = match self.source.find(SEP) {
            Some(i) => {
                let (slice, rest) = self.source.split_at(i);
                self.source = rest.trim_start_matches(SEP);
                slice
            }
            None => mem::take(&mut self.source),
        };

        match slice {
            "" => None,
            CURRENT_STR => Some(Component::CurDir),
            PARENT_STR => Some(Component::ParentDir),
            slice => Some(Component::Normal(slice)),
        }
    }
}

impl DoubleEndedIterator for Components<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.source = self.source.trim_end_matches(SEP);

        let slice = match self.source.rfind(SEP) {
            Some(i) => {
                let (rest, slice) = self.source.split_at(i + 1);
                self.source = rest.trim_end_matches(SEP);
                slice
            }
            None => mem::take(&mut self.source),
        };

        match slice {
            "" => None,
            CURRENT_STR => Some(Component::CurDir),
            PARENT_STR => Some(Component::ParentDir),
            slice => Some(Component::Normal(slice)),
        }
    }
}

impl<'a> Components<'a> {
    /// Construct a new component from the given string.
    fn new(source: &'a str) -> Components<'a> {
        Self { source }
    }

    /// Extracts a slice corresponding to the portion of the path remaining for iteration.
    ///
    /// # Examples
    ///
    #[must_use]
    #[inline]
    pub fn as_relative_path(&self) -> &'a RelativePath {
        RelativePath::new(self.source)
    }
}

impl<'a> cmp::PartialEq for Components<'a> {
    fn eq(&self, other: &Components<'a>) -> bool {
        Iterator::eq(self.clone(), other.clone())
    }
}

/// An iterator over the [`Component`]s of a [`RelativePath`], as [`str`]
/// slices.
///
/// This `struct` is created by the [`iter`][RelativePath::iter] method.
///
/// [`str`]: prim@str
#[derive(Clone)]
pub struct Iter<'a> {
    inner: Components<'a>,
}

impl<'a> Iterator for Iter<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<&'a str> {
        self.inner.next().map(Component::as_str)
    }
}

impl<'a> DoubleEndedIterator for Iter<'a> {
    fn next_back(&mut self) -> Option<&'a str> {
        self.inner.next_back().map(Component::as_str)
    }
}

/// Error kind for [`FromPathError`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum FromPathErrorKind {
    /// Non-relative component in path.
    NonRelative,
    /// Non-utf8 component in path.
    NonUtf8,
    /// Trying to convert a platform-specific path which uses a platform-specific separator.
    BadSeparator,
}

/// An error raised when attempting to convert a path using
/// [`RelativePathBuf::from_path`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FromPathError {
    kind: FromPathErrorKind,
}

impl FromPathError {
    /// Gets the underlying [`FromPathErrorKind`] that provides more details on
    /// what went wrong.
    ///
    /// # Examples
    ///
    #[must_use]
    pub fn kind(&self) -> FromPathErrorKind {
        self.kind
    }
}

impl From<FromPathErrorKind> for FromPathError {
    fn from(value: FromPathErrorKind) -> Self {
        Self { kind: value }
    }
}

impl fmt::Display for FromPathError {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match self.kind {
            FromPathErrorKind::NonRelative => "path contains non-relative component".fmt(fmt),
            FromPathErrorKind::NonUtf8 => "path contains non-utf8 component".fmt(fmt),
            FromPathErrorKind::BadSeparator => {
                "path contains platform-specific path separator".fmt(fmt)
            }
        }
    }
}

impl error::Error for FromPathError {}

/// An owned, mutable relative path.
///
/// This type provides methods to manipulate relative path objects.
#[derive(Clone)]
pub struct RelativePathBuf {
    inner: String,
}

impl RelativePathBuf {
    /// Create a new relative path buffer.
    #[must_use]
    pub fn new() -> RelativePathBuf {
        RelativePathBuf {
            inner: String::new(),
        }
    }

    /// Internal constructor to allocate a relative path buf with the given capacity.
    fn with_capacity(cap: usize) -> RelativePathBuf {
        RelativePathBuf {
            inner: String::with_capacity(cap),
        }
    }

    #[cfg(feature = "std")]
    /// Try to convert a [`Path`] to a [`RelativePathBuf`].
    ///
    /// [`Path`]: https://doc.rust-lang.org/std/path/struct.Path.html
    ///
    /// # Examples
    ///
    ///
    /// # Errors
    ///
    /// This will error in case the provided path is not a relative path, which
    /// is identifier by it having a [`Prefix`] or [`RootDir`] component.
    ///
    /// [`Prefix`]: std::path::Component::Prefix
    /// [`RootDir`]: std::path::Component::RootDir
    pub fn from_path<P: AsRef<path::Path>>(path: P) -> Result<RelativePathBuf, FromPathError> {
        use std::path::Component::{CurDir, Normal, ParentDir, Prefix, RootDir};

        let mut buffer = RelativePathBuf::new();

        for c in path.as_ref().components() {
            match c {
                Prefix(_) | RootDir => return Err(FromPathErrorKind::NonRelative.into()),
                CurDir => continue,
                ParentDir => buffer.push(PARENT_STR),
                Normal(s) => buffer.push(s.to_str().ok_or(FromPathErrorKind::NonUtf8)?),
            }
        }

        Ok(buffer)
    }

    /// Extends `self` with `path`.
    ///
    /// # Examples
    ///
    pub fn push<P>(&mut self, path: P)
    where
        P: AsRef<RelativePath>,
    {
        let other = path.as_ref();

        let other = if other.starts_with_sep() {
            &other.inner[1..]
        } else {
            &other.inner[..]
        };

        if !self.inner.is_empty() && !self.ends_with_sep() {
            self.inner.push(SEP);
        }

        self.inner.push_str(other);
    }

    /// Updates [`file_name`] to `file_name`.
    ///
    /// If [`file_name`] was [`None`], this is equivalent to pushing
    /// `file_name`.
    ///
    /// Otherwise it is equivalent to calling [`pop`] and then pushing
    /// `file_name`. The new path will be a sibling of the original path. (That
    /// is, it will have the same parent.)
    ///
    /// [`file_name`]: RelativePath::file_name
    /// [`pop`]: RelativePathBuf::pop
    /// [`None`]: https://doc.rust-lang.org/std/option/enum.Option.html
    ///
    /// # Examples
    ///
    pub fn set_file_name<S: AsRef<str>>(&mut self, file_name: S) {
        if self.file_name().is_some() {
            let popped = self.pop();
            debug_assert!(popped);
        }

        self.push(file_name.as_ref());
    }

    /// Updates [`extension`] to `extension`.
    ///
    /// Returns `false` and does nothing if
    /// [`file_name`][RelativePath::file_name] is [`None`], returns `true` and
    /// updates the extension otherwise.
    ///
    /// If [`extension`] is [`None`], the extension is added; otherwise it is
    /// replaced.
    ///
    /// [`extension`]: RelativePath::extension
    ///
    /// # Examples
    ///
    pub fn set_extension<S: AsRef<str>>(&mut self, extension: S) -> bool {
        let file_stem = match self.file_stem() {
            Some(stem) => stem,
            None => return false,
        };

        let end_file_stem = file_stem[file_stem.len()..].as_ptr() as usize;
        let start = self.inner.as_ptr() as usize;
        self.inner.truncate(end_file_stem.wrapping_sub(start));

        let extension = extension.as_ref();

        if !extension.is_empty() {
            self.inner.push(STEM_SEP);
            self.inner.push_str(extension);
        }

        true
    }

    /// Truncates `self` to [`parent`][RelativePath::parent].
    ///
    /// # Examples
    ///
    pub fn pop(&mut self) -> bool {
        match self.parent().map(|p| p.inner.len()) {
            Some(len) => {
                self.inner.truncate(len);
                true
            }
            None => false,
        }
    }

    /// Coerce to a [`RelativePath`] slice.
    #[must_use]
    pub fn as_relative_path(&self) -> &RelativePath {
        self
    }

    /// Consumes the `RelativePathBuf`, yielding its internal [`String`] storage.
    ///
    /// # Examples
    ///
    #[must_use]
    pub fn into_string(self) -> String {
        self.inner
    }

    /// Converts this `RelativePathBuf` into a [boxed][std::boxed::Box]
    /// [`RelativePath`].
    #[must_use]
    pub fn into_boxed_relative_path(self) -> Box<RelativePath> {
        let rw = Box::into_raw(self.inner.into_boxed_str()) as *mut RelativePath;
        unsafe { Box::from_raw(rw) }
    }
}

impl Default for RelativePathBuf {
    fn default() -> Self {
        RelativePathBuf::new()
    }
}

impl<'a> From<&'a RelativePath> for Cow<'a, RelativePath> {
    #[inline]
    fn from(s: &'a RelativePath) -> Cow<'a, RelativePath> {
        Cow::Borrowed(s)
    }
}

impl<'a> From<RelativePathBuf> for Cow<'a, RelativePath> {
    #[inline]
    fn from(s: RelativePathBuf) -> Cow<'a, RelativePath> {
        Cow::Owned(s)
    }
}

impl fmt::Debug for RelativePathBuf {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "{:?}", &self.inner)
    }
}

impl AsRef<RelativePath> for RelativePathBuf {
    fn as_ref(&self) -> &RelativePath {
        RelativePath::new(&self.inner)
    }
}

impl AsRef<str> for RelativePath {
    fn as_ref(&self) -> &str {
        &self.inner
    }
}

impl Borrow<RelativePath> for RelativePathBuf {
    #[inline]
    fn borrow(&self) -> &RelativePath {
        self
    }
}

impl<'a, T: ?Sized + AsRef<str>> From<&'a T> for RelativePathBuf {
    fn from(path: &'a T) -> RelativePathBuf {
        RelativePathBuf {
            inner: path.as_ref().to_owned(),
        }
    }
}

impl From<String> for RelativePathBuf {
    fn from(path: String) -> RelativePathBuf {
        RelativePathBuf { inner: path }
    }
}

impl From<RelativePathBuf> for String {
    fn from(path: RelativePathBuf) -> String {
        path.into_string()
    }
}

impl ops::Deref for RelativePathBuf {
    type Target = RelativePath;

    fn deref(&self) -> &RelativePath {
        RelativePath::new(&self.inner)
    }
}

impl cmp::PartialEq for RelativePathBuf {
    fn eq(&self, other: &RelativePathBuf) -> bool {
        self.components() == other.components()
    }
}

impl cmp::Eq for RelativePathBuf {}

impl cmp::PartialOrd for RelativePathBuf {
    #[inline]
    fn partial_cmp(&self, other: &RelativePathBuf) -> Option<cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl cmp::Ord for RelativePathBuf {
    #[inline]
    fn cmp(&self, other: &RelativePathBuf) -> cmp::Ordering {
        self.components().cmp(other.components())
    }
}

impl Hash for RelativePathBuf {
    fn hash<H: Hasher>(&self, h: &mut H) {
        self.as_relative_path().hash(h);
    }
}

impl<P> Extend<P> for RelativePathBuf
where
    P: AsRef<RelativePath>,
{
    #[inline]
    fn extend<I: IntoIterator<Item = P>>(&mut self, iter: I) {
        iter.into_iter().for_each(move |p| self.push(p.as_ref()));
    }
}

impl<P> FromIterator<P> for RelativePathBuf
where
    P: AsRef<RelativePath>,
{
    #[inline]
    fn from_iter<I: IntoIterator<Item = P>>(iter: I) -> RelativePathBuf {
        let mut buf = RelativePathBuf::new();
        buf.extend(iter);
        buf
    }
}

/// A borrowed, immutable relative path.
#[repr(transparent)]
pub struct RelativePath {
    inner: str,
}

/// An error returned from [`strip_prefix`] if the prefix was not found.
///
/// [`strip_prefix`]: RelativePath::strip_prefix
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StripPrefixError(());

impl RelativePath {
    /// Directly wraps a string slice as a `RelativePath` slice.
    pub fn new<S: AsRef<str> + ?Sized>(s: &S) -> &RelativePath {
        unsafe { &*(s.as_ref() as *const str as *const RelativePath) }
    }

    #[cfg(feature = "std")]
    /// Try to convert a [`Path`] to a [`RelativePath`] without allocating a buffer.
    ///
    /// [`Path`]: std::path::Path
    ///
    /// # Errors
    ///
    /// This requires the path to be a legal, platform-neutral relative path.
    /// Otherwise various forms of [`FromPathError`] will be returned as an
    /// [`Err`].
    ///
    /// # Examples
    ///
    pub fn from_path<P: ?Sized + AsRef<path::Path>>(
        path: &P,
    ) -> Result<&RelativePath, FromPathError> {
        use std::path::Component::{CurDir, Normal, ParentDir, Prefix, RootDir};

        let other = path.as_ref();

        let s = match other.to_str() {
            Some(s) => s,
            None => return Err(FromPathErrorKind::NonUtf8.into()),
        };

        let rel = RelativePath::new(s);

        // check that the component compositions are equal.
        for (a, b) in other.components().zip(rel.components()) {
            match (a, b) {
                (Prefix(_) | RootDir, _) => return Err(FromPathErrorKind::NonRelative.into()),
                (CurDir, Component::CurDir) | (ParentDir, Component::ParentDir) => continue,
                (Normal(a), Component::Normal(b)) if a == b => continue,
                _ => return Err(FromPathErrorKind::BadSeparator.into()),
            }
        }

        Ok(rel)
    }

    /// Yields the underlying [`str`] slice.
    ///
    /// [`str`]: prim@str
    ///
    /// # Examples
    ///
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.inner
    }

    /// Returns an object that implements [`Display`][std::fmt::Display].
    ///
    /// # Examples
    ///
    #[deprecated(note = "RelativePath implements std::fmt::Display directly")]
    #[must_use]
    pub fn display(&self) -> Display {
        Display { path: self }
    }

    /// Creates an owned [`RelativePathBuf`] with path adjoined to self.
    ///
    /// # Examples
    ///
    pub fn join<P>(&self, path: P) -> RelativePathBuf
    where
        P: AsRef<RelativePath>,
    {
        let mut out = self.to_relative_path_buf();
        out.push(path);
        out
    }

    /// Iterate over all components in this relative path.
    ///
    /// # Examples
    ///
    #[must_use]
    pub fn components(&self) -> Components {
        Components::new(&self.inner)
    }

    /// Produces an iterator over the path's components viewed as [`str`]
    /// slices.
    ///
    /// For more information about the particulars of how the path is separated
    /// into components, see [`components`][Self::components].
    ///
    /// [`str`]: prim@str
    ///
    /// # Examples
    ///
    #[must_use]
    pub fn iter(&self) -> Iter {
        Iter {
            inner: self.components(),
        }
    }

    /// Convert to an owned [`RelativePathBuf`].
    #[must_use]
    pub fn to_relative_path_buf(&self) -> RelativePathBuf {
        RelativePathBuf::from(self.inner.to_owned())
    }

    #[cfg(feature = "std")]
    /// Build an owned [`PathBuf`] relative to `base` for the current relative
    /// path.
    ///
    /// # Examples
    ///
    ///
    /// # Encoding an absolute path
    ///
    /// Absolute paths are, in contrast to when using [`PathBuf::push`] *ignored*
    /// and will be added unchanged to the buffer.
    ///
    /// This is to preserve the probability of a path conversion failing if the
    /// relative path contains platform-specific absolute path components.
    ///
    ///
    /// [`PathBuf`]: std::path::PathBuf
    /// [`PathBuf::push`]: std::path::PathBuf::push
    pub fn to_path<P: AsRef<path::Path>>(&self, base: P) -> path::PathBuf {
        let mut p = base.as_ref().to_path_buf().into_os_string();

        for c in self.components() {
            if !p.is_empty() {
                p.push(path::MAIN_SEPARATOR.encode_utf8(&mut [0u8, 0u8, 0u8, 0u8]));
            }

            p.push(c.as_str());
        }

        path::PathBuf::from(p)
    }

    #[cfg(feature = "std")]
    /// Build an owned [`PathBuf`] relative to `base` for the current relative
    /// path.
    ///
    /// This is similar to [`to_path`] except that it doesn't just
    /// unconditionally append one path to the other, instead it performs the
    /// following operations depending on its own components:
    ///
    /// * [`Component::CurDir`] leaves the `base` unmodified.
    /// * [`Component::ParentDir`] removes a component from `base` using
    ///   [`path::PathBuf::pop`].
    /// * [`Component::Normal`] pushes the given path component onto `base`
    ///   using the same mechanism as [`to_path`].
    ///
    /// [`to_path`]: RelativePath::to_path
    ///
    /// Note that the exact semantics of the path operation is determined by the
    /// corresponding [`PathBuf`] operation. E.g. popping a component off a path
    /// like `.` will result in an empty path.
    ///
    ///
    /// # Examples
    ///
    ///
    /// # Encoding an absolute path
    ///
    /// Behaves the same as [`to_path`][RelativePath::to_path] when encoding
    /// absolute paths.
    ///
    /// Absolute paths are, in contrast to when using [`PathBuf::push`] *ignored*
    /// and will be added unchanged to the buffer.
    ///
    /// This is to preserve the probability of a path conversion failing if the
    /// relative path contains platform-specific absolute path components.
    ///
    ///
    /// [`PathBuf`]: std::path::PathBuf
    /// [`PathBuf::push`]: std::path::PathBuf::push
    pub fn to_logical_path<P: AsRef<path::Path>>(&self, base: P) -> path::PathBuf {
        use self::Component::{CurDir, Normal, ParentDir};

        let mut p = base.as_ref().to_path_buf().into_os_string();

        for c in self.components() {
            match c {
                CurDir => continue,
                ParentDir => {
                    let mut temp = path::PathBuf::from(std::mem::take(&mut p));
                    temp.pop();
                    p = temp.into_os_string();
                }
                Normal(c) => {
                    if !p.is_empty() {
                        p.push(path::MAIN_SEPARATOR.encode_utf8(&mut [0u8, 0u8, 0u8, 0u8]));
                    }

                    p.push(c);
                }
            }
        }

        path::PathBuf::from(p)
    }

    /// Returns a relative path, without its final [`Component`] if there is one.
    ///
    /// # Examples
    ///
    #[must_use]
    pub fn parent(&self) -> Option<&RelativePath> {
        use self::Component::CurDir;

        if self.inner.is_empty() {
            return None;
        }

        let mut it = self.components();
        while let Some(CurDir) = it.next_back() {}
        Some(it.as_relative_path())
    }

    /// Returns the final component of the `RelativePath`, if there is one.
    ///
    /// If the path is a normal file, this is the file name. If it's the path of
    /// a directory, this is the directory name.
    ///
    /// Returns [`None`] If the path terminates in `..`.
    ///
    /// # Examples
    ///
    #[must_use]
    pub fn file_name(&self) -> Option<&str> {
        use self::Component::{CurDir, Normal, ParentDir};

        let mut it = self.components();

        while let Some(c) = it.next_back() {
            return match c {
                CurDir => continue,
                Normal(name) => Some(name),
                ParentDir => None,
            };
        }

        None
    }

    /// Returns a relative path that, when joined onto `base`, yields `self`.
    ///
    /// # Errors
    ///
    /// If `base` is not a prefix of `self` (i.e. [`starts_with`] returns
    /// `false`), returns [`Err`].
    ///
    /// [`starts_with`]: Self::starts_with
    ///
    /// # Examples
    ///
    pub fn strip_prefix<P>(&self, base: P) -> Result<&RelativePath, StripPrefixError>
    where
        P: AsRef<RelativePath>,
    {
        iter_after(self.components(), base.as_ref().components())
            .map(|c| c.as_relative_path())
            .ok_or(StripPrefixError(()))
    }

    /// Determines whether `base` is a prefix of `self`.
    ///
    /// Only considers whole path components to match.
    ///
    /// # Examples
    ///
    pub fn starts_with<P>(&self, base: P) -> bool
    where
        P: AsRef<RelativePath>,
    {
        iter_after(self.components(), base.as_ref().components()).is_some()
    }

    /// Determines whether `child` is a suffix of `self`.
    ///
    /// Only considers whole path components to match.
    ///
    /// # Examples
    ///
    pub fn ends_with<P>(&self, child: P) -> bool
    where
        P: AsRef<RelativePath>,
    {
        iter_after(self.components().rev(), child.as_ref().components().rev()).is_some()
    }

    /// Determines whether `self` is normalized.
    ///
    /// # Examples
    ///
    #[must_use]
    pub fn is_normalized(&self) -> bool {
        self.components()
            .skip_while(|c| matches!(c, Component::ParentDir))
            .all(|c| matches!(c, Component::Normal(_)))
    }

    /// Creates an owned [`RelativePathBuf`] like `self` but with the given file
    /// name.
    ///
    /// See [`set_file_name`] for more details.
    ///
    /// [`set_file_name`]: RelativePathBuf::set_file_name
    ///
    /// # Examples
    ///
    pub fn with_file_name<S: AsRef<str>>(&self, file_name: S) -> RelativePathBuf {
        let mut buf = self.to_relative_path_buf();
        buf.set_file_name(file_name);
        buf
    }

    /// Extracts the stem (non-extension) portion of [`file_name`][Self::file_name].
    ///
    /// The stem is:
    ///
    /// * [`None`], if there is no file name;
    /// * The entire file name if there is no embedded `.`;
    /// * The entire file name if the file name begins with `.` and has no other `.`s within;
    /// * Otherwise, the portion of the file name before the final `.`
    ///
    /// # Examples
    ///
    pub fn file_stem(&self) -> Option<&str> {
        self.file_name()
            .map(split_file_at_dot)
            .and_then(|(before, after)| before.or(after))
    }

    /// Extracts the extension of [`file_name`][Self::file_name], if possible.
    ///
    /// The extension is:
    ///
    /// * [`None`], if there is no file name;
    /// * [`None`], if there is no embedded `.`;
    /// * [`None`], if the file name begins with `.` and has no other `.`s within;
    /// * Otherwise, the portion of the file name after the final `.`
    ///
    /// # Examples
    ///
    pub fn extension(&self) -> Option<&str> {
        self.file_name()
            .map(split_file_at_dot)
            .and_then(|(before, after)| before.and(after))
    }

    /// Creates an owned [`RelativePathBuf`] like `self` but with the given
    /// extension.
    ///
    /// See [`set_extension`] for more details.
    ///
    /// [`set_extension`]: RelativePathBuf::set_extension
    ///
    /// # Examples
    ///
    pub fn with_extension<S: AsRef<str>>(&self, extension: S) -> RelativePathBuf {
        let mut buf = self.to_relative_path_buf();
        buf.set_extension(extension);
        buf
    }

    /// Build an owned [`RelativePathBuf`], joined with the given path and
    /// normalized.
    ///
    /// # Examples
    ///
    pub fn join_normalized<P>(&self, path: P) -> RelativePathBuf
    where
        P: AsRef<RelativePath>,
    {
        let mut buf = RelativePathBuf::new();
        relative_traversal(&mut buf, self.components());
        relative_traversal(&mut buf, path.as_ref().components());
        buf
    }

    /// Return an owned [`RelativePathBuf`], with all non-normal components
    /// moved to the beginning of the path.
    ///
    /// This permits for a normalized representation of different relative
    /// components.
    ///
    /// Normalization is a _destructive_ operation if the path references an
    /// actual filesystem path. An example of this is symlinks under unix, a
    /// path like `foo/../bar` might reference a different location other than
    /// `./bar`.
    ///
    /// Normalization is a logical operation and does not guarantee that the
    /// constructed path corresponds to what the filesystem would do. On Linux
    /// for example symbolic links could mean that the logical path doesn't
    /// correspond to the filesystem path.
    ///
    /// # Examples
    ///
    #[must_use]
    pub fn normalize(&self) -> RelativePathBuf {
        let mut buf = RelativePathBuf::with_capacity(self.inner.len());
        relative_traversal(&mut buf, self.components());
        buf
    }

    /// Constructs a relative path from the current path, to `path`.
    ///
    /// This function will return the empty [`RelativePath`] `""` if this source
    /// contains unnamed components like `..` that would have to be traversed to
    /// reach the destination `path`. This is necessary since we have no way of
    /// knowing what the names of those components are when we're building the
    /// new relative path.
    ///
    ///
    /// One exception to this is when two paths contains a common prefix at
    /// which point there's no need to know what the names of those unnamed
    /// components are.
    ///
    ///
    /// # Examples
    ///
    pub fn relative<P>(&self, path: P) -> RelativePathBuf
    where
        P: AsRef<RelativePath>,
    {
        let mut from = RelativePathBuf::with_capacity(self.inner.len());
        let mut to = RelativePathBuf::with_capacity(path.as_ref().inner.len());

        relative_traversal(&mut from, self.components());
        relative_traversal(&mut to, path.as_ref().components());

        let mut it_from = from.components();
        let mut it_to = to.components();

        // Strip a common prefixes - if any.
        let (lead_from, lead_to) = loop {
            match (it_from.next(), it_to.next()) {
                (Some(f), Some(t)) if f == t => continue,
                (f, t) => {
                    break (f, t);
                }
            }
        };

        // Special case: The path we are traversing from can't contain unnamed
        // components. A relative path might be any path, like `/`, or
        // `/foo/bar/baz`, and these components cannot be named in the relative
        // traversal.
        //
        // Also note that `relative_traversal` guarantees that all ParentDir
        // components are at the head of the path being built.
        if lead_from == Some(Component::ParentDir) {
            return RelativePathBuf::new();
        }

        let head = lead_from.into_iter().chain(it_from);
        let tail = lead_to.into_iter().chain(it_to);

        let mut buf = RelativePathBuf::with_capacity(usize::max(from.inner.len(), to.inner.len()));

        for c in head.map(|_| Component::ParentDir).chain(tail) {
            buf.push(c.as_str());
        }

        buf
    }

    /// Check if path starts with a path separator.
    #[inline]
    fn starts_with_sep(&self) -> bool {
        self.inner.starts_with(SEP)
    }

    /// Check if path ends with a path separator.
    #[inline]
    fn ends_with_sep(&self) -> bool {
        self.inner.ends_with(SEP)
    }
}

impl<'a> IntoIterator for &'a RelativePath {
    type IntoIter = Iter<'a>;
    type Item = &'a str;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// Conversion from a [`Box<str>`] reference to a [`Box<RelativePath>`].
///
/// # Examples
///
impl From<Box<str>> for Box<RelativePath> {
    #[inline]
    fn from(boxed: Box<str>) -> Box<RelativePath> {
        let rw = Box::into_raw(boxed) as *mut RelativePath;
        unsafe { Box::from_raw(rw) }
    }
}

/// Conversion from a [`str`] reference to a [`Box<RelativePath>`].
///
/// [`str`]: prim@str
///
/// # Examples
///
impl<T> From<&T> for Box<RelativePath>
where
    T: ?Sized + AsRef<str>,
{
    #[inline]
    fn from(path: &T) -> Box<RelativePath> {
        Box::<RelativePath>::from(Box::<str>::from(path.as_ref()))
    }
}

/// Conversion from [`RelativePathBuf`] to [`Box<RelativePath>`].
///
/// # Examples
///
impl From<RelativePathBuf> for Box<RelativePath> {
    #[inline]
    fn from(path: RelativePathBuf) -> Box<RelativePath> {
        let boxed: Box<str> = path.inner.into();
        let rw = Box::into_raw(boxed) as *mut RelativePath;
        unsafe { Box::from_raw(rw) }
    }
}

/// Clone implementation for [`Box<RelativePath>`].
///
/// # Examples
///
impl Clone for Box<RelativePath> {
    #[inline]
    fn clone(&self) -> Self {
        self.to_relative_path_buf().into_boxed_relative_path()
    }
}

/// Conversion from [`RelativePath`] to [`Arc<RelativePath>`].
///
/// # Examples
///
impl From<&RelativePath> for Arc<RelativePath> {
    #[inline]
    fn from(path: &RelativePath) -> Arc<RelativePath> {
        let arc: Arc<str> = path.inner.into();
        let rw = Arc::into_raw(arc) as *const RelativePath;
        unsafe { Arc::from_raw(rw) }
    }
}

/// Conversion from [`RelativePathBuf`] to [`Arc<RelativePath>`].
///
/// # Examples
///
impl From<RelativePathBuf> for Arc<RelativePath> {
    #[inline]
    fn from(path: RelativePathBuf) -> Arc<RelativePath> {
        let arc: Arc<str> = path.inner.into();
        let rw = Arc::into_raw(arc) as *const RelativePath;
        unsafe { Arc::from_raw(rw) }
    }
}

/// Conversion from [`RelativePathBuf`] to [`Rc<RelativePath>`].
///
/// # Examples
///
impl From<&RelativePath> for Rc<RelativePath> {
    #[inline]
    fn from(path: &RelativePath) -> Rc<RelativePath> {
        let rc: Rc<str> = path.inner.into();
        let rw = Rc::into_raw(rc) as *const RelativePath;
        unsafe { Rc::from_raw(rw) }
    }
}

/// Conversion from [`RelativePathBuf`] to [`Rc<RelativePath>`].
///
/// # Examples
///
impl From<RelativePathBuf> for Rc<RelativePath> {
    #[inline]
    fn from(path: RelativePathBuf) -> Rc<RelativePath> {
        let rc: Rc<str> = path.inner.into();
        let rw = Rc::into_raw(rc) as *const RelativePath;
        unsafe { Rc::from_raw(rw) }
    }
}

/// [`ToOwned`] implementation for [`RelativePath`].
///
/// # Examples
///
impl ToOwned for RelativePath {
    type Owned = RelativePathBuf;

    #[inline]
    fn to_owned(&self) -> RelativePathBuf {
        self.to_relative_path_buf()
    }
}

impl fmt::Debug for RelativePath {
    #[inline]
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "{:?}", &self.inner)
    }
}

/// [`AsRef<str>`] implementation for [`RelativePathBuf`].
///
/// # Examples
///
impl AsRef<str> for RelativePathBuf {
    #[inline]
    fn as_ref(&self) -> &str {
        &self.inner
    }
}

/// [`AsRef<RelativePath>`] implementation for [String].
///
/// # Examples
///
impl AsRef<RelativePath> for String {
    #[inline]
    fn as_ref(&self) -> &RelativePath {
        RelativePath::new(self)
    }
}

/// [`AsRef<RelativePath>`] implementation for [`str`].
///
/// [`str`]: prim@str
///
/// # Examples
///
impl AsRef<RelativePath> for str {
    #[inline]
    fn as_ref(&self) -> &RelativePath {
        RelativePath::new(self)
    }
}

impl AsRef<RelativePath> for RelativePath {
    #[inline]
    fn as_ref(&self) -> &RelativePath {
        self
    }
}

impl cmp::PartialEq for RelativePath {
    #[inline]
    fn eq(&self, other: &RelativePath) -> bool {
        self.components() == other.components()
    }
}

impl cmp::Eq for RelativePath {}

impl cmp::PartialOrd for RelativePath {
    #[inline]
    fn partial_cmp(&self, other: &RelativePath) -> Option<cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl cmp::Ord for RelativePath {
    #[inline]
    fn cmp(&self, other: &RelativePath) -> cmp::Ordering {
        self.components().cmp(other.components())
    }
}

impl Hash for RelativePath {
    #[inline]
    fn hash<H: Hasher>(&self, h: &mut H) {
        for c in self.components() {
            c.hash(h);
        }
    }
}

impl fmt::Display for RelativePath {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.inner, f)
    }
}

impl fmt::Display for RelativePathBuf {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.inner, f)
    }
}

/// Helper struct for printing relative paths.
///
/// This is not strictly necessary in the same sense as it is for [`Display`],
/// because relative paths are guaranteed to be valid UTF-8. But the behavior is
/// preserved to simplify the transition between [`Path`] and [`RelativePath`].
///
/// [`Path`]: std::path::Path
/// [`Display`]: std::fmt::Display
pub struct Display<'a> {
    path: &'a RelativePath,
}

impl fmt::Debug for Display<'_> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&self.path, f)
    }
}

impl fmt::Display for Display<'_> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.path, f)
    }
}

macro_rules! impl_cmp {
    ($lhs:ty, $rhs:ty) => {
        impl<'a, 'b> PartialEq<$rhs> for $lhs {
            #[inline]
            fn eq(&self, other: &$rhs) -> bool {
                <RelativePath as PartialEq>::eq(self, other)
            }
        }

        impl<'a, 'b> PartialEq<$lhs> for $rhs {
            #[inline]
            fn eq(&self, other: &$lhs) -> bool {
                <RelativePath as PartialEq>::eq(self, other)
            }
        }

        impl<'a, 'b> PartialOrd<$rhs> for $lhs {
            #[inline]
            fn partial_cmp(&self, other: &$rhs) -> Option<cmp::Ordering> {
                <RelativePath as PartialOrd>::partial_cmp(self, other)
            }
        }

        impl<'a, 'b> PartialOrd<$lhs> for $rhs {
            #[inline]
            fn partial_cmp(&self, other: &$lhs) -> Option<cmp::Ordering> {
                <RelativePath as PartialOrd>::partial_cmp(self, other)
            }
        }
    };
}

impl_cmp!(RelativePathBuf, RelativePath);
impl_cmp!(RelativePathBuf, &'a RelativePath);
impl_cmp!(Cow<'a, RelativePath>, RelativePath);
impl_cmp!(Cow<'a, RelativePath>, &'b RelativePath);
impl_cmp!(Cow<'a, RelativePath>, RelativePathBuf);

macro_rules! impl_cmp_str {
    ($lhs:ty, $rhs:ty) => {
        impl<'a, 'b> PartialEq<$rhs> for $lhs {
            #[inline]
            fn eq(&self, other: &$rhs) -> bool {
                <RelativePath as PartialEq>::eq(self, other.as_ref())
            }
        }

        impl<'a, 'b> PartialEq<$lhs> for $rhs {
            #[inline]
            fn eq(&self, other: &$lhs) -> bool {
                <RelativePath as PartialEq>::eq(self.as_ref(), other)
            }
        }

        impl<'a, 'b> PartialOrd<$rhs> for $lhs {
            #[inline]
            fn partial_cmp(&self, other: &$rhs) -> Option<cmp::Ordering> {
                <RelativePath as PartialOrd>::partial_cmp(self, other.as_ref())
            }
        }

        impl<'a, 'b> PartialOrd<$lhs> for $rhs {
            #[inline]
            fn partial_cmp(&self, other: &$lhs) -> Option<cmp::Ordering> {
                <RelativePath as PartialOrd>::partial_cmp(self.as_ref(), other)
            }
        }
    };
}

impl_cmp_str!(RelativePathBuf, str);
impl_cmp_str!(RelativePathBuf, &'a str);
impl_cmp_str!(RelativePathBuf, String);
impl_cmp_str!(RelativePath, str);
impl_cmp_str!(RelativePath, &'a str);
impl_cmp_str!(RelativePath, String);
impl_cmp_str!(&'a RelativePath, str);
impl_cmp_str!(&'a RelativePath, String);
