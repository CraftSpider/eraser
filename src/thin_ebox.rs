//! A more advanced erased box implementation, smaller but with a more complex implementation

use alloc::alloc::Layout;
use alloc::boxed::Box;
use core::ptr::{NonNull, Pointee};
use core::{mem, ptr};

// Ebox stuff

mod hidden {
    use super::*;

    /// The type stored on the heap by the box
    #[repr(C)]
    pub struct InnerData<T: ?Sized + Pointee> {
        pub(super) common: CommonInnerData,
        pub(super) meta: T::Metadata,
        pub(super) data: T,
    }

    impl<T: ?Sized + Pointee> InnerData<T> {
        fn alloc(val: &T) -> NonNull<InnerData<T>>
        where
            InnerData<T>: Pointee<Metadata = T::Metadata>,
        {
            let val_meta = (val as *const T).to_raw_parts().1;

            let layout = unsafe {
                Layout::for_value_raw(ptr::from_raw_parts::<InnerData<T>>(ptr::null(), val_meta))
            };

            let new = unsafe { NonNull::new_unchecked(alloc::alloc::alloc(layout)) };

            let new_meta = unsafe {
                *((&val_meta as *const T::Metadata).cast::<<InnerData<T> as Pointee>::Metadata>())
            };

            NonNull::from_raw_parts(new.cast(), new_meta)
        }

        pub(crate) fn new(val: Box<T>) -> NonNull<InnerData<T>>
        where
            InnerData<T>: Pointee<Metadata = T::Metadata>,
        {
            // Allocate a new InnerData for the value
            let new_ptr = Self::alloc(&*val);
            let b_layout = Layout::for_value(&*val);
            let b_size = mem::size_of_val(&*val);

            // Leak the value, get its pointer and metadata
            let (ptr, meta) = Box::into_raw(val).to_raw_parts();

            // Initialize the InnerData's drop and meta values
            unsafe {
                (*new_ptr.as_ptr()).common = CommonInnerData::new::<T>();
            };
            unsafe { (*new_ptr.as_ptr()).meta = meta };

            // Copy the possibly unsized value into our new InnerData
            let b_ptr = ptr.cast::<u8>();
            let new_data_ptr = unsafe { ptr::addr_of_mut!((*new_ptr.as_ptr()).data).cast::<u8>() };
            unsafe {
                ptr::copy_nonoverlapping(b_ptr, new_data_ptr, b_size);
            };

            // Deallocate the leaked value, as we've copied out of it
            unsafe {
                alloc::alloc::dealloc(ptr.cast(), b_layout);
            }

            new_ptr
        }
    }
}

use hidden::*;

fn drop_impl<T>(ptr: NonNull<()>)
where
    T: ?Sized + Pointee,
    InnerData<T>: Pointee<Metadata = T::Metadata>,
{
    let meta_ptr = unsafe {
        ptr.cast::<CommonInnerData>()
            .as_ptr()
            .add(1)
            .cast::<T::Metadata>()
    };
    let meta = unsafe { *meta_ptr };
    let ptr = NonNull::<InnerData<T>>::from_raw_parts(ptr, meta);
    unsafe { Box::from_raw(ptr.as_ptr()) };
}

#[repr(C)]
struct CommonInnerData {
    drop: fn(NonNull<()>),
}

impl CommonInnerData {
    fn new<T: ?Sized + Pointee>() -> CommonInnerData
    where
        InnerData<T>: Pointee<Metadata = T::Metadata>,
    {
        CommonInnerData {
            drop: drop_impl::<T>,
        }
    }
}

/// An erased box, storing a (possibly unsized) value of unknown type. Creating one is safe,
/// but converting it back into any type is unsafe as it requires the user to know the type
/// stored in the box.
///
/// This box will always be one pointer wide, storing the metadata on the heap alongside the
/// contained data. This requires more unsafety, but less indirection. For a simpler alternative,
/// take a look at [`ErasedBox`](crate::ErasedBox)
pub struct ThinErasedBox {
    /// Actually an [`InnerData`] of the type this box came from
    inner: NonNull<()>,
}

impl ThinErasedBox {
    /// Create a new `ThinErasedBox` from a value
    pub fn new<T: Pointee>(val: T) -> ThinErasedBox
    where
        InnerData<T>: Pointee<Metadata = T::Metadata>,
    {
        Box::new(val).into()
    }

    fn inner_data<T: ?Sized + Pointee>(&self) -> NonNull<InnerData<T>>
    where
        InnerData<T>: Pointee<Metadata = T::Metadata>,
    {
        let meta = unsafe {
            *self
                .inner
                .as_ptr()
                .cast::<CommonInnerData>()
                .add(1)
                .cast::<T::Metadata>()
        };

        NonNull::from_raw_parts(self.inner, meta)
    }

    /// Get a pointer to the value stored in this `ThinErasedBox`
    ///
    /// # Safety
    ///
    /// The provided `T` must be the same type as originally stored in the box
    pub unsafe fn reify_ptr<T: ?Sized + Pointee>(&self) -> NonNull<T>
    where
        InnerData<T>: Pointee<Metadata = T::Metadata>,
    {
        NonNull::from(&self.inner_data::<T>().as_ref().data)
    }

    /// Convert an `ThinErasedBox` back into a [`Box`] of the provided type
    ///
    /// # Safety
    ///
    /// The provided `T` must be the same type as originally stored in the box
    pub unsafe fn reify_box<T: ?Sized + Pointee>(self) -> Box<T>
    where
        InnerData<T>: Pointee<Metadata = T::Metadata>,
    {
        // Take ownership of inner, it will be dropped at the end of the function
        let inner = self.inner_data::<T>();
        let inner_ref = inner.as_ref();

        // Allocate space to move the unsized value into
        let layout = Layout::for_value(&inner_ref.data);
        let new_data = alloc::alloc::alloc(layout);

        // Copy the unsized value out of inner
        ptr::copy_nonoverlapping(
            (&inner_ref.data as *const T).cast::<u8>(),
            new_data,
            layout.size(),
        );

        // Create the return box from the new allocation
        let out = Box::from_raw(ptr::from_raw_parts_mut(new_data.cast(), inner_ref.meta));

        // Deallocate inner without dropping, as we copied out the value
        alloc::alloc::dealloc(inner.as_ptr().cast(), Layout::for_value(inner_ref));
        // Don't run our normal drop code on the inner we took ownership of
        mem::forget(self);

        out
    }

    /// Get a reference to the value stored in this `ThinErasedBox`
    ///
    /// # Safety
    ///
    /// The provided `T` must be the same type as originally stored in the box
    pub unsafe fn reify_ref<T: ?Sized + Pointee>(&self) -> &T
    where
        InnerData<T>: Pointee<Metadata = T::Metadata>,
    {
        self.reify_ptr().as_ref()
    }

    /// Get a mutable reference to the value stored in this `ThinErasedBox`
    ///
    /// # Safety
    ///
    /// The provided `T` must be the same type as originally stored in the box
    pub unsafe fn reify_mut<T: ?Sized + Pointee>(&mut self) -> &mut T
    where
        InnerData<T>: Pointee<Metadata = T::Metadata>,
    {
        self.reify_ptr().as_mut()
    }
}

impl<T: ?Sized + Pointee> From<Box<T>> for ThinErasedBox
where
    InnerData<T>: Pointee<Metadata = T::Metadata>,
{
    fn from(val: Box<T>) -> Self {
        let inner = InnerData::new(val);
        ThinErasedBox {
            inner: inner.cast(),
        }
    }
}

impl Drop for ThinErasedBox {
    fn drop(&mut self) {
        let f = {
            let data = unsafe { self.inner.cast::<CommonInnerData>().as_ref() };
            data.drop
        };

        f(self.inner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_eb_drop() {
        ThinErasedBox::new::<i32>(1);
    }

    #[test]
    fn test_eb_reify_box() {
        unsafe { ThinErasedBox::new::<u32>(1).reify_box::<u32>() };
    }

    #[test]
    fn test_eb_reify_ref() {
        let eb = ThinErasedBox::new::<bool>(true);
        let val = unsafe { eb.reify_ref::<bool>() };
        assert_eq!(*val, true);
    }

    #[test]
    fn test_eb_reify_mut() {
        let mut eb = ThinErasedBox::new::<f32>(1.5);
        let val = unsafe { eb.reify_mut::<f32>() };
        assert_eq!(*val, 1.5);
        *val = 2.5;
        let val2 = unsafe { eb.reify_mut::<f32>() };
        assert_eq!(*val2, 2.5);
    }
}
