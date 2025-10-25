#![allow(dead_code)]

#[cfg(feature = "std")]
mod std {
    extern crate std;

    pub type Vec<T> = std::vec::Vec<T>;
    pub type Box<T> = std::boxed::Box<T>;
}

mod alloc {
    extern crate alloc;

    pub type Vec<T> = alloc::vec::Vec<T>;
    pub type Box<T> = alloc::boxed::Box<T>;
}

#[cfg(feature = "std")]
pub use std::{Box, Vec};

#[cfg(not(feature = "std"))]
pub use alloc::{Box, Vec};
