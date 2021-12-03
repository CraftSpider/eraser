//! Erased pointer types, all are 3 pointers wide

use alloc::boxed::Box;
use core::ptr::{NonNull, Pointee};
use core::{fmt, ptr};

fn drop_impl<T: ?Sized + Pointee>(meta: NonNull<()>) {
    // SAFETY: We know that the meta came from a T of this type
    unsafe { Box::from_raw(meta.cast::<T::Metadata>().as_ptr()) };
}

/// An erased pointer, pointing to a (possibly unsized) value of unknown type. Creating one
/// is safe, but converting it back into any type is unsafe as it requires the user to know the type
/// stored behind the pointer.
///
/// This type will always be three pointers wide, even for sized types, due to needing to store
/// an unknown metadata.
pub struct ErasedPtr {
    data: *const (),
    meta: NonNull<()>,
    drop: fn(NonNull<()>),
}

impl ErasedPtr {
    /// Create a new `ErasedPtr` from an existing [`*const T`](*const)
    pub fn new<T: ?Sized>(val: *const T) -> ErasedPtr {
        let (data, meta) = val.to_raw_parts();
        let meta = NonNull::from(Box::leak(Box::new(meta))).cast();

        ErasedPtr {
            data,
            meta,
            drop: drop_impl::<T>,
        }
    }

    /// Get the raw pointer to the contained data
    pub fn raw_ptr(&self) -> *const () {
        self.data
    }

    /// Get the raw pointer to the contained data mutably
    pub fn raw_ptr_mut(&self) -> *mut () {
        self.data as *mut ()
    }

    /// Get the raw pointer to the meta of the contained data
    pub fn raw_meta_ptr(&self) -> NonNull<()> {
        self.meta
    }

    /// Get a pointer to the value stored in this `ErasedPtr`
    ///
    /// # Safety
    ///
    /// The provided `T` must be the same type as originally stored in the pointer
    pub unsafe fn reify_ptr<T: ?Sized + Pointee>(&self) -> *const T {
        let meta = self.meta.cast::<T::Metadata>().as_ref();
        ptr::from_raw_parts(self.data, *meta)
    }

    /// Get a mutable pointer to the value stored in this `ErasedPtr`
    ///
    /// # Safety
    ///
    /// The provided `T` must be the same type as originally stored in the pointer
    pub unsafe fn reify_ptr_mut<T: ?Sized + Pointee>(&self) -> *mut T {
        let meta = self.meta.cast::<T::Metadata>().as_ref();
        ptr::from_raw_parts_mut(self.data as *mut (), *meta)
    }
}

impl fmt::Pointer for ErasedPtr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Pointer::fmt(&self.data, f)
    }
}

impl fmt::Debug for ErasedPtr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ErasedPtr")
            .field("data", &self.data)
            .field("meta", &self.meta)
            .finish_non_exhaustive()
    }
}

impl<T: ?Sized> From<*const T> for ErasedPtr {
    fn from(val: *const T) -> Self {
        ErasedPtr::new(val)
    }
}

impl<T: ?Sized> From<*mut T> for ErasedPtr {
    fn from(val: *mut T) -> Self {
        ErasedPtr::new(val)
    }
}

impl Drop for ErasedPtr {
    fn drop(&mut self) {
        (self.drop)(self.meta)
    }
}

/// An erased non-null pointer, pointing to a (possibly unsized) value of unknown type. Creating one
/// is safe, but converting it back into any type is unsafe as it requires the user to know the type
/// stored behind the pointer.
///
/// This type will always be three pointers wide, even for sized types, due to needing to store
/// an unknown metadata.
pub struct ErasedNonNull {
    data: NonNull<()>,
    meta: NonNull<()>,
    drop: fn(NonNull<()>),
}

impl ErasedNonNull {
    /// Create a new `ErasedPtr` from a [`NonNull<T>`](NonNull)
    pub fn new<T: ?Sized>(val: NonNull<T>) -> ErasedNonNull {
        let (data, meta) = val.to_raw_parts();
        let meta = NonNull::from(Box::leak(Box::new(meta))).cast();

        ErasedNonNull {
            data,
            meta,
            drop: drop_impl::<T>,
        }
    }

    /// Get the raw pointer to the contained data
    pub fn raw_ptr(&self) -> NonNull<()> {
        self.data
    }

    /// Get the raw pointer to the meta of the contained data
    pub fn raw_meta_ptr(&self) -> NonNull<()> {
        self.meta
    }

    /// Get back the pointer stored in this `ErasedNonNull`
    ///
    /// # Safety
    ///
    /// The provided `T` must be the same type as originally stored in the pointer
    pub unsafe fn reify_ptr<T: ?Sized + Pointee>(&self) -> NonNull<T> {
        let meta = self.meta.cast::<T::Metadata>().as_ref();
        NonNull::from_raw_parts(self.data, *meta)
    }
}

impl fmt::Pointer for ErasedNonNull {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Pointer::fmt(&self.data, f)
    }
}

impl fmt::Debug for ErasedNonNull {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ErasedNonNull")
            .field("data", &self.data)
            .field("meta", &self.meta)
            .finish_non_exhaustive()
    }
}

impl<T: ?Sized> From<&T> for ErasedNonNull {
    fn from(val: &T) -> Self {
        ErasedNonNull::new(NonNull::from(val))
    }
}

impl<T: ?Sized> From<&mut T> for ErasedNonNull {
    fn from(val: &mut T) -> Self {
        ErasedNonNull::new(NonNull::from(val))
    }
}

impl Drop for ErasedNonNull {
    fn drop(&mut self) {
        (self.drop)(self.meta)
    }
}
