#![no_std]

// Tests run natively where std exists (the harness links it anyway); this makes
// it nameable from `#[cfg(test)]` modules despite the `no_std` contract build.
#[cfg(test)]
extern crate std;

mod contract;
mod error;
mod interface;
mod math;
mod pool;

#[cfg(test)]
mod test;

pub use contract::{LiquidityPool, LiquidityPoolClient};
