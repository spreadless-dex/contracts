use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    NextPoolIdOverflow = 1,
    InvalidProtocolFee = 2,
    PoolNotRegistered = 3,
    OwnershipRenunciationDisabled = 4,
}
