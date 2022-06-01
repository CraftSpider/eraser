//! A more advanced erased box implementation, smaller but with a more complex implementation

use alloc::alloc::Layout;
use alloc::boxed::Box;
use core::ptr::{NonNull, Pointee};
use core::{fmt, mem, ptr};

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

            let layout = {
                let min_size = [
                    mem::size_of::<CommonInnerData>(),
                    mem::size_of::<T::Metadata>(),
                    mem::size_of_val(val),
                ]
                .into_iter()
                .sum();

                let align = [
                    mem::align_of::<CommonInnerData>(),
                    mem::align_of::<T::Metadata>(),
                    mem::align_of_val(val),
                ]
                .into_iter()
                .max()
                .unwrap();

                Layout::from_size_align(min_size, align)
                    .expect("Valid size/align pair")
                    .pad_to_align()
            };

            // SAFETY: Layout size is guaranteed non-zero, as it's a sum involving at least one
            //         non-ZST
            let alloced = unsafe { alloc::alloc::alloc(layout) };
            let new = NonNull::new(alloced).expect("Allocation returned nullptr");

            NonNull::from_raw_parts(new.cast(), val_meta)
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

            // Initialize the InnerData's drop and meta values. Note we use pointer dereference
            // without intermediate references to avoid possible UB due to references to uninit
            // memory

            // SAFETY: We just allocated this pointer, we know it's valid
            unsafe {
                (*new_ptr.as_ptr()).common = CommonInnerData::new::<T>();
            };
            // SAFETY: We just allocated this pointer, we know it's valid
            unsafe { (*new_ptr.as_ptr()).meta = meta };

            // Copy the possibly unsized value into our new InnerData
            let b_ptr = ptr.cast::<u8>();
            // SAFETY: We just allocated `new_ptr`, we know it's valid
            let new_data_ptr = unsafe { ptr::addr_of_mut!((*new_ptr.as_ptr()).data).cast::<u8>() };
            // SAFETY:
            // - `b_ptr` is from a Box::into_raw, it is valid and aligned
            // - `new_data_ptr` is from our new allocation, which is valid and aligned
            // - `b_ptr` cannot overlap `new_data_ptr` as they are unrelated allocations
            unsafe {
                ptr::copy_nonoverlapping(b_ptr, new_data_ptr, b_size);
            };

            // Deallocate the leaked value, as we've copied out of it
            // SAFETY:
            // - We got the pointer from a `Box` using the global allocator
            // - The layout is from `Layout::for_value`
            if b_layout.size() != 0 {
                unsafe {
                    alloc::alloc::dealloc(ptr.cast(), b_layout);
                }
            }

            new_ptr
        }
    }
}

use hidden::*;

/// # Safety
///
/// This function requires the input pointer be an erased pointer to an instance of `InnerData<T>`,
/// and valid to pass to `Box::from_raw` (Derived from `Box::leak` or allocated with the global
/// allocator and a correct layout).
unsafe fn drop_impl<T>(ptr: NonNull<()>)
where
    T: ?Sized + Pointee,
    InnerData<T>: Pointee<Metadata = T::Metadata>,
{
    // SAFETY: We assume our input pointers to an `InnerData<T>` by safety constraints. This means
    //         we know a metadata resides at an offset of 1 `CommonInnerData` from the start of the
    //         allocation, and that it is part of the same allocation
    let meta_ptr = ptr
        .cast::<CommonInnerData>()
        .as_ptr()
        .add(1)
        .cast::<T::Metadata>();
    // SAFETY: We assume our input pointer is valid by safety constraints
    let meta = *meta_ptr;
    let ptr = NonNull::<InnerData<T>>::from_raw_parts(ptr, meta);
    // SAFETY: We assume out input pointer is from `Box::into_raw` by safety constraints
    Box::from_raw(ptr.as_ptr());
}

#[repr(C)]
struct CommonInnerData {
    drop: unsafe fn(NonNull<()>),
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
        // SAFETY: `inner` points to a valid `InnerData<T>`, which we know contains a `T::Metadata`
        //         at an offset of 1 `CommonInnerData` from the start of the allocation, and that it
        //         is part of the same allocation
        let meta_ptr = unsafe {
            self.inner
                .as_ptr()
                .cast::<CommonInnerData>()
                .add(1)
                .cast::<T::Metadata>()
        };

        // SAFETY: Our inner pointer is guaranteed valid and safe to dereference
        let meta = unsafe { *meta_ptr };

        NonNull::from_raw_parts(self.inner, meta)
    }

    /// Get a pointer to the value stored in this `ThinErasedBox`. This pointer is guaranteed
    /// correctly aligned and dereferencable, until this box is dropped.
    ///
    /// # Safety
    ///
    /// The provided `T` must be the same type as originally stored in the box
    pub unsafe fn reify_ptr<T: ?Sized + Pointee>(&self) -> NonNull<T>
    where
        InnerData<T>: Pointee<Metadata = T::Metadata>,
    {
        // SAFETY: `inner_data()` will return a valid pointer, assuming `T` matches our invariants
        //         We don't hold these mutable references longer than this statement, they cannot
        //         exist at the same time as another.
        NonNull::from(&mut self.inner_data::<T>().as_mut().data)
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
        // SAFETY: `inner_data()` will return a valid pointer, assuming `T` matches our invariants
        let inner_ref = inner.as_ref();

        // Allocate space to move the unsized value into

        let layout = Layout::for_value(&inner_ref.data);
        let new_data = if layout.size() != 0 {
            // SAFETY: Layout is guaranteed not zero-sized, and correct for the value
            alloc::alloc::alloc(layout)
        } else {
            // A non-null aligned pointer to a zero-sized type
            layout.align() as *mut u8
        };

        // Copy the unsized value out of inner

        if layout.size() != 0 {
            // SAFETY:
            // - `inner_ref.data` is from a reference, so valid and aligned
            // - Size isn't zero, `new_data` is from a fresh allocation, so valid and aligned
            // - Pointers are from unrelated allocations, so cannot overlap
            ptr::copy_nonoverlapping(
                (&inner_ref.data as *const T).cast::<u8>(),
                new_data,
                layout.size(),
            );
        }

        // Create the return box from the new allocation

        // SAFETY: Our new pointer is guaranteed from a valid allocation for `Box::from_raw`, or
        //         a correctly aligned one if ZST
        let out = Box::from_raw(ptr::from_raw_parts_mut(new_data.cast(), inner_ref.meta));

        // Deallocate inner without dropping, as we copied out the value

        // SAFETY: Our pointer came from `InnerData<T>::alloc`, which is of the correct type and
        //         layout, and guaranteed valid up until this point
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
        // SAFETY: Matching safety invariants
        let ptr = self.reify_ptr();
        // SAFETY: Returned pointer is guaranteed valid, and we only access it with matching
        //         lifetimes to our own references
        ptr.as_ref()
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
        // SAFETY: Matching safety invariants
        let mut ptr = self.reify_ptr();
        // SAFETY: Returned pointer is guaranteed valid, and we only access it with matching
        //         lifetimes to our own references
        ptr.as_mut()
    }
}

impl fmt::Pointer for ThinErasedBox {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Pointer::fmt(&self.inner, f)
    }
}

impl fmt::Debug for ThinErasedBox {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ThinErasedBox")
            .field("inner", &self.inner)
            .finish_non_exhaustive()
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
            // SAFETY:
            // - Our inner pointer is guaranteed to point to a valid `InnerData<T>`
            // - InnerData starts with a valid CommonInnerData.
            // - We have unique reference access, and `inner` is only accessed with matching
            //   lifetimes to our references
            let data = unsafe { self.inner.cast::<CommonInnerData>().as_ref() };
            data.drop
        };

        // SAFETY: Our inner pointer came from `InnerData<T>::alloc`, which is of the correct type
        //         and layout to fulfill the `drop_impl` constraints
        unsafe { f(self.inner) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::String;
    use alloc::format;

    #[test]
    fn test_eb_drop() {
        ThinErasedBox::new::<i32>(1);
    }

    #[test]
    fn test_eb_reify_ptr() {
        let eb = ThinErasedBox::new::<u32>(1);
        let ptr1 = unsafe { eb.reify_ptr::<u32>() };
        let ptr2 = unsafe { eb.reify_ptr::<u32>() };

        (|_, _| {})(ptr1, ptr2);
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

    #[test]
    fn test_zst() {
        #[derive(Debug, PartialEq)]
        struct Foo;

        let eb = ThinErasedBox::new(Foo);
        assert_eq!(*unsafe { eb.reify_ref::<Foo>() }, Foo);
    }

    #[test]
    fn test_str() {
        let eb: ThinErasedBox = String::from("foo").into_boxed_str().into();
        assert_eq!(unsafe { eb.reify_ref::<str>() }, "foo");
    }

    #[test]
    fn test_dyn_val() {
        let eb: ThinErasedBox = (Box::new(123.45) as Box<dyn fmt::Debug>).into();
        assert_eq!(format!("{:?}", unsafe { eb.reify_ref::<dyn fmt::Debug>() }), "123.45");
    }

    #[test]
    fn test_slice() {
        let eb: ThinErasedBox = (Box::new([1, 2, 3]) as Box<[i32]>).into();
        assert_eq!(unsafe { eb.reify_ref::<[i32]>() }, [1, 2, 3]);
    }
}
