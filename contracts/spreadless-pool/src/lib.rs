#![no_std]

mod contract;
mod error;
mod math;
mod pool;

pub use contract::{SpreadlessPool, SpreadlessPoolClient};
pub use spreadless_pool_interface::{
    AmpControl, SpreadlessPoolInterface, SpreadlessPoolInterfaceClient,
};
