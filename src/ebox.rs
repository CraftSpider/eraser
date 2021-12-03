//! A standard erased box implementation, larger but simple implementation

use alloc::boxed::Box;
use core::mem;
use core::ptr::{NonNull, Pointee};

#[inline]
fn reify_ptr<T: ?Sized + Pointee>(data: NonNull<()>, meta: NonNull<()>) -> NonNull<T> {
    // SAFETY: Meta will be valid as it came from a Box::leak call
    let meta = *unsafe { meta.cast::<T::Metadata>().as_ref() };
    // SAFETY: Meta will have come from Box::leak of the correct type
    NonNull::<T>::from_raw_parts(data, meta)
}

#[inline]
fn reify_box<T: ?Sized + Pointee>(data: NonNull<()>, meta: NonNull<()>) -> Box<T> {
    let data = reify_ptr(data, meta);
    let meta_ptr = meta.cast::<T::Metadata>().as_ptr();
    // SAFETY: Meta will have come from Box::leak of the correct type
    unsafe { Box::from_raw(meta_ptr) };
    unsafe { Box::from_raw(data.as_ptr()) }
}

fn drop_erased<T: ?Sized + Pointee>(data: NonNull<()>, meta: NonNull<()>) {
    reify_box::<T>(data, meta);
}

/// An erased box, storing a (possibly unsized) value of unknown type. Creating one is safe,
/// but converting it back into any type is unsafe as it requires the user to know the type
/// stored in the box.
///
/// This box will always be three pointers wide, even for sized types, due to needing to store
/// an unknown metadata. If you want a box that will always be 1 pointer wide, look at
/// [`ThinErasedBox`](crate::ThinErasedBox)
pub struct ErasedBox {
    data: NonNull<()>,
    meta: NonNull<()>,
    drop: fn(NonNull<()>, NonNull<()>),
}

impl ErasedBox {
    /// Create a new `ErasedBox` from a value
    pub fn new<T>(val: T) -> ErasedBox {
        ErasedBox::from(Box::new(val))
    }

    /// Create a new `ErasedBox` from a pointer to an existing allocation
    ///
    /// # Safety
    ///
    /// The pointer must be valid, and the allocation should match that which can later be passed
    /// to `Box::from_raw`
    pub unsafe fn from_raw<T: ?Sized>(val: NonNull<T>) -> ErasedBox {
        let (data, meta) = val.to_raw_parts();
        let meta = NonNull::from(Box::leak(Box::new(meta))).cast::<()>();

        ErasedBox {
            data,
            meta,
            drop: drop_erased::<T>,
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

    /// Get a pointer to the value stored in this `ErasedBox`
    ///
    /// # Safety
    ///
    /// The provided `T` must be the same type as originally stored in the box
    pub unsafe fn reify_ptr<T: ?Sized>(&self) -> NonNull<T> {
        reify_ptr(self.data, self.meta)
    }

    /// Convert an `ErasedBox` back into a [`Box`] of the provided type
    ///
    /// # Safety
    ///
    /// The provided `T` must be the same type as originally stored in the box
    pub unsafe fn reify_box<T: ?Sized + Pointee>(self) -> Box<T> {
        let data = reify_box(self.data, self.meta);
        // Skip Drop call to avoid dropping the moved-out data
        mem::forget(self);
        data
    }

    /// Get a reference to the value stored in this `ErasedBox`
    ///
    /// # Safety
    ///
    /// The provided `T` must be the same type as originally stored in the box
    pub unsafe fn reify_ref<T: ?Sized>(&self) -> &T {
        self.reify_ptr().as_ref()
    }

    /// Get a mutable reference to the value stored in this `ErasedBox`
    ///
    /// # Safety
    ///
    /// The provided `T` must be the same type as originally stored in the box
    pub unsafe fn reify_mut<T: ?Sized>(&mut self) -> &mut T {
        self.reify_ptr().as_mut()
    }
}

impl<T: ?Sized> From<Box<T>> for ErasedBox {
    fn from(b: Box<T>) -> Self {
        let val = NonNull::from(Box::leak(b));
        // SAFETY: We just got this pointer from `Box::leak`, it's sure to uphold the requirements
        unsafe { ErasedBox::from_raw(val) }
    }
}

impl Drop for ErasedBox {
    fn drop(&mut self) {
        (self.drop)(self.data, self.meta)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_eb_drop() {
        ErasedBox::new::<i32>(1);
    }

    #[test]
    fn test_eb_reify_box() {
        unsafe { ErasedBox::new::<u32>(1).reify_box::<u32>() };
    }

    #[test]
    fn test_eb_reify_ref() {
        let eb = ErasedBox::new::<bool>(true);
        let val = unsafe { eb.reify_ref::<bool>() };
        assert_eq!(*val, true);
    }

    #[test]
    fn test_eb_reify_mut() {
        let mut eb = ErasedBox::new::<f32>(1.5);
        let val = unsafe { eb.reify_mut::<f32>() };
        assert_eq!(*val, 1.5);
        *val = 2.5;
        let val2 = unsafe { eb.reify_mut::<f32>() };
        assert_eq!(*val2, 2.5);
    }
}
