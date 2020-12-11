use crate::{
    get_exception, qjs, AsFunction, Ctx, IntoAtom, IntoJs, JsFn, Object, Result, SendWhenParallel,
    Undefined, Value,
};

impl<'js> Object<'js> {
    /// Define a property of an object
    ///
    /// ```
    /// # use rquickjs::{Runtime, Context, Object};
    /// # let rt = Runtime::new().unwrap();
    /// # let ctx = Context::full(&rt).unwrap();
    /// # ctx.with(|ctx| {
    /// # let obj = Object::new(ctx).unwrap();
    /// // Define readonly property without value
    /// obj.prop("no_val", ()).unwrap();
    /// // Define readonly property with value
    /// obj.prop("ro_str", ("Some const text",)).unwrap();
    /// // Define readonly property using getter
    /// obj.prop("ro_str_get", (|| "Some readable text",)).unwrap();
    /// // Define readonly property using getter and setter
    /// obj.prop("ro_str_get_set", (
    ///     || "Some text",
    ///     |new_val: String| { /* do something */ },
    /// )).unwrap();
    /// # })
    /// ```
    #[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "properties")))]
    pub fn prop<K: IntoAtom<'js>, P: AsProperty<'js, A>, A>(&self, key: K, prop: P) -> Result<()> {
        let ctx = self.0.ctx;
        let key = key.into_atom(ctx);
        let (flags, value, getter, setter) = prop.config(ctx)?;
        let flags = flags | (qjs::JS_PROP_THROW as PropertyFlags);
        unsafe {
            let res = qjs::JS_DefineProperty(
                ctx.ctx,
                self.0.as_js_value(),
                key.atom,
                value.as_js_value(),
                getter.as_js_value(),
                setter.as_js_value(),
                flags,
            );
            if res < 0 {
                return Err(get_exception(self.0.ctx));
            }
        }
        Ok(())
    }
}

pub type PropertyFlags = qjs::c_int;

const DEFAULT_FLAGS: PropertyFlags =
    (qjs::JS_PROP_HAS_CONFIGURABLE | qjs::JS_PROP_HAS_ENUMERABLE | qjs::JS_PROP_HAS_WRITABLE) as _;

/// The property flag trait
#[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "properties")))]
pub trait AsPropertyFlag {
    fn modify(flags: PropertyFlags) -> PropertyFlags;
}

macro_rules! as_property_flag_impls {
    ($($($t:ident)*,)*) => {
        $(
            impl<$($t,)*> AsPropertyFlag for ($($t,)*)
            where
                $($t: AsPropertyFlag,)*
            {
                fn modify(flags: PropertyFlags) -> PropertyFlags {
                    $(let flags = <$t>::modify(flags);)*
                    flags
                }
            }
        )*
    };

    ($($(#[$m:meta])* $t:ident => $op:tt $v:expr,)*) => {
        $(
            $(#[$m])*
            #[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "properties")))]
            pub struct $t;

            impl AsPropertyFlag for $t {
                fn modify(flags: PropertyFlags) -> PropertyFlags {
                    flags $op $v as PropertyFlags
                }
            }
        )*
    }
}

as_property_flag_impls! {
    ,
    A,
    A B,
    A B C,
    A B C D,
    A B C D E,
}

as_property_flag_impls! {
    /// Define the property as no configurable
    NoConfigurable => & !qjs::JS_PROP_HAS_CONFIGURABLE,
    /// Define the property as no enumerable
    NoEnumerable => & !qjs::JS_PROP_HAS_ENUMERABLE,
    /// Define the property as no writable
    NoWritable => & !qjs::JS_PROP_HAS_WRITABLE,
}

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

macro_rules! as_property_impls {
    ($($(#[$m:meta])* $($t:ident [$($c:tt)*])*: $($a:ident)* => [$($f:tt)*] $v:tt $g:tt $s:tt,)*) => {
        $(
            $(#[$m])*
            impl<'js, $($t,)* $($a,)*> AsProperty<'js, ($($t,)* $($a,)*)> for ($($t,)*)
            where
                $($t: $($c)*,)*
            {
                fn config(self, _ctx: Ctx<'js>) -> Result<(PropertyFlags, Value<'js>, Value<'js>, Value<'js>)> {
                    Ok((
                        as_property_impls!(@flag $($f)*),
                        as_property_impls!(@val self _ctx $v),
                        as_property_impls!(@val self _ctx $g),
                        as_property_impls!(@val self _ctx $s),
                    ))
                }
            }
        )*
    };

    (@flag ) => { DEFAULT_FLAGS };
    (@flag F $($f:tt)*) => { F::modify(as_property_impls!(@flag $($f)*)) };
    (@flag T $($f:tt)*) => { (qjs::JS_PROP_HAS_VALUE as PropertyFlags) | as_property_impls!(@flag $($f)*) };
    (@flag G $($f:tt)*) => { (qjs::JS_PROP_HAS_GET as PropertyFlags) | as_property_impls!(@flag $($f)*) };
    (@flag S $($f:tt)*) => { (qjs::JS_PROP_HAS_SET as PropertyFlags) | as_property_impls!(@flag $($f)*) };
    (@flag RO $($f:tt)*) => { NoWritable::modify(as_property_impls!(@flag $($f)*)) };

    (@val $this:ident $ctx:ident _) => { Undefined.into_js($ctx)? };
    (@val $this:ident $ctx:ident T) => { $this.0.into_js($ctx)? };
    (@val $this:ident $ctx:ident G) => { JsFn::new("get", $this.0).into_js($ctx)? };
    (@val $this:ident $ctx:ident S) => { JsFn::new("set", $this.1).into_js($ctx)? };
}

as_property_impls! {
    /// Undefined property
    : => [] _ _ _,

    /// Value as a property
    T [IntoJs<'js>]: => [T] T _ _,
    /// Value with flags as a property
    T [IntoJs<'js>]
    F [AsPropertyFlag]: => [T F] T _ _,

    /// The property using getter only
    G [AsFunction<'js, A, R> + SendWhenParallel + 'static]: A R => [G RO] _ G _,
    /// The property using getter with flags
    G [AsFunction<'js, A, R> + SendWhenParallel + 'static]
    F [AsPropertyFlag]: A R => [G RO F] _ G _,
    /// The property using getter and setter
    G [AsFunction<'js, GA, GR> + SendWhenParallel + 'static]
    S [AsFunction<'js, SA, SR> + SendWhenParallel + 'static]: GA GR SA SR => [G S] _ G S,
    /// The property using getter and setter with flags
    G [AsFunction<'js, GA, GR> + SendWhenParallel + 'static]
    S [AsFunction<'js, SA, SR> + SendWhenParallel + 'static]
    F [AsPropertyFlag]: GA GR SA SR => [G S F] _ G S,
}

#[cfg(test)]
mod test {
    use crate::*;

    #[test]
    fn property_with_undefined() {
        test_with(|ctx| {
            let obj = Object::new(ctx).unwrap();
            obj.prop("key", ()).unwrap();

            let _: () = obj.get("key").unwrap();

            if let Err(Error::Exception { message, .. }) = obj.set("key", "") {
                assert_eq!(message, "'key' is read-only");
            } else {
                panic!("Should fail");
            }
        });
    }

    #[test]
    fn property_with_value() {
        test_with(|ctx| {
            let obj = Object::new(ctx).unwrap();
            obj.prop("key", ("str",)).unwrap();

            let s: StdString = obj.get("key").unwrap();
            assert_eq!(s, "str");

            if let Err(Error::Exception { message, .. }) = obj.set("key", "") {
                assert_eq!(message, "'key' is read-only");
            } else {
                panic!("Should fail");
            }
        });
    }

    #[test]
    fn property_with_getter_only() {
        test_with(|ctx| {
            let obj = Object::new(ctx).unwrap();
            obj.prop("key", (|| "str",)).unwrap();

            let s: StdString = obj.get("key").unwrap();
            assert_eq!(s, "str");

            if let Err(Error::Exception { message, .. }) = obj.set("key", "") {
                assert_eq!(message, "no setter for property");
            } else {
                panic!("Should fail");
            }
        });
    }

    #[test]
    fn property_with_getter_and_setter() {
        test_with(|ctx| {
            let val = SafeRef::new(StdString::new());
            let obj = Object::new(ctx).unwrap();
            obj.prop(
                "key",
                (
                    {
                        let val = val.clone();
                        move || val.lock().clone()
                    },
                    {
                        let val = val.clone();
                        move |s| {
                            *val.lock() = s;
                        }
                    },
                ),
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
