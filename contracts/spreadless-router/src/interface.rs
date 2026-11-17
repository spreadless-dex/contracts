use soroban_sdk::{contracttrait, contracttype, Address, BytesN, Env, String, Symbol, Vec};
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

/// SEP-40 asset identifier: a Stellar asset contract or an off-chain symbol.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub enum OracleAsset {
    Stellar(Address),
    Other(Symbol),
}

/// SEP-40 price record. `timestamp` is in seconds.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct PriceData {
    pub price: i128,
    pub timestamp: u64,
}

/// One asset on one SEP-40 oracle.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct PriceFeed {
    pub oracle: Address,
    pub asset: OracleAsset,
}

/// How to price one pool token in the peg's unit: `feed`, multiplied in order
/// by each of `quotes`. With no quotes, `feed` must already be denominated in
/// the peg's unit. With one, for example, a Stellar-DEX feed priced in USDC is
/// multiplied by USDC's USD price. Every quote must publish at the same
/// timestamps as `feed`. At most 2 quotes.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct TokenPriceSource {
    pub feed: PriceFeed,
    pub quotes: Vec<PriceFeed>,
}

/// Per-pool depeg protection, set by router governance. Pools without one are
/// unprotected.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct DepegGuardConfig {
    /// One source per pool token, in the pool's token order.
    pub sources: Vec<TokenPriceSource>,
    /// Target price, normalized to 14 decimals (1.0 == 100_000_000_000_000).
    pub peg_price: i128,
    /// Pause when a token is strictly further than this from the peg, in
    /// basis points of the peg. `1..=10_000`.
    pub max_deviation_bps: u32,
    /// Reject the latest reading when it is older than this, in seconds.
    /// `1..=86_400`.
    pub max_price_age: u64,
    /// How many of the most recent readings must all breach the threshold.
    /// `1..=10`.
    pub confirmations: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[contracttype]
pub enum DepegStatus {
    /// Every token is within the threshold (or not breached on every reading).
    Healthy,
    /// This call set the pool's protocol pause.
    Paused,
    /// The pool was already protocol-paused; no oracle was read.
    AlreadyPaused,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct DepegCheck {
    pub status: DepegStatus,
    /// The token furthest from the peg (`None` when `AlreadyPaused`).
    pub token: Option<Address>,
    /// Its latest price, normalized to 14 decimals.
    pub price: i128,
    /// Its smallest deviation across the confirmation readings, in bps.
    pub deviation_bps: u32,
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

    /// WASM used for pools created from now on. Existing pools keep their code
    /// until upgraded with `upgrade_pool`.
    fn set_pool_wasm_hash(e: Env, new_hash: BytesN<32>);

    /// Replace a registered pool's code in place (storage is preserved). The
    /// hash must already be uploaded. Reverts `PoolNotRegistered`.
    fn upgrade_pool(e: Env, pool_id: u32, new_wasm_hash: BytesN<32>);

    fn set_default_protocol_fee(e: Env, new_fee: u64);

    fn set_default_protocol_beneficiary(e: Env, new_beneficiary: Address);

    fn set_pool_protocol_fee(e: Env, pool_id: u32, new_fee: u64);

    fn set_pool_beneficiary(e: Env, pool_id: u32, new_beneficiary: Address);

    fn set_pool_amp_ramp(e: Env, pool_id: u32, target_factor: u32, duration: u64);

    fn stop_pool_amp_ramp(e: Env, pool_id: u32);

    fn pause_pool(e: Env, pool_id: u32);

    fn unpause_pool(e: Env, pool_id: u32);

    /// Protect a pool: from now on `check_depeg` may pause it. Replaces any
    /// existing configuration. Every feed must answer `decimals` and
    /// `lastprice` now. Owner only.
    /// Reverts: `PoolNotRegistered`, `InvalidDepegGuard`.
    fn set_depeg_guard(e: Env, pool_id: u32, config: DepegGuardConfig);

    /// Stop protecting a pool. Does not lift an existing pause. Owner only.
    /// Reverts: `PoolNotRegistered`, `DepegGuardNotSet`.
    fn remove_depeg_guard(e: Env, pool_id: u32);

    /// The pool's depeg protection, or `None` when it is unprotected.
    fn get_depeg_guard(e: Env, pool_id: u32) -> Option<DepegGuardConfig>;

    /// Permissionless. Read the pool's oracle feeds and set its protocol pause
    /// when some token has breached the threshold on every confirmation
    /// reading. Only governance can lift the pause (`unpause_pool`).
    ///
    /// Returns `AlreadyPaused` without reading oracles when the pool's protocol
    /// pause is already set, so keepers can call it freely.
    /// Reverts: `PoolNotRegistered`, `DepegGuardNotSet`, `StalePrice` (latest
    /// reading older than `max_price_age`), `PriceUnavailable` (missing,
    /// non-positive, out-of-order or unmatched readings), `PriceOverflow`.
    fn check_depeg(e: Env, pool_id: u32) -> DepegCheck;
}
