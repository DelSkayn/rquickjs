use std::{any::Any, mem::ManuallyDrop};

use crate::Ctx;

/// A trait for userdata which is stored in the runtime.
///
/// # Safety
/// For safe implementation of this trait none of its default implemented functions must be
/// overwritten and the type Static must be the same as the type UserData is being implemented for,
/// with the exception that the `'js` lifetime must be changed to a 'static lifetime. Se below for
/// an example of a correct implementation.
///
/// ## Example
/// ```
/// # use rquickjs::{Function, UserData};
///
/// struct MyUserData<'js>{
///     function: Function<'js>
/// }
///
/// unsafe impl<'js> UserData for MyUserData<'js>{
///     // The self type with the lifetime changed to static.
///     type Static = MyUserData<'static>;
/// }
///
/// ```
pub unsafe trait UserData<'js> {
    type Static: 'static + Any;

    /// Create the type.
    ///
    /// To avoid types being smuggled out, or into the runtime closure which are not allowed there
    /// userdata must be created by the runtime.
    fn create() -> Self;

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

    unsafe fn from_static(this: Self::Static) -> Self
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
