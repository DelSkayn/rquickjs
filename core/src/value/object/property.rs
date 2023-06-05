use crate::{
    function::AsFunction, qjs, Ctx, Function, IntoAtom, IntoJs, Object, Result, Undefined, Value,
};

impl<'js> Object<'js> {
    /// Define a property of an object
    ///
    /// ```
    /// # use rquickjs::{Runtime, Context, Object, object::{Property, Accessor}};
    /// # let rt = Runtime::new().unwrap();
    /// # let ctx = Context::full(&rt).unwrap();
    /// # ctx.with(|ctx| {
    /// # let obj = Object::new(ctx).unwrap();
    /// // Define readonly property without value
    /// obj.prop("no_val", ()).unwrap();
    /// // Define readonly property with value
    /// obj.prop("ro_str", "Some const text").unwrap();
    /// // Define readonly property with value and make it to be writable
    /// obj.prop("ro_str2", Property::from("Some const text").writable()).unwrap();
    /// // Define readonly property using getter and make it to be enumerable
    /// obj.prop("ro_str_get", Accessor::from(|| "Some readable text").enumerable()).unwrap();
    /// // Define readonly property using getter and setter
    /// obj.prop("ro_str_get_set",
    ///     Accessor::from(|| "Some text")
    ///         .set(|new_val: String| { /* do something */ })
    /// ).unwrap();
    /// # })
    /// ```
    #[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "properties")))]
    pub fn prop<K, V, P>(&self, key: K, prop: V) -> Result<()>
    where
        K: IntoAtom<'js>,
        V: AsProperty<'js, P>,
    {
        let ctx = self.0.ctx;
        let key = key.into_atom(ctx)?;
        let (flags, value, getter, setter) = prop.config(ctx)?;
        let flags = flags | (qjs::JS_PROP_THROW as PropertyFlags);
        unsafe {
            let res = qjs::JS_DefineProperty(
                ctx.as_ptr(),
                self.0.as_js_value(),
                key.atom,
                value.as_js_value(),
                getter.as_js_value(),
                setter.as_js_value(),
                flags,
            );
            if res < 0 {
                return Err(self.0.ctx.raise_exception());
            }
        }
        Ok(())
    }
}

pub type PropertyFlags = qjs::c_int;

/// The property interface
#[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "properties")))]
pub trait AsProperty<'js, P> {
    /// Property configuration
    ///
    /// Returns the tuple which includes the following:
    /// - flags
    /// - value or undefined when no value is here
    /// - getter or undefined if the property hasn't getter
    /// - setter or undefined if the property hasn't setter
    fn config(self, ctx: Ctx<'js>) -> Result<(PropertyFlags, Value<'js>, Value<'js>, Value<'js>)>;
}

macro_rules! wrapper_impls {
	  ($($(#[$type_meta:meta])* $type:ident<$($param:ident),*>($($field:ident)*; $($flag:ident)*))*) => {
        $(
            $(#[$type_meta])*
            #[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "properties")))]
            #[derive(Debug, Clone, Copy)]
            pub struct $type<$($param),*> {
                flags: PropertyFlags,
                $($field: $param,)*
            }

            impl<$($param),*> $type<$($param),*> {
                $(wrapper_impls!{@flag $flag concat!("Make the property to be ", stringify!($flag))})*
            }
        )*
	  };

    (@flag $flag:ident $doc:expr) => {
        #[doc = $doc]
        #[must_use]
        pub fn $flag(mut self) -> Self {
            self.flags |= wrapper_impls!(@flag $flag);
            self
        }
    };

    (@flag $flag:ident) => { wrapper_impls!{@_flag $flag} as PropertyFlags };
    (@_flag configurable) => { qjs::JS_PROP_CONFIGURABLE };
    (@_flag enumerable) => { qjs::JS_PROP_ENUMERABLE };
    (@_flag writable) => { qjs::JS_PROP_WRITABLE };
    (@_flag value) => { qjs::JS_PROP_HAS_VALUE };
    (@_flag get) => { qjs::JS_PROP_HAS_GET };
    (@_flag set) => { qjs::JS_PROP_HAS_SET };
}

impl<'js, T> AsProperty<'js, T> for T
where
    T: IntoJs<'js>,
{
    fn config(self, ctx: Ctx<'js>) -> Result<(PropertyFlags, Value<'js>, Value<'js>, Value<'js>)> {
        Ok((
            wrapper_impls!(@flag value),
            self.into_js(ctx)?,
            Undefined.into_js(ctx)?,
            Undefined.into_js(ctx)?,
        ))
    }
}

wrapper_impls! {
    /// The data descriptor of a property
    Property<T>(value; writable configurable enumerable)
    /// The accessor descriptor of a readonly property
    Accessor<G, S>(get set; configurable enumerable)
}

/// Create property data descriptor from value
impl<T> From<T> for Property<T> {
    fn from(value: T) -> Self {
        Self {
            flags: wrapper_impls!(@flag value),
            value,
        }
    }
}

impl<'js, T> AsProperty<'js, T> for Property<T>
where
    T: IntoJs<'js>,
{
    fn config(self, ctx: Ctx<'js>) -> Result<(PropertyFlags, Value<'js>, Value<'js>, Value<'js>)> {
        Ok((
            self.flags,
            self.value.into_js(ctx)?,
            Undefined.into_js(ctx)?,
            Undefined.into_js(ctx)?,
        ))
    }
}

impl<G> From<G> for Accessor<G, ()> {
    fn from(get: G) -> Self {
        Self {
            get,
            set: (),
            flags: wrapper_impls!(@flag get),
        }
    }
}

impl<G> Accessor<G, ()> {
    /// Create accessor from getter
    pub fn new_get(get: G) -> Self {
        Self {
            flags: wrapper_impls!(@flag get),
            get,
            set: (),
        }
    }

    /// Add setter to accessor
    pub fn set<S>(self, set: S) -> Accessor<G, S> {
        Accessor {
            flags: self.flags | wrapper_impls!(@flag set),
            get: self.get,
            set,
        }
    }
}

impl<S> Accessor<(), S> {
    /// Create accessor from setter
    pub fn new_set(set: S) -> Self {
        Self {
            flags: wrapper_impls!(@flag set),
            get: (),
            set,
        }
    }

    /// Add getter to accessor
    pub fn get<G>(self, get: G) -> Accessor<G, S> {
        Accessor {
            flags: self.flags | wrapper_impls!(@flag get),
            get,
            set: self.set,
        }
    }
}

impl<G, S> Accessor<G, S> {
    /// Create accessor from getter and setter
    pub fn new(get: G, set: S) -> Self {
        Self {
            flags: wrapper_impls!(@flag get) | wrapper_impls!(@flag set),
            get,
            set,
        }
    }
}

/// A property with getter only
impl<'js, G, GA, GR> AsProperty<'js, (GA, GR, (), ())> for Accessor<G, ()>
where
    G: AsFunction<'js, GA, GR> + 'js,
{
    fn config(self, ctx: Ctx<'js>) -> Result<(PropertyFlags, Value<'js>, Value<'js>, Value<'js>)> {
        Ok((
            self.flags,
            Undefined.into_js(ctx)?,
            Function::new(ctx, self.get)?.into_value(),
            Undefined.into_js(ctx)?,
        ))
    }
}

/// A property with setter only
impl<'js, S, SA, SR> AsProperty<'js, ((), (), SA, SR)> for Accessor<(), S>
where
    S: AsFunction<'js, SA, SR> + 'js,
{
    fn config(self, ctx: Ctx<'js>) -> Result<(PropertyFlags, Value<'js>, Value<'js>, Value<'js>)> {
        Ok((
            self.flags,
            Undefined.into_js(ctx)?,
            Undefined.into_js(ctx)?,
            Function::new(ctx, self.set)?.into_value(),
        ))
    }
}

/// A property with getter and setter
impl<'js, G, GA, GR, S, SA, SR> AsProperty<'js, (GA, GR, SA, SR)> for Accessor<G, S>
where
    G: AsFunction<'js, GA, GR> + 'js,
    S: AsFunction<'js, SA, SR> + 'js,
{
    fn config(self, ctx: Ctx<'js>) -> Result<(PropertyFlags, Value<'js>, Value<'js>, Value<'js>)> {
        Ok((
            self.flags,
            Undefined.into_js(ctx)?,
            Function::new(ctx, self.get)?.into_value(),
            Function::new(ctx, self.set)?.into_value(),
        ))
    }
}

#[cfg(test)]
mod test {
    use crate::{object::*, *};

    #[test]
    fn property_with_undefined() {
        test_with(|ctx| {
            let obj = Object::new(ctx).unwrap();
            obj.prop("key", ()).unwrap();

            let _: () = obj.get("key").unwrap();

            if let Err(Error::Exception) = obj.set("key", "") {
                let exception = Exception::from_js(ctx, ctx.catch()).unwrap();
                assert_eq!(exception.message().as_deref(), Some("'key' is read-only"));
            } else {
                panic!("Should fail");
            }
        });
    }

    #[test]
    fn property_with_value() {
        test_with(|ctx| {
            let obj = Object::new(ctx).unwrap();
            obj.prop("key", "str").unwrap();

            let s: StdString = obj.get("key").unwrap();
            assert_eq!(s, "str");

            if let Err(Error::Exception) = obj.set("key", "") {
                let exception = Exception::from_js(ctx, ctx.catch()).unwrap();
                assert_eq!(exception.message().as_deref(), Some("'key' is read-only"));
            } else {
                panic!("Should fail");
            }
        });
    }

    #[test]
    fn property_with_data_descriptor() {
        test_with(|ctx| {
            let obj = Object::new(ctx).unwrap();
            obj.prop("key", Property::from("str")).unwrap();

            let s: StdString = obj.get("key").unwrap();
            assert_eq!(s, "str");
        });
    }

    #[test]
    #[should_panic(expected = "Exception generated by quickjs:  'key' is read-only")]
    fn property_with_data_descriptor_readonly() {
        test_with(|ctx| {
            let obj = Object::new(ctx).unwrap();
            obj.prop("key", Property::from("str")).unwrap();
            obj.set("key", "text")
                .catch(ctx)
                .map_err(|error| panic!("{}", error))
                .unwrap();
        });
    }

    #[test]
    fn property_with_data_descriptor_writable() {
        test_with(|ctx| {
            let obj = Object::new(ctx).unwrap();
            obj.prop("key", Property::from("str").writable()).unwrap();
            obj.set("key", "text").unwrap();
        });
    }

    #[test]
    #[should_panic(expected = "Exception generated by quickjs:  property is not configurable")]
    fn property_with_data_descriptor_not_configurable() {
        test_with(|ctx| {
            let obj = Object::new(ctx).unwrap();
            obj.prop("key", Property::from("str")).unwrap();
            obj.prop("key", Property::from(39))
                .catch(ctx)
                .map_err(|error| panic!("{}", error))
                .unwrap();
        });
    }

    #[test]
    fn property_with_data_descriptor_configurable() {
        test_with(|ctx| {
            let obj = Object::new(ctx).unwrap();
            obj.prop("key", Property::from("str").configurable())
                .unwrap();
            obj.prop("key", Property::from(39)).unwrap();
        });
    }

    #[test]
    fn property_with_data_descriptor_not_enumerable() {
        test_with(|ctx| {
            let obj = Object::new(ctx).unwrap();
            obj.prop("key", Property::from("str")).unwrap();
            let keys: Vec<StdString> = obj
                .own_keys(object::Filter::new().string())
                .collect::<Result<_>>()
                .unwrap();
            assert_eq!(keys.len(), 1);
            assert_eq!(&keys[0], "key");
            let keys: Vec<StdString> = obj.keys().collect::<Result<_>>().unwrap();
            assert_eq!(keys.len(), 0);
        });
    }

    #[test]
    fn property_with_data_descriptor_enumerable() {
        test_with(|ctx| {
            let obj = Object::new(ctx).unwrap();
            obj.prop("key", Property::from("str").enumerable()).unwrap();
            let keys: Vec<StdString> = obj.keys().collect::<Result<_>>().unwrap();
            assert_eq!(keys.len(), 1);
            assert_eq!(&keys[0], "key");
        });
    }

    #[test]
    fn property_with_getter_only() {
        test_with(|ctx| {
            let obj = Object::new(ctx).unwrap();
            obj.prop("key", Accessor::from(|| "str")).unwrap();

            let s: StdString = obj.get("key").unwrap();
            assert_eq!(s, "str");

            if let Err(Error::Exception) = obj.set("key", "") {
                let exception = Exception::from_js(ctx, ctx.catch()).unwrap();
                assert_eq!(
                    exception.message().as_deref(),
                    Some("no setter for property")
                );
            } else {
                panic!("Should fail");
            }
        });
    }

    #[test]
    fn property_with_getter_and_setter() {
        test_with(|ctx| {
            let val = Ref::new(Mut::new(StdString::new()));
            let obj = Object::new(ctx).unwrap();
            obj.prop(
                "key",
                Accessor::from({
                    let val = val.clone();
                    move || val.lock().clone()
                })
                .set({
                    let val = val.clone();
                    move |s| {
                        *val.lock() = s;
                    }
                }),
            )
            .unwrap();

            let s: StdString = obj.get("key").unwrap();
            assert_eq!(s, "");

            obj.set("key", "str").unwrap();
            assert_eq!(val.lock().clone(), "str");

            let s: StdString = obj.get("key").unwrap();
            assert_eq!(s, "str");

            obj.set("key", "").unwrap();
            let s: StdString = obj.get("key").unwrap();
            assert_eq!(s, "");
            assert_eq!(val.lock().clone(), "");
        });
    }
}
