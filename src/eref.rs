//! Erased reference types, all are 3 pointers wide

use core::fmt;
use core::marker::PhantomData;
use core::ptr::Pointee;

use crate::ErasedNonNull;

/// An erased reference, referencing a (possibly unsized) value of unknown type. Creating one is
/// safe, but converting it back into any type is unsafe as it requires the user to know the type
/// stored behind the reference.
///
/// This type will always be three pointers wide, even for sized types, due to needing to store
/// an unknown metadata.
pub struct ErasedRef<'a> {
    ptr: ErasedNonNull,
    _phantom: PhantomData<&'a ()>,
}

impl<'a> ErasedRef<'a> {
    /// Create a new `ErasedRef` from a reference
    pub fn new<T: ?Sized>(val: &'a T) -> ErasedRef<'a> {
        ErasedRef {
            ptr: ErasedNonNull::from(val),
            _phantom: PhantomData,
        }
    }

    /// Get the internal erased pointer of this reference
    pub fn as_ptr(&self) -> &ErasedNonNull {
        &self.ptr
    }

    /// Get back the reference stored in this `ErasedRef`
    ///
    /// # Safety
    ///
    /// The provided `T` must be the same type as originally stored in the reference
    pub unsafe fn reify_ref<T: ?Sized + Pointee>(&self) -> &T {
        self.ptr.reify_ptr::<T>().as_ref()
    }
}

impl fmt::Pointer for ErasedRef<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Pointer::fmt(&self.ptr, f)
    }
}

impl fmt::Debug for ErasedRef<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ErasedRef")
            .field("ptr", &self.ptr)
            .finish_non_exhaustive()
    }
}

/// An erased mutable reference, referencing a (possibly unsized) value of unknown type. Creating
/// one is safe, but converting it back into any type is unsafe as it requires the user to know the
/// type stored behind the reference.
///
/// This type will always be three pointers wide, even for sized types, due to needing to store
/// an unknown metadata.
pub struct ErasedMut<'a> {
    ptr: ErasedNonNull,
    _phantom: PhantomData<&'a mut ()>,
}

impl<'a> ErasedMut<'a> {
    /// Create a new `ErasedMute` from a reference
    pub fn new<T: ?Sized>(val: &'a mut T) -> ErasedMut<'a> {
        ErasedMut {
            ptr: ErasedNonNull::from(val),
            _phantom: PhantomData,
        }
    }

    /// Get the internal erased pointer of this reference
    pub fn as_ptr(&self) -> &ErasedNonNull {
        &self.ptr
    }

    /// Get back the mutable reference stored in this `ErasedRef`
    ///
    /// # Safety
    ///
    /// The provided `T` must be the same type as originally stored in the reference
    pub unsafe fn reify_ref<T: ?Sized + Pointee>(&mut self) -> &mut T {
        self.ptr.reify_ptr::<T>().as_mut()
    }
}

impl fmt::Pointer for ErasedMut<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Pointer::fmt(&self.ptr, f)
    }
}

impl fmt::Debug for ErasedMut<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ErasedRef")
            .field("ptr", &self.ptr)
            .finish_non_exhaustive()
    }
}
