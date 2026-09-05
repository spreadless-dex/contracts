use soroban_sdk::{contracttrait, contracttype, Address, BytesN, Env, String, Vec};
pub use spreadless_pool_interface::AmpControl;

/// One leg of an exact-input routed swap.
///
/// `token_in` is implicit: it is the router's `token_in` for the first hop and
/// the previous hop's `token_out` thereafter.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct SwapHop {
    pub pool_id: u32,
    pub token_out: Address,
}

#[contracttrait]
pub trait SpreadlessRouterInterface {
    /// Deploy and register a pool owned by `creator` and permanently controlled
    /// by this router for protocol administration.
    ///
    /// Creation is permissionless, but `creator` must authorize the call. The
    /// router deliberately allows duplicate token baskets and configurations.
    /// The current router protocol-fee defaults are copied into the pool.
    #[allow(clippy::too_many_arguments)]
    fn create_pool(
        e: Env,
        creator: Address,
        tokens: Vec<Address>,
        amp_factor: u32,
        amp_control: AmpControl,
        swap_fee: u64,
        max_caps: Vec<i128>,
        lp_max_supply: i128,
        lp_name: String,
        lp_symbol: String,
    ) -> Address;

    /// Run an exact-input swap through one or more registered pools.
    ///
    /// Every hop sends its output to `to`, then the next hop spends exactly
    /// that returned amount. Only the final hop applies `min_out`; if any hop
    /// fails, the entire route is reverted atomically. Returns the final output.
    ///
    /// Reverts with `EmptySwapPath` when `path` has no hops and
    /// `PoolNotRegistered` when any hop references an unknown pool ID.
    fn swap_exact_in(
        e: Env,
        to: Address,
        token_in: Address,
        path: Vec<SwapHop>,
        amount_in: i128,
        min_out: i128,
    ) -> i128;

    /// ID that will be assigned to the next created pool.
    fn next_pool_id(e: Env) -> u32;

    /// Resolve a registered pool ID. Reading it also refreshes its TTL.
    fn pool_at(e: Env, id: u32) -> Option<Address>;

    fn get_pool_wasm_hash(e: Env) -> BytesN<32>;

    /// Default copied into newly created pools; existing pools are unaffected.
    fn get_default_protocol_fee(e: Env) -> u64;

    /// Default copied into newly created pools; existing pools are unaffected.
    fn get_default_protocol_beneficiary(e: Env) -> Address;

    fn set_pool_wasm_hash(e: Env, new_hash: BytesN<32>);

    fn set_default_protocol_fee(e: Env, new_fee: u64);

    fn set_default_protocol_beneficiary(e: Env, new_beneficiary: Address);

    fn set_pool_protocol_fee(e: Env, pool_id: u32, new_fee: u64);

    fn set_pool_beneficiary(e: Env, pool_id: u32, new_beneficiary: Address);

    fn set_pool_amp_ramp(e: Env, pool_id: u32, target_factor: u32, duration: u64);

    fn pause_pool(e: Env, pool_id: u32);

    fn unpause_pool(e: Env, pool_id: u32);
}
