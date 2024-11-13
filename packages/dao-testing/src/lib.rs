#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/README.md"))]

#[cfg(not(target_arch = "wasm32"))]
pub mod helpers;

#[cfg(not(target_arch = "wasm32"))]
pub mod contracts;

#[cfg(not(target_arch = "wasm32"))]
pub mod tests;
#[cfg(not(target_arch = "wasm32"))]
pub use tests::*;

#[cfg(not(target_arch = "wasm32"))]
pub mod suite;
#[cfg(not(target_arch = "wasm32"))]
pub use suite::*;

// Integration tests using an actual chain binary, requires
// the "test-tube" feature to be enabled
// cargo test --features test-tube
#[cfg(not(target_arch = "wasm32"))]
#[cfg(feature = "test-tube")]
pub mod test_tube;
