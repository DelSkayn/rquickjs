use std::{any::Any, mem::ManuallyDrop};

/// A trait for userdata which is stored in the runtime.
///
/// # Safety
/// For safe implementation of this trait none of its default implemented functions must be
/// overwritten and the type Static must be the correct type.
///
/// The static type must be the original type with the `'js` lifetime changed to `'static`.
///
/// All rquickjs javascript value have a `'js` lifetime, this lifetime is managed by rquickjs to
/// ensure that rquickjs values are used correctly. You can derive some lifetimes from this
/// lifetimes but only lifetimes on rquickjs structs can be soundly changed by this trait.
///
/// If changing a values type to its `UserData::Static` type would cause any borrow, non-rquickjs
/// struct with a '`js` struct to no have a different lifetime then the implementation is unsound.
///
/// ## Example
/// Below is a correctly implemented UserData, the `'js` on `Function` is directly derived from a
/// `Ctx<'js>`.
/// ```
/// # use rquickjs::{Function, runtime::UserData};
///
/// struct MyUserData<'js>{
///     function: Option<Function<'js>>
/// }
///
/// unsafe impl<'js> UserData<'js> for MyUserData<'js>{
///     // The self type with the lifetime changed to static.
///     type Static = MyUserData<'static>;
///
///     fn create() -> Self{
///         MyUserData{
///             function: None
///         }
///     }
/// }
/// ```
///
/// The example below is __unsound__ as it changes the `&'js` borrow to static.
///
/// ```no_run
/// # use rquickjs::{Function, runtime::UserData};
///
/// struct MyUserData<'js>{
///     // This is unsound!
///     // The &'js lifetime here is not a lifetime on a rquickjs struct.
///     function: &'js Function<'js>
/// }
///
/// unsafe impl<'js> UserData<'js> for MyUserData<'js>{
///     // The self type with the lifetime changed to static.
///     type Static = MyUserData<'static>;
///
///     fn create() -> Self{
///         MyUserData{
///             function: None
///         }
///     }
/// }
/// ```
///
///
pub unsafe trait UserData<'js> {
    type Static: 'static + Any;

    unsafe fn to_static(this: Self) -> Self::Static
    where
        Self: Sized,
    {
        assert_eq!(
            std::mem::size_of::<Self>(),
            std::mem::size_of::<Self::Static>(),
            "Invalid implementation of UserData, size_of::<Self>() != size_of::<Self::Static>()"
        );
        assert_eq!(
            std::mem::align_of::<Self>(),
            std::mem::align_of::<Self::Static>(),
            "Invalid implementation of UserData, align_of::<Self>() != align_of::<Self::Static>()"
        );

        union Trans<A, B> {
            from: ManuallyDrop<A>,
            to: ManuallyDrop<B>,
        }

        ManuallyDrop::into_inner(
            (Trans {
                from: ManuallyDrop::new(this),
            })
            .to,
        )
    }

    unsafe fn from_static_box(this: Box<Self::Static>) -> Box<Self>
    where
        Self: Sized,
    {
        assert_eq!(
            std::mem::size_of::<Self>(),
            std::mem::size_of::<Self::Static>(),
            "Invalid implementation of UserData, size_of::<Self>() != size_of::<Self::Static>()"
        );
        assert_eq!(
            std::mem::align_of::<Self>(),
            std::mem::align_of::<Self::Static>(),
            "Invalid implementation of UserData, align_of::<Self>() != align_of::<Self::Static>()"
        );

        // a super unsafe way to cast between types, This is nessacry because normal transmute will
        // complain that Self and Self::Static are not related so might not have the same size.
        //
        //
        union Trans<A, B> {
            from: ManuallyDrop<A>,
            to: ManuallyDrop<B>,
        }

        ManuallyDrop::into_inner(
            (Trans {
                from: ManuallyDrop::new(this),
            })
            .to,
        )
    }

    unsafe fn from_static_ref<'a>(this: &'a Self::Static) -> &'a Self
    where
        Self: Sized,
    {
        // a super unsafe way to cast between types, This is nessacry because normal transmute will
        // complain that Self and Self::Static are not related so might not have the same size.
        //
        //
        union Trans<A, B> {
            from: ManuallyDrop<A>,
            to: ManuallyDrop<B>,
        }

        ManuallyDrop::into_inner(
            (Trans {
                from: ManuallyDrop::new(this),
            })
            .to,
        )
    }
}
