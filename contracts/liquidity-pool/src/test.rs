use soroban_sdk::{
    contract, contractimpl, contracttype,
    testutils::{Address as _, Ledger as _},
    token::{StellarAssetClient, TokenClient},
    vec, Address, Env, MuxedAddress, String,
};

use crate::{LiquidityPool, LiquidityPoolClient};

const UNIT: i128 = 1_000_000_000_000; // 1e12 raw of a 7-decimal SAC (1,000,000.0 tokens)

#[derive(Clone)]
#[contracttype]
enum FeeTokenKey {
    Balance(Address),
    FeeBps,
}

#[contract]
struct FeeToken;

#[contractimpl]
impl FeeToken {
    pub fn mint(e: Env, to: Address, amount: i128) {
        fee_token_add_balance(&e, &to, amount);
    }

    pub fn set_fee_bps(e: Env, fee_bps: u32) {
        e.storage().instance().set(&FeeTokenKey::FeeBps, &fee_bps);
    }

    pub fn allowance(_e: Env, _from: Address, _spender: Address) -> i128 {
        0
    }

    pub fn approve(
        _e: Env,
        _from: Address,
        _spender: Address,
        _amount: i128,
        _live_until_ledger: u32,
    ) {
    }

    pub fn balance(e: Env, id: Address) -> i128 {
        fee_token_balance(&e, &id)
    }

    pub fn transfer(e: Env, from: Address, to: MuxedAddress, amount: i128) {
        from.require_auth();
        fee_token_transfer(&e, &from, &to.address(), amount);
    }

    pub fn transfer_from(e: Env, spender: Address, from: Address, to: Address, amount: i128) {
        spender.require_auth();
        fee_token_transfer(&e, &from, &to, amount);
    }

    pub fn burn(e: Env, from: Address, amount: i128) {
        from.require_auth();
        fee_token_spend_balance(&e, &from, amount);
    }

    pub fn burn_from(e: Env, spender: Address, from: Address, amount: i128) {
        spender.require_auth();
        fee_token_spend_balance(&e, &from, amount);
    }

    pub fn decimals(_e: Env) -> u32 {
        7
    }

    pub fn name(e: Env) -> String {
        String::from_str(&e, "Fee Token")
    }

    pub fn symbol(e: Env) -> String {
        String::from_str(&e, "FEE")
    }
}

fn fee_token_balance(e: &Env, id: &Address) -> i128 {
    e.storage()
        .instance()
        .get(&FeeTokenKey::Balance(id.clone()))
        .unwrap_or(0)
}

fn fee_token_set_balance(e: &Env, id: &Address, amount: i128) {
    e.storage()
        .instance()
        .set(&FeeTokenKey::Balance(id.clone()), &amount);
}

fn fee_token_add_balance(e: &Env, id: &Address, amount: i128) {
    if amount < 0 {
        panic!("invalid amount");
    }
    let next = fee_token_balance(e, id)
        .checked_add(amount)
        .expect("balance overflow");
    fee_token_set_balance(e, id, next);
}

fn fee_token_spend_balance(e: &Env, id: &Address, amount: i128) {
    if amount < 0 {
        panic!("invalid amount");
    }
    let next = fee_token_balance(e, id)
        .checked_sub(amount)
        .filter(|b| *b >= 0)
        .expect("insufficient balance");
    fee_token_set_balance(e, id, next);
}

fn fee_token_transfer(e: &Env, from: &Address, to: &Address, amount: i128) {
    let fee_bps: u32 = e
        .storage()
        .instance()
        .get(&FeeTokenKey::FeeBps)
        .unwrap_or(0);
    let fee = amount * fee_bps as i128 / 10_000;
    fee_token_spend_balance(e, from, amount);
    fee_token_add_balance(e, to, amount - fee);
}

// Builds a 2-token stable pool (both 7-decimal Stellar asset contracts) with no
// protocol fee, mints `UNIT` of each to `user`, returns env + relevant addresses.
fn setup() -> (Env, Address, Address, Address, Address) {
    let (e, user, _beneficiary, pool_id, addr_a, addr_b) = setup_with(0);
    (e, user, pool_id, addr_a, addr_b)
}

// Like `setup` but with a configurable `protocol_fee`, and also returns the
// beneficiary address.
fn setup_with(protocol_fee: u64) -> (Env, Address, Address, Address, Address, Address) {
    let e = Env::default();
    e.mock_all_auths();

    let admin = Address::generate(&e);
    let user = Address::generate(&e);
    let beneficiary = Address::generate(&e);

    let sac_x = e.register_stellar_asset_contract_v2(admin.clone());
    let sac_y = e.register_stellar_asset_contract_v2(admin);
    // pool requires tokens in strictly ascending address order
    let (addr_a, addr_b) = if sac_x.address() < sac_y.address() {
        (sac_x.address(), sac_y.address())
    } else {
        (sac_y.address(), sac_x.address())
    };

    StellarAssetClient::new(&e, &addr_a).mint(&user, &UNIT);
    StellarAssetClient::new(&e, &addr_b).mint(&user, &UNIT);

    let tokens = vec![&e, addr_a.clone(), addr_b.clone()];
    let max_caps = vec![&e, 10_000_000_000_000_000i128, 10_000_000_000_000_000i128]; // 1e16 raw
    let pool_id = e.register(
        LiquidityPool,
        (
            user.clone(),                  // owner
            tokens,                        // tokens (sorted)
            100u32,                        // amp factor
            100_000u64,                    // swap fee = 0.01%
            protocol_fee,                  // protocol fee (1e9 == 100% of the swap fee)
            beneficiary.clone(),           // beneficiary
            max_caps,                      // per-token caps (raw)
            1_000_000_000_000_000_000i128, // LP max supply (1e18)
        ),
    );

    (e, user, beneficiary, pool_id, addr_a, addr_b)
}

fn setup_with_fee_token(fee_bps: u32) -> (Env, Address, Address, Address, Address) {
    let e = Env::default();
    e.mock_all_auths();

    let admin = Address::generate(&e);
    let user = Address::generate(&e);
    let beneficiary = Address::generate(&e);

    let sac = e.register_stellar_asset_contract_v2(admin);
    let addr_sac = sac.address();
    let addr_fee = e.register(FeeToken, ());
    let fee = FeeTokenClient::new(&e, &addr_fee);
    fee.set_fee_bps(&fee_bps);

    StellarAssetClient::new(&e, &addr_sac).mint(&user, &UNIT);
    fee.mint(&user, &UNIT);

    let (addr_a, addr_b) = if addr_fee < addr_sac {
        (addr_fee.clone(), addr_sac.clone())
    } else {
        (addr_sac.clone(), addr_fee.clone())
    };
    let tokens = vec![&e, addr_a, addr_b];
    let max_caps = vec![&e, 10_000_000_000_000_000i128, 10_000_000_000_000_000i128];
    let pool_id = e.register(
        LiquidityPool,
        (
            user.clone(),
            tokens,
            100u32,
            100_000u64,
            0u64,
            beneficiary,
            max_caps,
            1_000_000_000_000_000_000i128,
        ),
    );

    (e, user, pool_id, addr_fee, addr_sac)
}

fn assert_pool_balances_match_reserves(
    e: &Env,
    pool: &LiquidityPoolClient,
    pool_id: &Address,
    addr_a: &Address,
    addr_b: &Address,
) {
    let reserves = pool.get_reserves();
    assert_eq!(
        TokenClient::new(e, addr_a).balance(pool_id),
        reserves.get(0).unwrap()
    );
    assert_eq!(
        TokenClient::new(e, addr_b).balance(pool_id),
        reserves.get(1).unwrap()
    );
}

#[test]
fn deposit_then_full_withdraw_is_exact_roundtrip() {
    let (e, user, pool_id, addr_a, addr_b) = setup();
    let pool = LiquidityPoolClient::new(&e, &pool_id);
    let token_a = TokenClient::new(&e, &addr_a);
    let token_b = TokenClient::new(&e, &addr_b);

    // First (balanced) deposit: LP minted == invariant D, no fee.
    let lp = pool.deposit(&user, &vec![&e, UNIT, UNIT], &0i128);
    assert!(lp > 0);
    assert_eq!(pool.balance(&user), lp);
    assert_eq!(pool.total_supply(), lp);
    assert_eq!(pool.get_reserves(), vec![&e, UNIT, UNIT]);
    assert_eq!(token_a.balance(&pool_id), UNIT);
    assert_eq!(token_a.balance(&user), 0);

    // Withdraw everything -> get back exactly what was deposited (no fee on a
    // proportional exit).
    let out = pool.withdraw(&user, &lp, &vec![&e, 0i128, 0i128]);
    assert_eq!(out, vec![&e, UNIT, UNIT]);
    assert_eq!(pool.total_supply(), 0);
    assert_eq!(pool.get_reserves(), vec![&e, 0i128, 0i128]);
    assert_eq!(token_a.balance(&user), UNIT);
    assert_eq!(token_b.balance(&user), UNIT);
}

#[test]
fn two_balanced_deposits_accumulate_then_full_withdraw() {
    let (e, user, pool_id, addr_a, addr_b) = setup();
    let pool = LiquidityPoolClient::new(&e, &pool_id);

    let lp1 = pool.deposit(
        &user,
        &vec![&e, 600_000_000_000i128, 600_000_000_000i128],
        &0i128,
    );

    // Top the user up and add the rest, keeping the 1:1 ratio (still fee-free).
    StellarAssetClient::new(&e, &addr_a).mint(&user, &400_000_000_000);
    StellarAssetClient::new(&e, &addr_b).mint(&user, &400_000_000_000);
    let lp2 = pool.deposit(
        &user,
        &vec![&e, 400_000_000_000i128, 400_000_000_000i128],
        &0i128,
    );

    assert!(lp2 > 0);
    assert_eq!(pool.total_supply(), lp1 + lp2);
    assert_eq!(pool.balance(&user), lp1 + lp2);
    assert_eq!(pool.get_reserves(), vec![&e, UNIT, UNIT]);

    let out = pool.withdraw(&user, &(lp1 + lp2), &vec![&e, 0i128, 0i128]);
    assert_eq!(out, vec![&e, UNIT, UNIT]);
    assert_eq!(pool.total_supply(), 0);
}

#[test]
fn single_sided_deposit_mints_lp() {
    let (e, user, pool_id, addr_a, addr_b) = setup();
    let pool = LiquidityPoolClient::new(&e, &pool_id);

    pool.deposit(
        &user,
        &vec![&e, 500_000_000_000i128, 500_000_000_000i128],
        &0i128,
    );
    // Add only token A (exercises the unbalanced/fee path in the join math).
    let lp2 = pool.deposit(&user, &vec![&e, 500_000_000_000i128, 0i128], &0i128);

    assert!(lp2 > 0);
    assert_eq!(pool.get_reserves(), vec![&e, UNIT, 500_000_000_000i128]);
    let _ = (addr_a, addr_b);
}

#[test]
#[should_panic]
fn deposit_below_min_lp_out_reverts() {
    let (e, user, pool_id, _addr_a, _addr_b) = setup();
    let pool = LiquidityPoolClient::new(&e, &pool_id);
    // Demand absurdly high LP out -> slippage guard trips.
    pool.deposit(&user, &vec![&e, UNIT, UNIT], &i128::MAX);
}

#[test]
#[should_panic]
fn deposit_rejects_fee_on_transfer_token() {
    let (e, user, pool_id, _addr_fee, _addr_sac) = setup_with_fee_token(1_000);
    let pool = LiquidityPoolClient::new(&e, &pool_id);

    pool.deposit(&user, &vec![&e, UNIT, UNIT], &0i128);
}

#[test]
#[should_panic]
fn swap_rejects_fee_on_transfer_input() {
    let (e, user, pool_id, addr_fee, addr_sac) = setup_with_fee_token(0);
    let pool = LiquidityPoolClient::new(&e, &pool_id);
    let fee = FeeTokenClient::new(&e, &addr_fee);

    pool.deposit(&user, &vec![&e, UNIT, UNIT], &0i128);
    fee.mint(&user, &1_000_000_000i128);
    fee.set_fee_bps(&1_000);

    pool.swap_exact_in(&user, &addr_fee, &addr_sac, &1_000_000_000i128, &0i128);
}

#[test]
fn swap_exact_in_works() {
    let (e, user, pool_id, addr_a, addr_b) = setup();
    let pool = LiquidityPoolClient::new(&e, &pool_id);
    let token_a = TokenClient::new(&e, &addr_a);
    let token_b = TokenClient::new(&e, &addr_b);

    pool.deposit(&user, &vec![&e, UNIT, UNIT], &0i128);
    StellarAssetClient::new(&e, &addr_a).mint(&user, &1_000_000_000); // 1e9 more A to swap
    let a_before = token_a.balance(&user);
    let b_before = token_b.balance(&user);

    let amount_in = 1_000_000_000i128;
    let out = pool.swap_exact_in(&user, &addr_a, &addr_b, &amount_in, &0i128);

    assert!(out > 0);
    assert!(out < amount_in); // fee charged + slippage, ~1:1 price
    assert!(out > amount_in * 99 / 100); // but close (high amp, tiny fee)
    assert_eq!(token_a.balance(&user), a_before - amount_in);
    assert_eq!(token_b.balance(&user), b_before + out);

    let r = pool.get_reserves();
    assert_eq!(r.get(0).unwrap(), UNIT + amount_in); // A reserve up by the input (lossless at 7-dec)
    assert!(r.get(1).unwrap() < UNIT); // B reserve drained by the output
    assert_pool_balances_match_reserves(&e, &pool, &pool_id, &addr_a, &addr_b);
}

#[test]
fn swap_exact_out_works() {
    let (e, user, pool_id, addr_a, addr_b) = setup();
    let pool = LiquidityPoolClient::new(&e, &pool_id);
    let token_a = TokenClient::new(&e, &addr_a);
    let token_b = TokenClient::new(&e, &addr_b);

    pool.deposit(&user, &vec![&e, UNIT, UNIT], &0i128);
    StellarAssetClient::new(&e, &addr_a).mint(&user, &2_000_000_000); // enough to cover output + fee
    let a_before = token_a.balance(&user);
    let b_before = token_b.balance(&user);

    let amount_out = 1_000_000_000i128;
    let spent = pool.swap_exact_out(&user, &addr_a, &addr_b, &amount_out, &i128::MAX);

    assert_eq!(token_b.balance(&user), b_before + amount_out); // received EXACTLY amount_out
    assert_eq!(token_a.balance(&user), a_before - spent);
    assert!(spent > amount_out); // paid fee + slippage
    assert!(spent < amount_out * 101 / 100); // but close
    assert_pool_balances_match_reserves(&e, &pool, &pool_id, &addr_a, &addr_b);
}

#[test]
#[should_panic]
fn swap_exact_out_respects_max_in() {
    let (e, user, pool_id, addr_a, addr_b) = setup();
    let pool = LiquidityPoolClient::new(&e, &pool_id);
    pool.deposit(&user, &vec![&e, UNIT, UNIT], &0i128);
    StellarAssetClient::new(&e, &addr_a).mint(&user, &1_000_000_000);
    // The true cost exceeds amount_out, so max_in == amount_out must revert.
    pool.swap_exact_out(
        &user,
        &addr_a,
        &addr_b,
        &1_000_000_000i128,
        &1_000_000_000i128,
    );
}

#[test]
#[should_panic]
fn swap_same_token_reverts() {
    let (e, user, pool_id, addr_a, _addr_b) = setup();
    let pool = LiquidityPoolClient::new(&e, &pool_id);
    pool.deposit(&user, &vec![&e, UNIT, UNIT], &0i128);
    pool.swap_exact_in(&user, &addr_a, &addr_a, &1_000_000i128, &0i128);
}

#[test]
fn swap_routes_protocol_fee_to_beneficiary() {
    let (e, user, beneficiary, pool_id, addr_a, addr_b) = setup_with(500_000_000); // 50% of the swap fee
    let pool = LiquidityPoolClient::new(&e, &pool_id);
    let token_b = TokenClient::new(&e, &addr_b);

    pool.deposit(&user, &vec![&e, UNIT, UNIT], &0i128);
    StellarAssetClient::new(&e, &addr_a).mint(&user, &100_000_000_000); // 1e11 to make the fee visible
    assert_eq!(token_b.balance(&beneficiary), 0);

    pool.swap_exact_in(&user, &addr_a, &addr_b, &100_000_000_000i128, &0i128);

    // The protocol's cut of the swap fee was routed to the beneficiary in token B.
    assert!(token_b.balance(&beneficiary) > 0);
}

// --- admin ---

#[test]
#[should_panic]
fn paused_deposit_reverts() {
    let (e, user, pool_id, _a, _b) = setup();
    let pool = LiquidityPoolClient::new(&e, &pool_id);
    pool.pause();
    pool.deposit(&user, &vec![&e, UNIT, UNIT], &0i128); // blocked by #[when_not_paused]
}

#[test]
fn unpause_restores_ops() {
    let (e, user, pool_id, _a, _b) = setup();
    let pool = LiquidityPoolClient::new(&e, &pool_id);

    pool.pause();
    assert!(pool.paused());
    pool.unpause();
    assert!(!pool.paused());

    let lp = pool.deposit(&user, &vec![&e, UNIT, UNIT], &0i128);
    assert!(lp > 0);
}

#[test]
fn amp_ramp_interpolates_over_time() {
    let (e, _user, pool_id, _a, _b) = setup();
    let pool = LiquidityPoolClient::new(&e, &pool_id);
    assert_eq!(pool.get_amp(), 100); // initial static factor

    let start = e.ledger().timestamp();
    pool.set_amp_ramp(&200u32, &1000u64); // ramp 100 -> 200 over 1000s
    assert_eq!(pool.get_amp(), 100); // at start
    e.ledger().with_mut(|l| l.timestamp = start + 500);
    assert_eq!(pool.get_amp(), 150); // midpoint
    e.ledger().with_mut(|l| l.timestamp = start + 1000);
    assert_eq!(pool.get_amp(), 200); // end
}

#[test]
#[should_panic]
fn set_amp_ramp_rejects_invalid_factor() {
    let (e, _user, pool_id, _a, _b) = setup();
    let pool = LiquidityPoolClient::new(&e, &pool_id);
    pool.set_amp_ramp(&0u32, &0u64); // factor 0 < MIN_AMP
}

#[test]
fn set_swap_fee_changes_output() {
    let (e, user, pool_id, addr_a, addr_b) = setup();
    let pool = LiquidityPoolClient::new(&e, &pool_id);
    pool.deposit(&user, &vec![&e, UNIT, UNIT], &0i128);
    StellarAssetClient::new(&e, &addr_a).mint(&user, &2_000_000_000);

    let out_low_fee = pool.swap_exact_in(&user, &addr_a, &addr_b, &1_000_000_000i128, &0i128);
    pool.set_swap_fee(&10_000_000u64); // raise to max 1%
    let out_high_fee = pool.swap_exact_in(&user, &addr_a, &addr_b, &1_000_000_000i128, &0i128);

    assert!(out_high_fee < out_low_fee); // a bigger fee leaves the user with less
}

#[test]
#[should_panic]
fn set_swap_fee_rejects_out_of_range() {
    let (e, _user, pool_id, _a, _b) = setup();
    let pool = LiquidityPoolClient::new(&e, &pool_id);
    pool.set_swap_fee(&10_000_001u64); // > max 1%
}

#[test]
fn ownership_two_step_transfer() {
    let (e, user, pool_id, _a, _b) = setup();
    let pool = LiquidityPoolClient::new(&e, &pool_id);
    let new_owner = Address::generate(&e);

    assert_eq!(pool.get_owner(), Some(user.clone()));
    pool.transfer_ownership(&new_owner, &1000u32);
    assert_eq!(pool.get_owner(), Some(user)); // not transferred until accepted
    pool.accept_ownership();
    assert_eq!(pool.get_owner(), Some(new_owner));
}

#[test]
#[should_panic]
fn admin_requires_owner_auth() {
    // Deliberately NO `mock_all_auths`: the `#[only_owner]` gate then has no
    // authorization to satisfy, so an admin call must revert.
    let e = Env::default();
    let admin = Address::generate(&e);
    let owner = Address::generate(&e);
    let beneficiary = Address::generate(&e);

    let sac_x = e.register_stellar_asset_contract_v2(admin.clone());
    let sac_y = e.register_stellar_asset_contract_v2(admin);
    let (a, b) = if sac_x.address() < sac_y.address() {
        (sac_x.address(), sac_y.address())
    } else {
        (sac_y.address(), sac_x.address())
    };

    let pool_id = e.register(
        LiquidityPool,
        (
            owner,
            vec![&e, a, b],
            100u32,
            100_000u64,
            0u64,
            beneficiary,
            vec![&e, 10_000_000_000_000_000i128, 10_000_000_000_000_000i128],
            1_000_000_000_000_000_000i128,
        ),
    );
    let pool = LiquidityPoolClient::new(&e, &pool_id);
    pool.set_swap_fee(&200_000u64); // unauthorized -> reverts
}

// ===========================================================================
// Additional coverage. Paste into the same test module as the file above
// (it reuses the existing imports, `UNIT`, `setup`, and `setup_with`).
// ===========================================================================

// Like `setup` but lets a test pin the per-token cap and the LP max supply,
// so the cap / supply guards can be tripped on the first deposit.
fn setup_limited(
    token_cap: i128,
    lp_max_supply: i128,
) -> (Env, Address, Address, Address, Address) {
    let e = Env::default();
    e.mock_all_auths();
    let admin = Address::generate(&e);
    let user = Address::generate(&e);
    let beneficiary = Address::generate(&e);
    let sac_x = e.register_stellar_asset_contract_v2(admin.clone());
    let sac_y = e.register_stellar_asset_contract_v2(admin);
    let (addr_a, addr_b) = if sac_x.address() < sac_y.address() {
        (sac_x.address(), sac_y.address())
    } else {
        (sac_y.address(), sac_x.address())
    };
    StellarAssetClient::new(&e, &addr_a).mint(&user, &UNIT);
    StellarAssetClient::new(&e, &addr_b).mint(&user, &UNIT);
    let pool_id = e.register(
        LiquidityPool,
        (
            user.clone(),
            vec![&e, addr_a.clone(), addr_b.clone()],
            100u32,
            100_000u64,
            0u64,
            beneficiary,
            vec![&e, token_cap, token_cap],
            lp_max_supply,
        ),
    );
    (e, user, pool_id, addr_a, addr_b)
}

// --- deposit guards ---

#[test]
#[should_panic]
fn first_deposit_must_be_balanced_reverts() {
    let (e, user, pool_id, _a, _b) = setup();
    let pool = LiquidityPoolClient::new(&e, &pool_id);
    // A single-sided first deposit into an empty pool is not allowed.
    pool.deposit(&user, &vec![&e, UNIT, 0i128], &0i128);
}

#[test]
#[should_panic]
fn zero_deposit_reverts() {
    let (e, user, pool_id, _a, _b) = setup();
    let pool = LiquidityPoolClient::new(&e, &pool_id);
    pool.deposit(&user, &vec![&e, UNIT, UNIT], &0i128);
    pool.deposit(&user, &vec![&e, 0i128, 0i128], &0i128); // nothing in -> revert
}

#[test]
#[should_panic]
fn deposit_wrong_length_reverts() {
    let (e, user, pool_id, _a, _b) = setup();
    let pool = LiquidityPoolClient::new(&e, &pool_id);
    // 3 amounts for a 2-token pool.
    pool.deposit(&user, &vec![&e, UNIT, UNIT, UNIT], &0i128);
}

#[test]
#[should_panic]
fn deposit_over_token_cap_reverts() {
    // Per-token cap at half a UNIT; a UNIT-sized balanced deposit blows past it.
    let (e, user, pool_id, _a, _b) =
        setup_limited(500_000_000_000i128, 1_000_000_000_000_000_000i128);
    let pool = LiquidityPoolClient::new(&e, &pool_id);
    pool.deposit(&user, &vec![&e, UNIT, UNIT], &0i128);
}

#[test]
#[should_panic]
fn deposit_over_lp_max_supply_reverts() {
    // Tiny LP supply ceiling; the first deposit would mint far more than that.
    let (e, user, pool_id, _a, _b) = setup_limited(1_000_000_000_000_000_000_000i128, 1_000i128);
    let pool = LiquidityPoolClient::new(&e, &pool_id);
    pool.deposit(&user, &vec![&e, UNIT, UNIT], &0i128);
}

// --- withdraw ---

#[test]
fn partial_withdraw_is_proportional() {
    let (e, user, pool_id, addr_a, addr_b) = setup();
    let pool = LiquidityPoolClient::new(&e, &pool_id);
    let token_a = TokenClient::new(&e, &addr_a);
    let lp = pool.deposit(&user, &vec![&e, UNIT, UNIT], &0i128);

    let half = lp / 2;
    let a_before = token_a.balance(&user);
    let out = pool.withdraw(&user, &half, &vec![&e, 0i128, 0i128]);

    let out_a = out.get(0).unwrap();
    let out_b = out.get(1).unwrap();
    assert!(out_a > 0 && out_a == out_b); // symmetric pool -> equal legs
    assert!(2 * out_a >= UNIT - 2 && 2 * out_a <= UNIT); // ~half of each reserve
    assert_eq!(pool.total_supply(), lp - half);
    assert_eq!(pool.get_reserves(), vec![&e, UNIT - out_a, UNIT - out_b]);
    assert_eq!(token_a.balance(&user), a_before + out_a);
    assert_pool_balances_match_reserves(&e, &pool, &pool_id, &addr_a, &addr_b);
}

#[test]
fn withdraw_one_token_reduces_only_selected_reserve() {
    let (e, user, pool_id, addr_a, addr_b) = setup();
    let pool = LiquidityPoolClient::new(&e, &pool_id);
    let token_a = TokenClient::new(&e, &addr_a);
    let token_b = TokenClient::new(&e, &addr_b);
    let lp = pool.deposit(&user, &vec![&e, UNIT, UNIT], &0i128);

    let burn = lp / 2;
    let out = pool.withdraw_one_token(&user, &burn, &addr_a, &0i128);

    assert!(out > 0);
    assert!(out < UNIT);
    assert_eq!(pool.total_supply(), lp - burn);
    assert_eq!(token_a.balance(&user), out);
    assert_eq!(token_b.balance(&user), 0);
    assert_eq!(pool.get_reserves(), vec![&e, UNIT - out, UNIT]);
    assert_pool_balances_match_reserves(&e, &pool, &pool_id, &addr_a, &addr_b);
}

#[test]
fn withdraw_one_token_after_imbalance_uses_current_reserves() {
    let (e, user, pool_id, addr_a, addr_b) = setup();
    let pool = LiquidityPoolClient::new(&e, &pool_id);
    let token_a = TokenClient::new(&e, &addr_a);
    let token_b = TokenClient::new(&e, &addr_b);
    let lp = pool.deposit(&user, &vec![&e, UNIT, UNIT], &0i128);

    StellarAssetClient::new(&e, &addr_a).mint(&user, &100_000_000_000);
    pool.swap_exact_in(&user, &addr_a, &addr_b, &100_000_000_000i128, &0i128);

    let reserves_before = pool.get_reserves();
    assert!(reserves_before.get(0).unwrap() > UNIT);
    assert!(reserves_before.get(1).unwrap() < UNIT);

    let burn = lp / 4;
    let user_a_before = token_a.balance(&user);
    let user_b_before = token_b.balance(&user);
    let out = pool.withdraw_one_token(&user, &burn, &addr_b, &0i128);
    let reserves_after = pool.get_reserves();

    assert!(out > 0);
    assert_eq!(pool.total_supply(), lp - burn);
    assert_eq!(
        reserves_after.get(0).unwrap(),
        reserves_before.get(0).unwrap()
    );
    assert_eq!(
        reserves_after.get(1).unwrap(),
        reserves_before.get(1).unwrap() - out
    );
    assert_eq!(token_a.balance(&user), user_a_before);
    assert_eq!(token_b.balance(&user), user_b_before + out);
    assert_pool_balances_match_reserves(&e, &pool, &pool_id, &addr_a, &addr_b);

    let (balanced_e, balanced_user, balanced_pool_id, _balanced_a, balanced_b) = setup();
    let balanced_pool = LiquidityPoolClient::new(&balanced_e, &balanced_pool_id);
    let balanced_lp = balanced_pool.deposit(&balanced_user, &vec![&balanced_e, UNIT, UNIT], &0i128);
    let balanced_out =
        balanced_pool.withdraw_one_token(&balanced_user, &(balanced_lp / 4), &balanced_b, &0i128);

    assert!(out < balanced_out);
}

#[test]
#[should_panic]
fn withdraw_one_token_below_min_out_reverts() {
    let (e, user, pool_id, addr_a, _addr_b) = setup();
    let pool = LiquidityPoolClient::new(&e, &pool_id);
    let lp = pool.deposit(&user, &vec![&e, UNIT, UNIT], &0i128);

    pool.withdraw_one_token(&user, &(lp / 2), &addr_a, &i128::MAX);
}

#[test]
#[should_panic]
fn withdraw_one_token_rejects_fee_on_transfer_output() {
    let (e, user, pool_id, addr_fee, _addr_sac) = setup_with_fee_token(0);
    let pool = LiquidityPoolClient::new(&e, &pool_id);
    let fee = FeeTokenClient::new(&e, &addr_fee);
    let lp = pool.deposit(&user, &vec![&e, UNIT, UNIT], &0i128);

    fee.set_fee_bps(&1_000);
    pool.withdraw_one_token(&user, &(lp / 2), &addr_fee, &0i128);
}

#[test]
#[should_panic]
fn withdraw_one_token_unknown_token_reverts() {
    let (e, user, pool_id, _addr_a, _addr_b) = setup();
    let pool = LiquidityPoolClient::new(&e, &pool_id);
    let lp = pool.deposit(&user, &vec![&e, UNIT, UNIT], &0i128);
    let bogus = Address::generate(&e);

    pool.withdraw_one_token(&user, &(lp / 2), &bogus, &0i128);
}

#[test]
fn withdraw_after_swap_keeps_reserves_matched_to_balances() {
    let (e, user, pool_id, addr_a, addr_b) = setup();
    let pool = LiquidityPoolClient::new(&e, &pool_id);

    pool.deposit(&user, &vec![&e, UNIT, UNIT], &0i128);
    StellarAssetClient::new(&e, &addr_a).mint(&user, &1_000_000_000);
    pool.swap_exact_in(&user, &addr_a, &addr_b, &1_000_000_000i128, &0i128);

    let lp = pool.balance(&user);
    pool.withdraw(&user, &(lp / 3), &vec![&e, 0i128, 0i128]);

    assert_pool_balances_match_reserves(&e, &pool, &pool_id, &addr_a, &addr_b);
}

#[test]
#[should_panic]
fn withdraw_below_min_out_reverts() {
    let (e, user, pool_id, _a, _b) = setup();
    let pool = LiquidityPoolClient::new(&e, &pool_id);
    let lp = pool.deposit(&user, &vec![&e, UNIT, UNIT], &0i128);
    // Demand more out than a proportional exit can return -> slippage guard.
    pool.withdraw(&user, &lp, &vec![&e, i128::MAX, i128::MAX]);
}

// --- swap ---

#[test]
#[should_panic]
fn swap_exact_in_below_min_out_reverts() {
    let (e, user, pool_id, addr_a, addr_b) = setup();
    let pool = LiquidityPoolClient::new(&e, &pool_id);
    pool.deposit(&user, &vec![&e, UNIT, UNIT], &0i128);
    StellarAssetClient::new(&e, &addr_a).mint(&user, &1_000_000_000);
    pool.swap_exact_in(&user, &addr_a, &addr_b, &1_000_000_000i128, &i128::MAX);
}

#[test]
#[should_panic]
fn swap_exact_in_zero_amount_reverts() {
    let (e, user, pool_id, addr_a, addr_b) = setup();
    let pool = LiquidityPoolClient::new(&e, &pool_id);
    pool.deposit(&user, &vec![&e, UNIT, UNIT], &0i128);
    pool.swap_exact_in(&user, &addr_a, &addr_b, &0i128, &0i128);
}

#[test]
#[should_panic]
fn swap_unknown_token_reverts() {
    let (e, user, pool_id, _addr_a, addr_b) = setup();
    let pool = LiquidityPoolClient::new(&e, &pool_id);
    pool.deposit(&user, &vec![&e, UNIT, UNIT], &0i128);
    let bogus = Address::generate(&e); // not one of the pool's tokens
    pool.swap_exact_in(&user, &bogus, &addr_b, &1_000_000i128, &0i128);
}

#[test]
fn no_protocol_fee_when_zero() {
    // Default protocol_fee == 0, so the beneficiary never accrues.
    let (e, user, beneficiary, pool_id, addr_a, addr_b) = setup_with(0);
    let pool = LiquidityPoolClient::new(&e, &pool_id);
    let token_b = TokenClient::new(&e, &addr_b);
    pool.deposit(&user, &vec![&e, UNIT, UNIT], &0i128);
    StellarAssetClient::new(&e, &addr_a).mint(&user, &10_000_000_000);
    pool.swap_exact_in(&user, &addr_a, &addr_b, &10_000_000_000i128, &0i128);
    assert_eq!(token_b.balance(&beneficiary), 0);
}

// --- admin / config ---

#[test]
#[should_panic]
fn set_protocol_fee_out_of_range_reverts() {
    let (e, _user, pool_id, _a, _b) = setup();
    let pool = LiquidityPoolClient::new(&e, &pool_id);
    pool.set_protocol_fee(&1_000_000_001u64); // > 1e9 (100% of the swap fee)
}

#[test]
fn set_beneficiary_redirects_fee() {
    let (e, user, old_ben, pool_id, addr_a, addr_b) = setup_with(500_000_000); // 50% of fee
    let pool = LiquidityPoolClient::new(&e, &pool_id);
    let token_b = TokenClient::new(&e, &addr_b);
    let new_ben = Address::generate(&e);
    pool.set_beneficiary(&new_ben);

    pool.deposit(&user, &vec![&e, UNIT, UNIT], &0i128);
    StellarAssetClient::new(&e, &addr_a).mint(&user, &100_000_000_000);
    pool.swap_exact_in(&user, &addr_a, &addr_b, &100_000_000_000i128, &0i128);

    assert!(token_b.balance(&new_ben) > 0); // fee now lands on the new beneficiary
    assert_eq!(token_b.balance(&old_ben), 0); // and not the old one
}

#[test]
#[should_panic]
fn set_amp_ramp_rejects_too_large() {
    let (e, _user, pool_id, _a, _b) = setup();
    let pool = LiquidityPoolClient::new(&e, &pool_id);
    pool.set_amp_ramp(&12_001u32, &0u64); // > MAX_AMP (12000)
}

#[test]
fn amp_ramp_down_after_up() {
    let (e, _user, pool_id, _a, _b) = setup();
    let pool = LiquidityPoolClient::new(&e, &pool_id);
    let start = e.ledger().timestamp();

    pool.set_amp_ramp(&200u32, &1000u64); // 100 -> 200
    e.ledger().with_mut(|l| l.timestamp = start + 1000);
    assert_eq!(pool.get_amp(), 200);

    let mid_start = e.ledger().timestamp();
    pool.set_amp_ramp(&150u32, &1000u64); // now ramp back down 200 -> 150
    assert_eq!(pool.get_amp(), 200); // ramp starts from the current factor
    e.ledger().with_mut(|l| l.timestamp = mid_start + 500);
    assert_eq!(pool.get_amp(), 175);
    e.ledger().with_mut(|l| l.timestamp = mid_start + 1000);
    assert_eq!(pool.get_amp(), 150);
}

// --- LP share token (SEP-41) / views / ownership ---

#[test]
fn lp_token_transfer_and_approve() {
    let (e, user, pool_id, _a, _b) = setup();
    let pool = LiquidityPoolClient::new(&e, &pool_id);
    let lp = pool.deposit(&user, &vec![&e, UNIT, UNIT], &0i128);

    let other = Address::generate(&e);
    pool.transfer(&user, &other, &(lp / 4));
    assert_eq!(pool.balance(&other), lp / 4);
    assert_eq!(pool.balance(&user), lp - lp / 4);
    assert_eq!(pool.total_supply(), lp); // transfers don't change supply

    let spender = Address::generate(&e);
    let sink = Address::generate(&e);
    pool.approve(&user, &spender, &(lp / 4), &10_000u32);
    assert_eq!(pool.allowance(&user, &spender), lp / 4);
    pool.transfer_from(&spender, &user, &sink, &(lp / 8));
    assert_eq!(pool.balance(&sink), lp / 8);
    assert_eq!(pool.allowance(&user, &spender), lp / 4 - lp / 8);
}

#[test]
#[should_panic]
fn direct_lp_burn_reverts() {
    let (e, user, pool_id, _a, _b) = setup();
    let pool = LiquidityPoolClient::new(&e, &pool_id);
    let lp = pool.deposit(&user, &vec![&e, UNIT, UNIT], &0i128);

    pool.burn(&user, &(lp / 2));
}

#[test]
#[should_panic]
fn direct_lp_burn_from_reverts() {
    let (e, user, pool_id, _a, _b) = setup();
    let pool = LiquidityPoolClient::new(&e, &pool_id);
    let lp = pool.deposit(&user, &vec![&e, UNIT, UNIT], &0i128);

    let spender = Address::generate(&e);
    pool.approve(&user, &spender, &(lp / 2), &10_000u32);
    pool.burn_from(&spender, &user, &(lp / 2));
}

#[test]
fn lp_token_decimals_is_nine() {
    let (e, _user, pool_id, _a, _b) = setup();
    let pool = LiquidityPoolClient::new(&e, &pool_id);
    assert_eq!(pool.decimals(), 9);
}

#[test]
fn get_tokens_is_sorted() {
    let (e, _user, pool_id, addr_a, addr_b) = setup();
    let pool = LiquidityPoolClient::new(&e, &pool_id);
    assert!(addr_a < addr_b);
    assert_eq!(pool.get_tokens(), vec![&e, addr_a, addr_b]);
}

#[test]
fn renounce_ownership_clears_owner() {
    let (e, _user, pool_id, _a, _b) = setup();
    let pool = LiquidityPoolClient::new(&e, &pool_id);
    pool.renounce_ownership();
    assert_eq!(pool.get_owner(), None);
}

// --- events ---

#[test]
fn deposit_and_swap_emit_events() {
    use crate::contract::{Deposit, Swap};
    use soroban_sdk::{testutils::Events as _, Event};

    let (e, user, pool_id, addr_a, addr_b) = setup();
    let pool = LiquidityPoolClient::new(&e, &pool_id);

    let amounts_in = vec![&e, UNIT, UNIT];
    let lp = pool.deposit(&user, &amounts_in, &0i128);
    let deposit_event = Deposit {
        to: user.clone(),
        amounts_in,
        lp_minted: lp,
        protocol_lp: 0, // balanced deposit, no protocol cut
    }
    .to_xdr(&e, &pool_id);
    assert!(e
        .events()
        .all()
        .filter_by_contract(&pool_id)
        .events()
        .contains(&deposit_event));

    StellarAssetClient::new(&e, &addr_a).mint(&user, &1_000_000_000);
    let amount_in = 1_000_000_000i128;
    let out = pool.swap_exact_in(&user, &addr_a, &addr_b, &amount_in, &0i128);
    let swap_event = Swap {
        to: user.clone(),
        token_in: addr_a.clone(),
        token_out: addr_b.clone(),
        amount_in,
        amount_out: out,
    }
    .to_xdr(&e, &pool_id);
    assert!(e
        .events()
        .all()
        .filter_by_contract(&pool_id)
        .events()
        .contains(&swap_event));
}

// --- protocol fee split on deposits / single-token withdrawals ---

#[test]
fn imbalanced_deposit_mints_protocol_lp_without_changing_user_lp() {
    // Baseline: same operations with no protocol fee.
    let (e0, user0, _b0, pool0_id, _addr_a0, _addr_b0) = setup_with(0);
    let pool0 = LiquidityPoolClient::new(&e0, &pool0_id);
    pool0.deposit(
        &user0,
        &vec![&e0, 500_000_000_000i128, 500_000_000_000i128],
        &0i128,
    );
    let user_lp_no_proto = pool0.deposit(&user0, &vec![&e0, 500_000_000_000i128, 0i128], &0i128);

    // 50% protocol fee.
    let (e, user, beneficiary, pool_id, _addr_a, _addr_b) = setup_with(500_000_000);
    let pool = LiquidityPoolClient::new(&e, &pool_id);
    pool.deposit(
        &user,
        &vec![&e, 500_000_000_000i128, 500_000_000_000i128],
        &0i128,
    );
    assert_eq!(pool.balance(&beneficiary), 0); // balanced deposit -> no protocol LP

    let user_lp = pool.deposit(&user, &vec![&e, 500_000_000_000i128, 0i128], &0i128);

    // The depositor receives the same LP regardless of the protocol fee...
    assert_eq!(user_lp, user_lp_no_proto);
    // ...but the beneficiary now holds protocol LP minted from the imbalance fee.
    assert!(pool.balance(&beneficiary) > 0);
}

#[test]
fn withdraw_one_token_routes_protocol_fee_to_beneficiary() {
    // Baseline: no protocol fee.
    let (e0, user0, _b0, pool0_id, addr_a0, _addr_b0) = setup_with(0);
    let pool0 = LiquidityPoolClient::new(&e0, &pool0_id);
    let lp0 = pool0.deposit(&user0, &vec![&e0, UNIT, UNIT], &0i128);
    let out_no_proto = pool0.withdraw_one_token(&user0, &(lp0 / 2), &addr_a0, &0i128);

    // 50% protocol fee.
    let (e, user, beneficiary, pool_id, addr_a, addr_b) = setup_with(500_000_000);
    let pool = LiquidityPoolClient::new(&e, &pool_id);
    let token_a = TokenClient::new(&e, &addr_a);
    let lp = pool.deposit(&user, &vec![&e, UNIT, UNIT], &0i128);
    assert_eq!(token_a.balance(&beneficiary), 0);

    let out = pool.withdraw_one_token(&user, &(lp / 2), &addr_a, &0i128);

    // The caller's payout is unchanged by the protocol fee (the cut comes out of
    // the LP share of the fee, not the caller's output)...
    assert_eq!(out, out_no_proto);
    // ...and the beneficiary received its cut of the imbalance fee in token A.
    assert!(token_a.balance(&beneficiary) > 0);
    // Reserves stay matched to actual balances after both transfers.
    assert_pool_balances_match_reserves(&e, &pool, &pool_id, &addr_a, &addr_b);
}

#[test]
fn proportional_withdraw_charges_no_protocol_fee() {
    // Proportional exits are fee-free, so the beneficiary never accrues on them.
    let (e, user, beneficiary, pool_id, addr_a, addr_b) = setup_with(500_000_000);
    let pool = LiquidityPoolClient::new(&e, &pool_id);
    let token_a = TokenClient::new(&e, &addr_a);
    let token_b = TokenClient::new(&e, &addr_b);

    let lp = pool.deposit(&user, &vec![&e, UNIT, UNIT], &0i128);
    pool.withdraw(&user, &(lp / 2), &vec![&e, 0i128, 0i128]);

    assert_eq!(token_a.balance(&beneficiary), 0);
    assert_eq!(token_b.balance(&beneficiary), 0);
    assert_eq!(pool.balance(&beneficiary), 0);
}

// An existing LP is not harmed when a third party makes an imbalanced deposit:
// they earn a share of that deposit's swap fee. The protocol fee reduces that
// share (the protocol skims its cut) but never reverses it into a loss of
// principal — the freshly minted protocol LP cannot exceed the fee itself.
#[test]
fn existing_lp_keeps_principal_when_third_party_deposits() {
    // Total tokens (par sum) an initial LP recovers after a third party makes a
    // large single-sided deposit, for a given protocol fee.
    fn recovered(protocol_fee: u64) -> i128 {
        let (e, alice, _ben, pool_id, addr_a, _addr_b) = setup_with(protocol_fee);
        let pool = LiquidityPoolClient::new(&e, &pool_id);
        let lp_a = pool.deposit(&alice, &vec![&e, UNIT, UNIT], &0i128);

        let bob = Address::generate(&e);
        StellarAssetClient::new(&e, &addr_a).mint(&bob, &(2 * UNIT));
        pool.deposit(&bob, &vec![&e, 2 * UNIT, 0i128], &0i128);

        let out = pool.withdraw(&alice, &lp_a, &vec![&e, 0i128, 0i128]);
        out.get(0).unwrap() + out.get(1).unwrap()
    }

    let deposited = 2 * UNIT;
    let no_proto = recovered(0);
    let with_proto = recovered(500_000_000); // 50%

    // The initial LP comes out ahead of their deposit either way — they earn a
    // slice of the new depositor's fee, not a loss.
    assert!(no_proto > deposited);
    assert!(with_proto > deposited);
    // The protocol's cut only trims that slice; it never pushes the LP below
    // where they'd be with no protocol fee.
    assert!(with_proto <= no_proto);
}
