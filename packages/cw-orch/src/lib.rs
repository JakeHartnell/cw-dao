#[cfg(not(target_arch = "wasm32"))]
mod core;
#[cfg(not(target_arch = "wasm32"))]
mod distribution;
#[cfg(not(target_arch = "wasm32"))]
mod external;
#[cfg(not(target_arch = "wasm32"))]
mod pre_propose;
#[cfg(not(target_arch = "wasm32"))]
mod proposal;
#[cfg(not(target_arch = "wasm32"))]
mod staking;
#[cfg(not(target_arch = "wasm32"))]
mod test_contracts;
#[cfg(not(target_arch = "wasm32"))]
mod voting;

#[cfg(not(target_arch = "wasm32"))]
pub use core::*;
#[cfg(not(target_arch = "wasm32"))]
pub use distribution::*;
#[cfg(not(target_arch = "wasm32"))]
pub use external::*;
#[cfg(not(target_arch = "wasm32"))]
pub use pre_propose::*;
#[cfg(not(target_arch = "wasm32"))]
pub use proposal::*;
#[cfg(not(target_arch = "wasm32"))]
pub use staking::*;
#[cfg(not(target_arch = "wasm32"))]
pub use test_contracts::*;
#[cfg(not(target_arch = "wasm32"))]
pub use voting::*;

#[cfg(feature = "wasm_test")]
#[cfg(test)]
pub mod tests;
