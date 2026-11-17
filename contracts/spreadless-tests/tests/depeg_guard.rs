//! Depeg guard: permissionless `check_depeg` reads SEP-40 feeds and sets the
//! pool's protocol pause; governance alone lifts it. The router is exercised as
//! a WASM black box against a mock oracle that mirrors Reflector's behavior
//! (newest-first records, one extra record from `prices`).

#![allow(clippy::too_many_arguments)]

use soroban_sdk::{
    contract, contractclient, contractimpl, contracttype,
    testutils::{Address as _, Ledger as _},
    token::{StellarAssetClient, TokenClient},
    Address, BytesN, Env, Error, String, Symbol, Vec,
};
use spreadless_pool_interface::{AmpControl, SpreadlessPoolInterfaceClient};

const POOL_WASM: &[u8] =
    include_bytes!("../../../target/wasm32v1-none/release/spreadless_pool.wasm");
const ROUTER_WASM: &[u8] =
    include_bytes!("../../../target/wasm32v1-none/release/spreadless_router.wasm");
/// The router code live on mainnet before the guard (deployments/mainnet.json).
const MAINNET_ROUTER_WASM: &[u8] = include_bytes!("../fixtures/router-mainnet-aec9b485.wasm");

const POOL_NOT_REGISTERED: u32 = 3;
const DEPEG_GUARD_NOT_SET: u32 = 7;
const INVALID_DEPEG_GUARD: u32 = 8;
const STALE_PRICE: u32 = 9;
const PRICE_UNAVAILABLE: u32 = 10;
const ENFORCED_PAUSE: u32 = 1000;

/// 1.0 at the guard's 14-decimal scale.
const ONE: i128 = 100_000_000_000_000;
const NOW: u64 = 1_794_915_542;
/// Reflector's slot just before `NOW`.
const LATEST: u64 = 1_794_915_300;
const SLOT: u64 = 300;

/// `micros` millionths of a unit, at 14 decimals: `usd(1_000_000) == ONE`.
fn usd(micros: i128) -> i128 {
    micros * 100_000_000
}

// ---------------------------------------------------------------------------
// Mirrors of contracts/spreadless-router/src/interface.rs
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub enum OracleAsset {
    Stellar(Address),
    Other(Symbol),
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct PriceData {
    pub price: i128,
    pub timestamp: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct PriceFeed {
    pub oracle: Address,
    pub asset: OracleAsset,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct TokenPriceSource {
    pub feed: PriceFeed,
    pub quotes: Vec<PriceFeed>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct DepegGuardConfig {
    pub sources: Vec<TokenPriceSource>,
    pub peg_price: i128,
    pub max_deviation_bps: u32,
    pub max_price_age: u64,
    pub confirmations: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[contracttype]
pub enum DepegStatus {
    Healthy,
    Paused,
    AlreadyPaused,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct DepegCheck {
    pub status: DepegStatus,
    pub token: Option<Address>,
    pub price: i128,
    pub deviation_bps: u32,
}

#[contractclient(name = "RouterClient")]
#[allow(dead_code)]
trait RouterInterface {
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
    fn next_pool_id(e: Env) -> u32;
    fn pool_at(e: Env, id: u32) -> Option<Address>;
    fn pause_pool(e: Env, pool_id: u32);
    fn unpause_pool(e: Env, pool_id: u32);
    fn set_depeg_guard(e: Env, pool_id: u32, config: DepegGuardConfig);
    fn remove_depeg_guard(e: Env, pool_id: u32);
    fn get_depeg_guard(e: Env, pool_id: u32) -> Option<DepegGuardConfig>;
    fn check_depeg(e: Env, pool_id: u32) -> DepegCheck;
}

#[contractclient(name = "UpgradeClient")]
#[allow(dead_code)]
trait UpgradeInterface {
    fn upgrade(e: Env, new_wasm_hash: BytesN<32>, operator: Address);
}

// ---------------------------------------------------------------------------
// Mock SEP-40 oracle
// ---------------------------------------------------------------------------

#[derive(Clone)]
#[contracttype]
enum MockKey {
    Decimals,
    Exact,
    History(OracleAsset),
}

#[contract]
pub struct MockOracle;

#[contractimpl]
impl MockOracle {
    pub fn set_decimals(e: Env, decimals: u32) {
        e.storage().instance().set(&MockKey::Decimals, &decimals);
    }

    /// `true`: `prices(n)` returns exactly `n` records (plain SEP-40).
    /// `false` (default): one extra record, as Reflector does.
    pub fn set_exact(e: Env, exact: bool) {
        e.storage().instance().set(&MockKey::Exact, &exact);
    }

    /// Replace an asset's history; `records` are newest first.
    pub fn set_history(e: Env, asset: OracleAsset, records: Vec<PriceData>) {
        e.storage()
            .instance()
            .set(&MockKey::History(asset), &records);
    }

    pub fn decimals(e: Env) -> u32 {
        e.storage().instance().get(&MockKey::Decimals).unwrap_or(14)
    }

    pub fn lastprice(e: Env, asset: OracleAsset) -> Option<PriceData> {
        Self::history(&e, asset).first()
    }

    pub fn prices(e: Env, asset: OracleAsset, records: u32) -> Option<Vec<PriceData>> {
        let history = Self::history(&e, asset);
        if history.is_empty() {
            return None;
        }
        let exact: bool = e.storage().instance().get(&MockKey::Exact).unwrap_or(false);
        let wanted = if exact { records } else { records + 1 };
        Some(history.slice(0..wanted.min(history.len())))
    }
}

impl MockOracle {
    fn history(e: &Env, asset: OracleAsset) -> Vec<PriceData> {
        e.storage()
            .instance()
            .get(&MockKey::History(asset))
            .unwrap_or(Vec::new(e))
    }
}

// ---------------------------------------------------------------------------
// Fixture
// ---------------------------------------------------------------------------

struct Fixture {
    env: Env,
    router: Address,
    owner: Address,
    pool: Address,
    user: Address,
    tokens: [Address; 3],
    /// USD-denominated feed (like Reflector's CEX/DEX feed).
    usd_oracle: Address,
    /// USDC-denominated feed (like Reflector's Stellar DEX feed).
    dex_oracle: Address,
}

impl Fixture {
    fn router(&self) -> RouterClient<'_> {
        RouterClient::new(&self.env, &self.router)
    }

    fn pool(&self) -> SpreadlessPoolInterfaceClient<'_> {
        SpreadlessPoolInterfaceClient::new(&self.env, &self.pool)
    }

    fn oracle(&self, address: &Address) -> MockOracleClient<'_> {
        MockOracleClient::new(&self.env, address)
    }

    fn symbol(&self, index: usize) -> OracleAsset {
        OracleAsset::Other(Symbol::new(&self.env, ["TKA", "TKB", "TKC"][index]))
    }

    fn usdc(&self) -> OracleAsset {
        OracleAsset::Other(Symbol::new(&self.env, "USDC"))
    }

    /// Readings newest first, one per slot ending at `latest`.
    fn feed(&self, oracle: &Address, asset: OracleAsset, latest: u64, prices: &[i128]) {
        let mut records = Vec::new(&self.env);
        for (i, price) in prices.iter().enumerate() {
            records.push_back(PriceData {
                price: *price,
                timestamp: latest - SLOT * i as u64,
            });
        }
        self.oracle(oracle).set_history(&asset, &records);
    }

    /// Give every token a healthy, current USD history on `usd_oracle`.
    fn healthy_usd_feeds(&self) {
        for i in 0..3 {
            self.feed(&self.usd_oracle, self.symbol(i), LATEST, &[ONE; 12]);
        }
    }

    fn direct_source(&self, index: usize) -> TokenPriceSource {
        TokenPriceSource {
            feed: PriceFeed {
                oracle: self.usd_oracle.clone(),
                asset: self.symbol(index),
            },
            quotes: Vec::new(&self.env),
        }
    }

    /// Token priced on the DEX feed in USDC, converted with USDC/USD.
    fn quoted_source(&self, index: usize) -> TokenPriceSource {
        TokenPriceSource {
            feed: PriceFeed {
                oracle: self.dex_oracle.clone(),
                asset: OracleAsset::Stellar(self.tokens[index].clone()),
            },
            quotes: Vec::from_array(
                &self.env,
                [PriceFeed {
                    oracle: self.usd_oracle.clone(),
                    asset: self.usdc(),
                }],
            ),
        }
    }

    /// The defaults proposed for mainnet: 2%, 15 minutes, 3 readings.
    fn config(&self, sources: &[TokenPriceSource]) -> DepegGuardConfig {
        let mut list = Vec::new(&self.env);
        for source in sources {
            list.push_back(source.clone());
        }
        DepegGuardConfig {
            sources: list,
            peg_price: ONE,
            max_deviation_bps: 200,
            max_price_age: 900,
            confirmations: 3,
        }
    }

    fn direct_config(&self) -> DepegGuardConfig {
        self.config(&[
            self.direct_source(0),
            self.direct_source(1),
            self.direct_source(2),
        ])
    }

    fn protect(&self) {
        self.healthy_usd_feeds();
        self.router().set_depeg_guard(&0, &self.direct_config());
    }

    fn swap_works(&self) -> bool {
        self.pool()
            .try_swap_exact_in(&self.user, &self.tokens[0], &self.tokens[1], &1_000_000, &0)
            .is_ok()
    }
}

fn fixture_with_router(router_wasm: &[u8]) -> Fixture {
    let env = Env::default();
    env.cost_estimate().budget().reset_unlimited();
    env.mock_all_auths();
    env.ledger().with_mut(|l| l.timestamp = NOW);

    let owner = Address::generate(&env);
    let creator = Address::generate(&env);
    let user = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let mut tokens = std::vec![
        env.register_stellar_asset_contract_v2(token_admin.clone())
            .address(),
        env.register_stellar_asset_contract_v2(token_admin.clone())
            .address(),
        env.register_stellar_asset_contract_v2(token_admin.clone())
            .address(),
    ];
    tokens.sort();
    let tokens: [Address; 3] = [tokens[0].clone(), tokens[1].clone(), tokens[2].clone()];

    let pool_wasm_hash = env.deployer().upload_contract_wasm(POOL_WASM);
    let router = env.register(
        router_wasm,
        (
            owner.clone(),
            pool_wasm_hash,
            100_000_000_u64,
            owner.clone(),
        ),
    );
    let mut token_list = Vec::new(&env);
    for token in &tokens {
        token_list.push_back(token.clone());
        StellarAssetClient::new(&env, token).mint(&user, &3_000_000_000);
    }
    let pool = RouterClient::new(&env, &router).create_pool(
        &creator,
        &token_list,
        &100,
        &AmpControl::ProtocolManaged,
        &3_000_000,
        &Vec::from_array(&env, [10_000_000_000; 3]),
        &1_000_000_000_000,
        &String::from_str(&env, "Guarded LP"),
        &String::from_str(&env, "GLP"),
    );
    SpreadlessPoolInterfaceClient::new(&env, &pool).deposit(
        &user,
        &Vec::from_array(&env, [500_000_000; 3]),
        &0,
    );

    let usd_oracle = env.register(MockOracle, ());
    let dex_oracle = env.register(MockOracle, ());

    Fixture {
        env,
        router,
        owner,
        pool,
        user,
        tokens,
        usd_oracle,
        dex_oracle,
    }
}

fn fixture() -> Fixture {
    fixture_with_router(ROUTER_WASM)
}

fn assert_contract_error<T, E>(result: Result<T, Result<Error, E>>, code: u32) {
    assert_eq!(
        result.err().and_then(|e| e.ok()),
        Some(Error::from_contract_error(code))
    );
}

// ---------------------------------------------------------------------------
// Optional protection and configuration
// ---------------------------------------------------------------------------

#[test]
fn pools_are_unprotected_until_governance_opts_in() {
    let f = fixture();
    assert_eq!(f.router().get_depeg_guard(&0), None);
    assert_contract_error(f.router().try_check_depeg(&0), DEPEG_GUARD_NOT_SET);

    f.protect();
    assert_eq!(f.router().get_depeg_guard(&0), Some(f.direct_config()));

    f.router().remove_depeg_guard(&0);
    assert_eq!(f.router().get_depeg_guard(&0), None);
    assert_contract_error(f.router().try_check_depeg(&0), DEPEG_GUARD_NOT_SET);
    assert_contract_error(f.router().try_remove_depeg_guard(&0), DEPEG_GUARD_NOT_SET);
}

#[test]
fn only_the_router_owner_configures_protection() {
    let f = fixture();
    f.healthy_usd_feeds();
    f.env.set_auths(&[]);

    assert!(f
        .router()
        .try_set_depeg_guard(&0, &f.direct_config())
        .is_err());
    assert_eq!(f.router().get_depeg_guard(&0), None);

    f.env.mock_all_auths();
    f.router().set_depeg_guard(&0, &f.direct_config());
    f.env.set_auths(&[]);
    assert!(f.router().try_remove_depeg_guard(&0).is_err());
    assert!(f.router().get_depeg_guard(&0).is_some());
}

#[test]
fn invalid_configurations_are_rejected() {
    let f = fixture();
    f.healthy_usd_feeds();
    let good = f.direct_config();

    let mut cases: std::vec::Vec<DepegGuardConfig> = std::vec::Vec::new();
    let mut two_sources = good.clone();
    two_sources.sources.pop_back();
    cases.push(two_sources);
    cases.push(DepegGuardConfig {
        peg_price: 0,
        ..good.clone()
    });
    cases.push(DepegGuardConfig {
        max_deviation_bps: 0,
        ..good.clone()
    });
    cases.push(DepegGuardConfig {
        max_deviation_bps: 10_001,
        ..good.clone()
    });
    cases.push(DepegGuardConfig {
        max_price_age: 0,
        ..good.clone()
    });
    cases.push(DepegGuardConfig {
        max_price_age: 86_401,
        ..good.clone()
    });
    cases.push(DepegGuardConfig {
        confirmations: 0,
        ..good.clone()
    });
    cases.push(DepegGuardConfig {
        confirmations: 11,
        ..good.clone()
    });
    // A feed the oracle has no price for (a typo'd asset, for instance).
    let mut unknown_asset = good.clone();
    unknown_asset.sources.set(
        0,
        TokenPriceSource {
            feed: PriceFeed {
                oracle: f.usd_oracle.clone(),
                asset: OracleAsset::Other(Symbol::new(&f.env, "NOPE")),
            },
            quotes: Vec::new(&f.env),
        },
    );
    cases.push(unknown_asset);

    for config in cases {
        assert_contract_error(
            f.router().try_set_depeg_guard(&0, &config),
            INVALID_DEPEG_GUARD,
        );
    }
    assert_eq!(f.router().get_depeg_guard(&0), None);

    assert_contract_error(
        f.router().try_set_depeg_guard(&7, &good),
        POOL_NOT_REGISTERED,
    );
    assert_contract_error(f.router().try_check_depeg(&7), POOL_NOT_REGISTERED);
}

// ---------------------------------------------------------------------------
// Pausing
// ---------------------------------------------------------------------------

#[test]
fn healthy_prices_leave_the_pool_trading() {
    let f = fixture();
    f.protect();
    f.feed(&f.usd_oracle, f.symbol(1), LATEST, &[usd(1_000_500); 4]);

    let check = f.router().check_depeg(&0);

    assert_eq!(check.status, DepegStatus::Healthy);
    assert_eq!(check.token, Some(f.tokens[1].clone()));
    assert_eq!(check.price, usd(1_000_500));
    assert_eq!(check.deviation_bps, 5);
    assert!(!f.pool().paused());
    assert!(f.swap_works());
}

#[test]
fn sustained_depeg_pauses_and_only_governance_lifts_it() {
    let f = fixture();
    f.protect();
    f.feed(&f.usd_oracle, f.symbol(2), LATEST, &[usd(950_000); 4]);

    // Anyone may trigger it: no authorization at all.
    f.env.set_auths(&[]);
    let check = f.router().check_depeg(&0);

    assert_eq!(check.status, DepegStatus::Paused);
    assert_eq!(check.token, Some(f.tokens[2].clone()));
    assert_eq!(check.price, usd(950_000));
    assert_eq!(check.deviation_bps, 500);
    assert!(f.pool().is_protocol_paused());
    assert_contract_error(
        f.pool()
            .try_swap_exact_in(&f.user, &f.tokens[0], &f.tokens[1], &1_000_000, &0),
        ENFORCED_PAUSE,
    );

    // Outsiders and the pool's creator cannot lift it.
    assert!(f.router().try_unpause_pool(&0).is_err());
    f.env.mock_all_auths();
    assert!(f.pool().try_unpause().is_err());
    assert!(f.pool().is_protocol_paused());

    // Withdrawals stay open while paused.
    let shares = TokenClient::new(&f.env, &f.pool).balance(&f.user) / 10;
    assert!(f
        .pool()
        .try_withdraw(&f.user, &shares, &Vec::from_array(&f.env, [0; 3]))
        .is_ok());

    // Governance lifts it once the token is back on its peg.
    f.feed(&f.usd_oracle, f.symbol(2), LATEST + SLOT, &[ONE; 4]);
    f.env.ledger().with_mut(|l| l.timestamp = NOW + SLOT);
    f.router().unpause_pool(&0);
    assert_eq!(f.router().check_depeg(&0).status, DepegStatus::Healthy);
    assert!(f.swap_works());
}

#[test]
fn unpausing_during_a_depeg_requires_removing_or_loosening_the_guard() {
    let f = fixture();
    f.protect();
    f.feed(&f.usd_oracle, f.symbol(0), LATEST, &[usd(900_000); 4]);
    f.router().check_depeg(&0);

    // The depeg persists: a plain unpause is re-paused by the next check.
    f.router().unpause_pool(&0);
    assert_eq!(f.router().check_depeg(&0).status, DepegStatus::Paused);

    // Governance chooses to trade through it by removing protection.
    f.router().unpause_pool(&0);
    f.router().remove_depeg_guard(&0);
    assert_contract_error(f.router().try_check_depeg(&0), DEPEG_GUARD_NOT_SET);
    assert!(f.swap_works());
}

#[test]
fn a_single_deviating_reading_does_not_pause() {
    let f = fixture();
    f.protect();
    // Only the newest reading is off; the two before it are on peg.
    f.feed(
        &f.usd_oracle,
        f.symbol(1),
        LATEST,
        &[usd(800_000), usd(999_000), ONE, ONE],
    );

    let check = f.router().check_depeg(&0);

    assert_eq!(check.status, DepegStatus::Healthy);
    // A token's deviation is its smallest across the three readings, so the
    // spike counts as 0 bps and no token stands out.
    assert_eq!(check.deviation_bps, 0);
    assert!(!f.pool().paused());

    // Two of three is still not enough.
    f.feed(
        &f.usd_oracle,
        f.symbol(1),
        LATEST,
        &[usd(800_000), usd(800_000), ONE, ONE],
    );
    assert_eq!(f.router().check_depeg(&0).status, DepegStatus::Healthy);
}

#[test]
fn the_threshold_is_strict_in_both_directions() {
    let f = fixture();
    f.protect();

    // Exactly 2.00% below or above: not past the threshold.
    f.feed(&f.usd_oracle, f.symbol(0), LATEST, &[usd(980_000); 4]);
    assert_eq!(f.router().check_depeg(&0).status, DepegStatus::Healthy);
    f.feed(&f.usd_oracle, f.symbol(0), LATEST, &[usd(1_020_000); 4]);
    assert_eq!(f.router().check_depeg(&0).status, DepegStatus::Healthy);

    // One millionth further: past it.
    f.feed(&f.usd_oracle, f.symbol(0), LATEST, &[usd(1_020_001); 4]);
    let check = f.router().check_depeg(&0);
    assert_eq!(check.status, DepegStatus::Paused);
    assert_eq!(check.deviation_bps, 200);

    f.router().unpause_pool(&0);
    f.feed(&f.usd_oracle, f.symbol(0), LATEST, &[usd(979_999); 4]);
    assert_eq!(f.router().check_depeg(&0).status, DepegStatus::Paused);
}

#[test]
fn the_worst_token_is_reported() {
    let f = fixture();
    f.protect();
    f.feed(&f.usd_oracle, f.symbol(0), LATEST, &[usd(970_000); 4]);
    f.feed(&f.usd_oracle, f.symbol(2), LATEST, &[usd(940_000); 4]);

    let check = f.router().check_depeg(&0);

    assert_eq!(check.status, DepegStatus::Paused);
    assert_eq!(check.token, Some(f.tokens[2].clone()));
    assert_eq!(check.deviation_bps, 600);
}

// ---------------------------------------------------------------------------
// Stale and missing data is rejected, never acted on
// ---------------------------------------------------------------------------

#[test]
fn stale_prices_revert_without_pausing() {
    let f = fixture();
    f.protect();
    // Depegged, but the newest reading is 901 seconds old.
    f.feed(&f.usd_oracle, f.symbol(1), NOW - 901, &[usd(500_000); 4]);

    assert_contract_error(f.router().try_check_depeg(&0), STALE_PRICE);
    assert!(!f.pool().paused());

    // Exactly 900 seconds old is still acceptable.
    f.feed(&f.usd_oracle, f.symbol(1), NOW - 900, &[usd(500_000); 4]);
    assert_eq!(f.router().check_depeg(&0).status, DepegStatus::Paused);
}

#[test]
fn a_stale_healthy_token_still_blocks_the_check() {
    let f = fixture();
    f.protect();
    f.feed(&f.usd_oracle, f.symbol(0), LATEST, &[usd(500_000); 4]);
    f.feed(&f.usd_oracle, f.symbol(2), NOW - 2_000, &[ONE; 4]);

    // Every feed must be current; a partial view is not acted on.
    assert_contract_error(f.router().try_check_depeg(&0), STALE_PRICE);
    assert!(!f.pool().paused());
}

#[test]
fn missing_or_malformed_history_reverts_without_pausing() {
    let f = fixture();
    f.protect();
    let oracle = f.oracle(&f.usd_oracle);

    // Too few readings for three confirmations (Reflector would add one).
    f.feed(&f.usd_oracle, f.symbol(1), LATEST, &[usd(500_000); 1]);
    assert_contract_error(f.router().try_check_depeg(&0), PRICE_UNAVAILABLE);

    // No readings at all.
    oracle.set_history(&f.symbol(1), &Vec::new(&f.env));
    assert_contract_error(f.router().try_check_depeg(&0), PRICE_UNAVAILABLE);

    // Non-positive price.
    f.feed(&f.usd_oracle, f.symbol(1), LATEST, &[0, ONE, ONE, ONE]);
    assert_contract_error(f.router().try_check_depeg(&0), PRICE_UNAVAILABLE);

    // Out-of-order timestamps.
    let mut records = Vec::new(&f.env);
    for timestamp in [LATEST - SLOT, LATEST, LATEST - 2 * SLOT] {
        records.push_back(PriceData {
            price: usd(500_000),
            timestamp,
        });
    }
    oracle.set_history(&f.symbol(1), &records);
    assert_contract_error(f.router().try_check_depeg(&0), PRICE_UNAVAILABLE);

    assert!(!f.pool().paused());
}

#[test]
fn plain_sep40_oracles_without_the_extra_record_work() {
    let f = fixture();
    f.protect();
    f.oracle(&f.usd_oracle).set_exact(&true);
    f.feed(&f.usd_oracle, f.symbol(1), LATEST, &[usd(500_000); 3]);

    assert_eq!(f.router().check_depeg(&0).status, DepegStatus::Paused);
}

#[test]
fn an_already_paused_pool_is_left_alone_without_reading_oracles() {
    let f = fixture();
    f.protect();
    f.router().pause_pool(&0);
    // Oracles are now stale; it doesn't matter.
    f.env.ledger().with_mut(|l| l.timestamp = NOW + 10 * 86_400);

    let check = f.router().check_depeg(&0);

    assert_eq!(check.status, DepegStatus::AlreadyPaused);
    assert_eq!(check.token, None);
    assert!(f.pool().is_protocol_paused());
}

#[test]
fn an_owner_pause_does_not_stop_the_protocol_pause() {
    let f = fixture();
    f.protect();
    f.pool().pause();
    f.feed(&f.usd_oracle, f.symbol(0), LATEST, &[usd(900_000); 4]);

    assert_eq!(f.router().check_depeg(&0).status, DepegStatus::Paused);
    // Lifting the owner's flag no longer reopens the pool.
    f.pool().unpause();
    assert!(f.pool().paused());
}

// ---------------------------------------------------------------------------
// Prices quoted through USDC (Reflector's Stellar DEX feed)
// ---------------------------------------------------------------------------

fn quoted_fixture() -> Fixture {
    let f = fixture();
    f.healthy_usd_feeds();
    f.feed(&f.usd_oracle, f.usdc(), LATEST, &[ONE; 12]);
    for i in 0..2 {
        f.feed(
            &f.dex_oracle,
            OracleAsset::Stellar(f.tokens[i].clone()),
            LATEST,
            &[ONE; 12],
        );
    }
    // Two tokens priced in USDC on the DEX feed, one directly in USD.
    let config = f.config(&[f.quoted_source(0), f.quoted_source(1), f.direct_source(2)]);
    f.router().set_depeg_guard(&0, &config);
    f
}

#[test]
fn quoted_prices_are_converted_before_comparing() {
    let f = quoted_fixture();
    // 1.02 USDC per token, but USDC is at 0.98 USD: 0.9996 USD, healthy.
    f.feed(
        &f.dex_oracle,
        OracleAsset::Stellar(f.tokens[0].clone()),
        LATEST,
        &[usd(1_020_000); 4],
    );
    f.feed(&f.usd_oracle, f.usdc(), LATEST, &[usd(980_000); 4]);

    let check = f.router().check_depeg(&0);

    assert_eq!(check.status, DepegStatus::Healthy);
    assert!(!f.pool().paused());
}

#[test]
fn a_depeg_of_the_quoted_token_pauses() {
    let f = quoted_fixture();
    f.feed(
        &f.dex_oracle,
        OracleAsset::Stellar(f.tokens[1].clone()),
        LATEST,
        &[usd(950_000); 4],
    );

    let check = f.router().check_depeg(&0);

    assert_eq!(check.status, DepegStatus::Paused);
    assert_eq!(check.token, Some(f.tokens[1].clone()));
    assert_eq!(check.price, usd(950_000));
}

#[test]
fn a_depeg_of_the_quote_asset_itself_pauses() {
    let f = quoted_fixture();
    // The DEX feed still shows 1.0 USDC, but USDC lost its peg.
    f.feed(&f.usd_oracle, f.usdc(), LATEST, &[usd(900_000); 4]);

    let check = f.router().check_depeg(&0);

    assert_eq!(check.status, DepegStatus::Paused);
    assert_eq!(check.price, usd(900_000));
}

#[test]
fn quote_readings_must_share_timestamps() {
    let f = quoted_fixture();
    // The USDC feed missed a slot: its readings are shifted by one period.
    f.feed(&f.usd_oracle, f.usdc(), LATEST - SLOT, &[ONE; 4]);

    assert_contract_error(f.router().try_check_depeg(&0), PRICE_UNAVAILABLE);
}

#[test]
fn oracle_decimals_are_normalized() {
    let f = fixture();
    f.healthy_usd_feeds();
    let seven = f.env.register(MockOracle, ());
    let eighteen = f.env.register(MockOracle, ());
    MockOracleClient::new(&f.env, &seven).set_decimals(&7);
    MockOracleClient::new(&f.env, &eighteen).set_decimals(&18);
    f.feed(&seven, f.symbol(0), LATEST, &[9_990_000; 4]); // 0.999 at 7 decimals
    f.feed(
        &eighteen,
        f.symbol(1),
        LATEST,
        &[970_000_000_000_000_000; 4],
    ); // 0.97

    let source = |oracle: &Address, index: usize| TokenPriceSource {
        feed: PriceFeed {
            oracle: oracle.clone(),
            asset: f.symbol(index),
        },
        quotes: Vec::new(&f.env),
    };
    let config = f.config(&[source(&seven, 0), source(&eighteen, 1), f.direct_source(2)]);
    f.router().set_depeg_guard(&0, &config);

    let check = f.router().check_depeg(&0);

    assert_eq!(check.status, DepegStatus::Paused);
    assert_eq!(check.token, Some(f.tokens[1].clone()));
    assert_eq!(check.price, usd(970_000));
    assert_eq!(check.deviation_bps, 300);
}

// ---------------------------------------------------------------------------
// Cost and upgrade path
// ---------------------------------------------------------------------------

#[test]
fn the_heaviest_check_fits_the_default_budget() {
    let f = quoted_fixture();
    let mut config = f.router().get_depeg_guard(&0).unwrap();
    config.confirmations = 10;
    // Every token quoted: the most oracle calls a 3-token pool can make.
    config.sources = Vec::from_array(
        &f.env,
        [f.quoted_source(0), f.quoted_source(1), f.quoted_source(1)],
    );
    f.feed(
        &f.dex_oracle,
        OracleAsset::Stellar(f.tokens[1].clone()),
        LATEST,
        &[usd(900_000); 12],
    );
    f.router().set_depeg_guard(&0, &config);

    f.env.cost_estimate().budget().reset_default();
    assert_eq!(f.router().check_depeg(&0).status, DepegStatus::Paused);
    std::println!("{:#?}", f.env.cost_estimate().resources());
}

#[test]
fn upgrading_the_mainnet_router_keeps_its_pools_and_adds_the_guard() {
    let f = fixture_with_router(MAINNET_ROUTER_WASM);
    // The old router has no guard entrypoints.
    assert!(f.router().try_get_depeg_guard(&0).is_err());

    let new_hash = f.env.deployer().upload_contract_wasm(ROUTER_WASM);
    UpgradeClient::new(&f.env, &f.router).upgrade(&new_hash, &f.owner);

    assert_eq!(f.router().next_pool_id(), 1);
    assert_eq!(f.router().pool_at(&0), Some(f.pool.clone()));
    assert_eq!(f.pool().get_protocol_controller(), f.router);
    assert_eq!(f.router().get_depeg_guard(&0), None);

    f.protect();
    f.feed(&f.usd_oracle, f.symbol(0), LATEST, &[usd(900_000); 4]);
    assert_eq!(f.router().check_depeg(&0).status, DepegStatus::Paused);
    f.router().unpause_pool(&0);
    assert!(f.swap_works());
}
