use soroban_sdk::{contracttrait, Address, BytesN, Env, String, Vec};

#[contracttrait]
pub trait PoolFactoryInterface {
    /// Deploy and register a pool owned by `creator`.
    ///
    /// Creation is permissionless, but `creator` must authorize the call. The
    /// factory deliberately allows duplicate token baskets and configurations.
    #[allow(clippy::too_many_arguments)]
    fn create_pool(
        e: Env,
        creator: Address,
        tokens: Vec<Address>,
        amp_factor: u32,
        swap_fee: u64,
        protocol_fee: u64,
        beneficiary: Address,
        max_caps: Vec<i128>,
        lp_max_supply: i128,
        lp_name: String,
        lp_symbol: String,
    ) -> Address;

    fn pool_count(e: Env) -> u32;

    fn pool_at(e: Env, index: u32) -> Option<Address>;

    fn is_pool(e: Env, pool: Address) -> bool;

    fn get_pool_wasm_hash(e: Env) -> BytesN<32>;

    fn set_pool_wasm_hash(e: Env, new_hash: BytesN<32>);
}
