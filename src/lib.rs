#![doc = include_str!("../README.md")]
/// Implementation for std::io
pub mod io;
pub mod needle;
/// Implementation for futures
#[cfg(feature = "futures")]
pub mod futures;
pub use crate::needle::Needle;
