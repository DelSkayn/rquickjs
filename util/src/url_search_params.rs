use either_rs::Either;
use rquickjs::{
    atom::PredefinedAtom,
    class::Trace,
    function::{Func, Opt, This},
    Array, Coerced, Ctx, Function, Null, Object, Result,
};

/// The URLSearchParams interface defines utility methods to work with the query string of a URL.
#[derive(Default, Clone, Trace)]
#[rquickjs::class]
pub struct URLSearchParams {
    data: Vec<(String, String)>,
}

#[rquickjs::methods(rename_all = "camelCase")]
impl URLSearchParams {
    /// Returns a URLSearchParams object instance.
    #[qjs(constructor)]
    fn new(input: Opt<Either<String, Either<Array<'_>, Object<'_>>>>) -> Result<Self> {
        let Some(data) = input.0 else {
            return Ok(Self::default());
        };

        let data = match data {
            Either::Left(url) => {
                let query = match url.split_once('?') {
                    Some((_, query)) => query,
                    None => &url,
                };
                query
                    .split('&')
                    .map(|part| {
                        let mut parts = part.splitn(2, '=');
                        let name = parts.next().unwrap_or_default().to_string();
                        let value = parts.next().unwrap_or_default().to_string();
                        (name, value)
                    })
                    .collect()
            }
            Either::Right(Either::Left(array)) => {
                let mut data = Vec::new();
                for it in array {
                    let inner = it?.get::<Array<'_>>()?;
                    let name = inner.get::<Coerced<String>>(0)?;
                    let value = inner.get::<Coerced<String>>(1)?;
                    data.push((name.0, value.0));
                }
                data
            }
            Either::Right(Either::Right(iter_or_record)) => {
                match iter_or_record.get::<_, Function>("next") {
                    Ok(next) => {
                        let mut data = Vec::new();
                        loop {
                            let next = next.call::<_, Object<'_>>(())?;
                            if next.get::<_, bool>(PredefinedAtom::Done)? {
                                break;
                            }
                            let value = next.get::<_, Array<'_>>("value")?;
                            let name = value.get::<Coerced<String>>(0)?;
                            let value = value.get::<Coerced<String>>(1)?;
                            data.push((name.0, value.0));
                        }
                        data
                    }
                    Err(_) => {
                        let mut data = Vec::new();
                        for it in iter_or_record {
                            let (name, value) = it?;
                            let name = name.to_string()?;
                            let value = value.get::<Coerced<String>>()?;
                            data.push((name, value.0));
                        }
                        data
                    }
                }
            }
        };

        Ok(Self { data })
    }

    /// Returns an iterator allowing iteration through all key/value pairs contained in this object in the same order as they appear in the query string.
    #[qjs(rename = PredefinedAtom::SymbolIterator)]
    pub fn iterate<'js>(&self, ctx: Ctx<'js>, this: This<Self>) -> Result<Object<'js>> {
        self.entries(ctx, this)
    }

    /// Appends a specified key/value pair as a new search parameter.
    fn append(&mut self, name: Coerced<String>, value: Coerced<String>) {
        self.data.push((name.0, value.0));
    }

    /// Deletes search parameters that match a name, and optional value, from the list of all search parameters.
    fn delete(&mut self, name: Coerced<String>, value: Opt<Coerced<String>>) {
        self.data.retain(|(n, v)| {
            if n == &name.0 {
                if let Some(value) = &value.0 {
                    v != &value.0
                } else {
                    false
                }
            } else {
                true
            }
        });
    }

    /// Returns an iterator allowing iteration through all key/value pairs contained in this object in the same order as they appear in the query string.
    pub fn entries<'js>(&self, ctx: Ctx<'js>, this: This<Self>) -> Result<Object<'js>> {
        let res = Object::new(ctx)?;

        res.set("position", 0usize)?;
        res.set(
            PredefinedAtom::SymbolIterator,
            Func::from(|it: This<Object<'js>>| -> Result<Object<'js>> { Ok(it.0) }),
        )?;
        res.set(
            PredefinedAtom::Next,
            Func::from(
                move |ctx: Ctx<'js>, it: This<Object<'js>>| -> Result<Object<'js>> {
                    let position = it.get::<_, usize>("position")?;
                    let res = Object::new(ctx.clone())?;
                    if this.data.len() <= position {
                        res.set(PredefinedAtom::Done, true)?;
                    } else {
                        let (name, value) = &this.data[position];
                        res.set(
                            "value",
                            vec![
                                rquickjs::String::from_str(ctx.clone(), name),
                                rquickjs::String::from_str(ctx, value),
                            ],
                        )?;
                        it.set("position", position + 1)?;
                    }
                    Ok(res)
                },
            ),
        )?;
        Ok(res)
    }

    /// Allows iteration through all values contained in this object via a callback function.
    fn for_each<'js>(&self, ctx: Ctx<'js>, callback: Function<'js>) -> Result<()> {
        for (name, value) in &self.data {
            let ctx = ctx.clone();
            callback.call::<_, ()>((
                rquickjs::String::from_str(ctx.clone(), name),
                rquickjs::String::from_str(ctx, value),
            ))?;
        }
        Ok(())
    }

    /// Returns the first value associated with the given search parameter.
    fn get<'js>(
        &self,
        ctx: Ctx<'js>,
        name: Coerced<String>,
    ) -> Result<Either<rquickjs::String<'js>, Null>> {
        let Some((_, value)) = self.data.iter().find(|(n, _)| n == &name.0) else {
            return Ok(Either::Right(Null));
        };
        Ok(Either::Left(rquickjs::String::from_str(ctx, value)?))
    }

    /// Returns all the values associated with a given search parameter.
    fn get_all<'js>(
        &self,
        ctx: Ctx<'js>,
        name: Coerced<String>,
    ) -> Result<Vec<rquickjs::String<'js>>> {
        let values = self
            .data
            .iter()
            .filter(|(n, _)| n == &name.0)
            .map(|(_, v)| rquickjs::String::from_str(ctx.clone(), v))
            .collect::<Result<Vec<_>>>()?;
        Ok(values)
    }

    /// Returns a boolean value indicating if a given parameter, or parameter and value pair, exists.
    fn has(&self, name: Coerced<String>, value: Opt<Coerced<String>>) -> bool {
        self.data.iter().any(|(n, v)| {
            if n == &name.0 {
                if let Some(value) = &value.0 {
                    v == &value.0
                } else {
                    true
                }
            } else {
                false
            }
        })
    }

    /// Returns an iterator allowing iteration through all keys of the key/value pairs contained in this object.
    pub fn keys<'js>(&self, ctx: Ctx<'js>, this: This<Self>) -> Result<Object<'js>> {
        let res = Object::new(ctx)?;

        res.set("position", 0usize)?;
        res.set(
            PredefinedAtom::SymbolIterator,
            Func::from(|it: This<Object<'js>>| -> Result<Object<'js>> { Ok(it.0) }),
        )?;
        res.set(
            PredefinedAtom::Next,
            Func::from(
                move |ctx: Ctx<'js>, it: This<Object<'js>>| -> Result<Object<'js>> {
                    let position = it.get::<_, usize>("position")?;
                    let res = Object::new(ctx.clone())?;
                    if this.data.len() <= position {
                        res.set(PredefinedAtom::Done, true)?;
                    } else {
                        let (name, _) = &this.data[position];
                        res.set("value", rquickjs::String::from_str(ctx, name))?;
                        it.set("position", position + 1)?;
                    }
                    Ok(res)
                },
            ),
        )?;
        Ok(res)
    }

    /// Indicates the total number of search parameter entries.
    #[qjs(get)]
    fn size(&self) -> usize {
        self.data.len()
    }

    /// Sets the value associated with a given search parameter to the given value. If there are several values, the others are deleted.
    fn set(&mut self, name: Coerced<String>, mut value: Coerced<String>) {
        let mut found = false;
        self.data.retain_mut(|(n, v)| {
            if n == &name.0 {
                if !found {
                    std::mem::swap(v, &mut value.0);
                    found = true;
                    true
                } else {
                    false
                }
            } else {
                true
            }
        });
    }

    /// Sorts all key/value pairs, if any, by their keys.
    fn sort(&mut self) {
        self.data.sort();
    }

    /// Returns a string containing a query string suitable for use in a URL.
    #[allow(clippy::inherent_to_string)]
    fn to_string(&self) -> String {
        self.data
            .iter()
            .map(|(name, value)| format!("{}={}", name, value))
            .collect::<Vec<_>>()
            .join("&")
    }

    /// Returns an iterator allowing iteration through all values of the key/value pairs contained in this object.
    pub fn values<'js>(&self, ctx: Ctx<'js>, this: This<Self>) -> Result<Object<'js>> {
        let res = Object::new(ctx)?;

        res.set("position", 0usize)?;
        res.set(
            PredefinedAtom::SymbolIterator,
            Func::from(|it: This<Object<'js>>| -> Result<Object<'js>> { Ok(it.0) }),
        )?;
        res.set(
            PredefinedAtom::Next,
            Func::from(
                move |ctx: Ctx<'js>, it: This<Object<'js>>| -> Result<Object<'js>> {
                    let position = it.get::<_, usize>("position")?;
                    let res = Object::new(ctx.clone())?;
                    if this.data.len() <= position {
                        res.set(PredefinedAtom::Done, true)?;
                    } else {
                        let (_, value) = &this.data[position];
                        res.set("value", rquickjs::String::from_str(ctx, value))?;
                        it.set("position", position + 1)?;
                    }
                    Ok(res)
                },
            ),
        )?;
        Ok(res)
    }
}

#[cfg(test)]
mod tests {
    use rquickjs::{CatchResultExt, Class};

    use super::*;
    use crate::*;

    #[test]
    fn test_basic() {
        test_with(|ctx| {
            Class::<URLSearchParams>::define(&ctx.globals()).unwrap();
            let result = ctx
                .eval::<String, _>(
                    r#"
                const params = new URLSearchParams();
                params.append('a', '1');
                params.append('b', '2');
                params.append('a', '3');
                params.append('b', '4');
                params.append('c', 8);
                params.delete('a');
                params.delete('b', '2');
                params.toString()
            "#,
                )
                .catch(&ctx)
                .unwrap();
            assert_eq!(result, "b=4&c=8");
        })
    }

    #[test]
    fn test_iterate() {
        test_with(|ctx| {
            Class::<URLSearchParams>::define(&ctx.globals()).unwrap();
            let result = ctx
                .eval::<String, _>(
                    r#"
                const params = new URLSearchParams();
                params.append('a', '1');
                params.append('b', '2');
                params.append('a', '3');
                let res = [];
                for (const [name, value] of params) {
                    res.push(`${name}=${value}`);
                }
                res.join('&')
            "#,
                )
                .catch(&ctx)
                .unwrap();
            assert_eq!(result, "a=1&b=2&a=3");
        })
    }

    #[test]
    fn test_iterate_entries() {
        test_with(|ctx| {
            Class::<URLSearchParams>::define(&ctx.globals()).unwrap();
            let result = ctx
                .eval::<String, _>(
                    r#"
                const params = new URLSearchParams();
                params.append('a', '1');
                params.append('b', '2');
                params.append('a', '3');
                let res = [];
                for (const [name, value] of params.entries()) {
                    res.push(`${name}=${value}`);
                }
                res.join('&')
            "#,
                )
                .catch(&ctx)
                .unwrap();
            assert_eq!(result, "a=1&b=2&a=3");
        })
    }

    #[test]
    fn test_iterate_keys() {
        test_with(|ctx| {
            Class::<URLSearchParams>::define(&ctx.globals()).unwrap();
            let result = ctx
                .eval::<String, _>(
                    r#"
                const params = new URLSearchParams();
                params.append('a', '1');
                params.append('b', '2');
                params.append('a', '3');
                let res = [];
                for (const name of params.keys()) {
                    res.push(name);
                }
                res.join('&')
            "#,
                )
                .catch(&ctx)
                .unwrap();
            assert_eq!(result, "a&b&a");
        })
    }

    #[test]
    fn test_iterate_values() {
        test_with(|ctx| {
            Class::<URLSearchParams>::define(&ctx.globals()).unwrap();
            let result = ctx
                .eval::<String, _>(
                    r#"
                const params = new URLSearchParams();
                params.append('a', '1');
                params.append('b', '2');
                params.append('a', '3');
                let res = [];
                for (const name of params.values()) {
                    res.push(name);
                }
                res.join('&')
            "#,
                )
                .catch(&ctx)
                .unwrap();
            assert_eq!(result, "1&2&3");
        })
    }

    #[test]
    fn test_new_string() {
        test_with(|ctx| {
            Class::<URLSearchParams>::define(&ctx.globals()).unwrap();
            let result = ctx
                .eval::<String, _>(
                    r#"
                const params = new URLSearchParams('a=1&b=2&a=3');
                params.toString()
            "#,
                )
                .catch(&ctx)
                .unwrap();
            assert_eq!(result, "a=1&b=2&a=3");
        })
    }

    #[test]
    fn test_new_string_url() {
        test_with(|ctx| {
            Class::<URLSearchParams>::define(&ctx.globals()).unwrap();
            let result = ctx
                .eval::<String, _>(
                    r#"
                const params = new URLSearchParams('https://google.com?a=1&b=2&a=3');
                params.toString()
            "#,
                )
                .catch(&ctx)
                .unwrap();
            assert_eq!(result, "a=1&b=2&a=3");
        })
    }

    #[test]
    fn test_new_object() {
        test_with(|ctx| {
            Class::<URLSearchParams>::define(&ctx.globals()).unwrap();
            let result = ctx
                .eval::<String, _>(
                    r#"
                const params = new URLSearchParams({'a': 1, 'b': 2});
                params.toString()
            "#,
                )
                .catch(&ctx)
                .unwrap();
            assert_eq!(result, "a=1&b=2");
        })
    }

    #[test]
    fn test_new_array() {
        test_with(|ctx| {
            Class::<URLSearchParams>::define(&ctx.globals()).unwrap();
            let result = ctx
                .eval::<String, _>(
                    r#"
                const params = new URLSearchParams([['a', 1], ['b', 2], ['a', 3]]);
                params.toString()
            "#,
                )
                .catch(&ctx)
                .unwrap();
            assert_eq!(result, "a=1&b=2&a=3");
        })
    }

    #[test]
    fn test_new_iterator() {
        test_with(|ctx| {
            Class::<URLSearchParams>::define(&ctx.globals()).unwrap();
            let result = ctx
                .eval::<String, _>(
                    r#"
                const params = new URLSearchParams();
                params.append('a', '1');
                params.append('b', '2');
                params.append('a', '3');
                const params2 = new URLSearchParams(params.entries());
                params2.toString()
            "#,
                )
                .catch(&ctx)
                .unwrap();
            assert_eq!(result, "a=1&b=2&a=3");
        })
    }

    #[test]
    fn test_size() {
        test_with(|ctx| {
            Class::<URLSearchParams>::define(&ctx.globals()).unwrap();
            let result = ctx
                .eval::<usize, _>(
                    r#"
                const params = new URLSearchParams();
                params.append('a', '1');
                params.append('b', '2');
                params.append('a', '3');
                params.size
            "#,
                )
                .catch(&ctx)
                .unwrap();
            assert_eq!(result, 3);
        })
    }

    #[test]
    fn test_set() {
        test_with(|ctx| {
            Class::<URLSearchParams>::define(&ctx.globals()).unwrap();
            let result = ctx
                .eval::<String, _>(
                    r#"
                const params = new URLSearchParams();
                params.append('a', '1');
                params.append('b', '2');
                params.append('a', '3');
                params.set('a', '4');
                params.toString()
            "#,
                )
                .catch(&ctx)
                .unwrap();
            assert_eq!(result, "a=4&b=2");
        })
    }

    #[test]
    fn test_get() {
        test_with(|ctx| {
            Class::<URLSearchParams>::define(&ctx.globals()).unwrap();
            let result = ctx
                .eval::<String, _>(
                    r#"
                const params = new URLSearchParams();
                params.append('a', '1');
                params.append('b', '2');
                params.append('a', '3');
                params.get('a')
            "#,
                )
                .catch(&ctx)
                .unwrap();
            assert_eq!(result, "1");
        })
    }

    #[test]
    fn test_get_all() {
        test_with(|ctx| {
            Class::<URLSearchParams>::define(&ctx.globals()).unwrap();
            let result = ctx
                .eval::<String, _>(
                    r#"
                const params = new URLSearchParams();
                params.append('a', '1');
                params.append('b', '2');
                params.append('a', '3');
                params.getAll('a').join('&')
            "#,
                )
                .catch(&ctx)
                .unwrap();
            assert_eq!(result, "1&3");
        })
    }

    #[test]
    fn test_get_all_missing() {
        test_with(|ctx| {
            Class::<URLSearchParams>::define(&ctx.globals()).unwrap();
            let result = ctx
                .eval::<String, _>(
                    r#"
                const params = new URLSearchParams();
                params.append('a', '1');
                params.append('b', '2');
                params.append('a', '3');
                params.getAll('c').join('&')
            "#,
                )
                .catch(&ctx)
                .unwrap();
            assert_eq!(result, "");
        })
    }

    #[test]
    fn test_has() {
        test_with(|ctx| {
            Class::<URLSearchParams>::define(&ctx.globals()).unwrap();
            let result = ctx
                .eval::<bool, _>(
                    r#"
                const params = new URLSearchParams();
                params.append('a', '1');
                params.append('b', '2');
                params.append('a', '3');
                params.has('b')
            "#,
                )
                .catch(&ctx)
                .unwrap();
            assert!(result);
        })
    }

    #[test]
    fn test_has_value() {
        test_with(|ctx| {
            Class::<URLSearchParams>::define(&ctx.globals()).unwrap();
            let result = ctx
                .eval::<bool, _>(
                    r#"
                const params = new URLSearchParams();
                params.append('a', '1');
                params.append('b', '2');
                params.append('a', '3');
                params.has('b', 5)
            "#,
                )
                .catch(&ctx)
                .unwrap();
            assert!(!result);
        })
    }

    #[test]
    fn test_has_not() {
        test_with(|ctx| {
            Class::<URLSearchParams>::define(&ctx.globals()).unwrap();
            let result = ctx
                .eval::<bool, _>(
                    r#"
                const params = new URLSearchParams();
                params.append('a', '1');
                params.append('b', '2');
                params.append('a', '3');
                params.has('c')
            "#,
                )
                .catch(&ctx)
                .unwrap();
            assert!(!result);
        })
    }
}
