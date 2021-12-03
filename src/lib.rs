//! Implementations of type-erasure tools, which store fully non-generic data on the heap.
//!
//! Current features include:
//!
//! # Erased Boxes
//!
//! These are useful for cases where `Box<dyn Any>` doesn't fulfill your needs, including
//! non-`'static` data or wanting to store the data in one pointer even when unsized. As
//! a trade-off, there is no safe way to retrieve the data, as the user must already know the
//! type and lifetimes involved and verify them without the help of the compiler.
//!
//! # Erased Pointer
//!
//! The unowned equivalent to an erased box. Basically just a pointer-meta pair, that ensures
//! the meta is handled correctly on destruction.

#![feature(ptr_metadata, layout_for_ptr)]
#![warn(
    missing_docs,
    elided_lifetimes_in_paths,
    explicit_outlives_requirements,
    missing_abi,
    noop_method_call,
    pointer_structural_match,
    semicolon_in_expressions_from_macros,
    unused_import_braces,
    unused_lifetimes,
    clippy::cargo,
    clippy::missing_panics_doc,
    clippy::doc_markdown,
    clippy::ptr_as_ptr,
    clippy::cloned_instead_of_copied,
    clippy::unreadable_literal
)]
#![no_std]

extern crate alloc;

pub mod ebox;
pub mod eptr;
pub mod eref;
pub mod thin_ebox;

pub use ebox::ErasedBox;
pub use eptr::{ErasedNonNull, ErasedPtr};
pub use eref::{ErasedMut, ErasedRef};
pub use thin_ebox::ThinErasedBox;
