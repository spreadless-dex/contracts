#![no_std]

mod contract;
mod depeg;
mod error;
mod interface;
mod storage;

pub use contract::{
    DefaultBeneficiaryUpdated, DefaultProtocolFeeUpdated, DepegGuardRemoved, DepegGuardSet,
    DepegPauseTriggered, PoolAmpRampSet, PoolBeneficiaryUpdated, PoolCreated, PoolPauseUpdated,
    PoolProtocolFeeUpdated, PoolUpgraded, PoolWasmUpdated, SpreadlessRouter,
    SpreadlessRouterClient,
};
pub use interface::{
    AmpControl, DepegCheck, DepegGuardConfig, DepegStatus, OracleAsset, PriceData, PriceFeed,
    SwapHop, TokenPriceSource,
};
