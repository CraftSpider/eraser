
#![feature(ptr_metadata, layout_for_ptr)]

pub mod ebox;
pub mod thin_ebox;

pub use ebox::ErasedBox;
pub use thin_ebox::ThinErasedBox;
