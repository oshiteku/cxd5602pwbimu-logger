// This is the lib interface for the crate
#[path = "lib/mod.rs"]
mod lib_inner;

// Re-export all types from the internal lib module
pub use lib_inner::*;