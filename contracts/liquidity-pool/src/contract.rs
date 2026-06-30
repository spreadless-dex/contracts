//! The contract itself: the `LiquidityPool` type, its AMM operations (init,
//! deposit, withdraw, and swaps later), and its LP-share token impls.
//!
//! The operations are a thin orchestration layer — every formula lives in
//! `math`, and the `i128 <-> u64` scaling and storage live in `pool`. The struct
//! and ALL `#[contractimpl]` blocks are kept in this one module on purpose:
//! `#[contractimpl(contracttrait)]` generates client methods on the
//! macro-generated `LiquidityPoolClient`, and rust-analyzer only resolves those
//! when they share a module with `#[contract]`.

// The entrypoints are `LiquidityPoolInterface` trait-impl methods (not `pub`),
// exported via the macro for wasm + tests but unreachable in a plain host
// `cargo build` — which makes their private helpers (e.g. `commit_swap`) look
// dead. Silence that here; the wasm + test builds are warning-clean.
#![allow(dead_code, clippy::too_many_arguments)]

use soroban_sdk::{
    contract, contractevent, contractimpl, contractmeta, panic_with_error, token, Address, Env,
    MuxedAddress, String, Vec,
};
use stellar_access::ownable::{self, Ownable};
use stellar_contract_utils::pausable;
use stellar_macros::{only_owner, when_not_paused};
use stellar_tokens::fungible::{burnable::FungibleBurnable, capped, Base, FungibleToken};

use crate::error::Error;
use crate::interface::LiquidityPoolInterface;
use crate::math::{AMP_PRECISION, MAX_TOKENS, MIN_TOKENS};
use crate::pool::{self, Pool, PoolToken};

// Metadata that is added on to the WASM custom section
contractmeta!(
    key = "Description",
    val = "Multi-asset StableSwap AMM (Stabble-style); the pool is its own LP token"
);

// --- contract events ---
//
// Domain events for off-chain indexers/UIs. The LP-share token side (mint,
// burn, transfer) already emits SEP-41 events via OpenZeppelin's `Base`; these
// cover the AMM operations it doesn't. Amounts are in each token's raw units.

/// Liquidity added. Topics: `("deposit", to)`.
#[contractevent]
#[derive(Clone)]
pub struct Deposit {
    #[topic]
    pub to: Address,
    pub amounts_in: Vec<i128>,
    pub lp_minted: i128,
}

/// Proportional withdrawal. Topics: `("withdraw", to)`.
#[contractevent]
#[derive(Clone)]
pub struct Withdraw {
    #[topic]
    pub to: Address,
    pub lp_burned: i128,
    pub amounts_out: Vec<i128>,
}

/// Single-token withdrawal. Topics: `("withdraw_one_token", to)`.
#[contractevent]
#[derive(Clone)]
pub struct WithdrawOneToken {
    #[topic]
    pub to: Address,
    pub token_out: Address,
    pub lp_burned: i128,
    pub amount_out: i128,
}

/// Swap, exact-in or exact-out. Topics: `("swap", to)`.
#[contractevent]
#[derive(Clone)]
pub struct Swap {
    #[topic]
    pub to: Address,
    pub token_in: Address,
    pub token_out: Address,
    pub amount_in: i128,
    pub amount_out: i128,
}

/// The liquidity pool. It is simultaneously the AMM and its own SEP-41 LP-share
/// token (Uniswap-V2-pair style).
#[contract]
pub struct LiquidityPool;

#[contractimpl]
impl LiquidityPool {
    /// Initialize the pool.
    ///
    /// * `tokens` must be 2..=MAX_TOKENS distinct addresses in strictly
    ///   ascending order (canonical, dedup-free).
    /// * `amp_factor` is the amplification *factor* (effective A); the ramp
    ///   starts static (initial == target).
    /// * `swap_fee` / `protocol_fee` use 1e9 == 100%.
    /// * `max_caps` are per-token caps in that token's raw units.
    /// * `lp_max_supply` caps total LP shares (the pool's own token supply).
    pub fn __constructor(
        e: Env,
        owner: Address,
        tokens: Vec<Address>,
        amp_factor: u32,
        swap_fee: u64,
        protocol_fee: u64,
        beneficiary: Address,
        max_caps: Vec<i128>,
        lp_max_supply: i128,
    ) {
        let n = tokens.len();
        if (n as usize) < MIN_TOKENS || (n as usize) > MAX_TOKENS {
            panic_with_error!(&e, Error::InvalidTokenCount);
        }
        if max_caps.len() != n {
            panic_with_error!(&e, Error::CapsLengthMismatch);
        }
        if !pool::is_valid_amp_factor(amp_factor) {
            panic_with_error!(&e, Error::InvalidAmpFactor);
        }
        if !pool::is_valid_swap_fee(swap_fee) {
            panic_with_error!(&e, Error::InvalidSwapFee);
        }
        if !pool::is_valid_protocol_fee(protocol_fee) {
            panic_with_error!(&e, Error::InvalidProtocolFee);
        }

        let mut pool_tokens = Vec::new(&e);
        let mut i = 0u32;
        while i < n {
            let addr = tokens.get(i).unwrap();
            if i > 0 && tokens.get(i - 1).unwrap() >= addr {
                panic_with_error!(&e, Error::TokensNotSorted);
            }
            let decimals = token::Client::new(&e, &addr).decimals();
            let (scaling_factor, scaling_up) = match pool::scaling_for(decimals) {
                Some(s) => s,
                None => panic_with_error!(&e, Error::InvalidDecimals),
            };
            let max_cap =
                match pool::to_internal(max_caps.get(i).unwrap(), scaling_factor, scaling_up) {
                    Some(c) => c,
                    None => panic_with_error!(&e, Error::InvalidCap),
                };
            pool_tokens.push_back(PoolToken {
                token: addr,
                decimals,
                scaling_factor,
                scaling_up,
                reserve: 0,
                max_cap,
            });
            i += 1;
        }

        let pool = Pool {
            tokens: pool_tokens,
            amp_initial_factor: amp_factor,
            amp_target_factor: amp_factor,
            ramp_start_ts: 0,
            ramp_stop_ts: 0,
            swap_fee,
            protocol_fee,
            beneficiary,
        };
        pool::write_pool(&e, &pool);
        pool::extend_instance_ttl(&e);

        ownable::set_owner(&e, &owner);

        // The pool contract is itself the SEP-41 LP-share token. LP shares use
        // the internal 9-decimal scale (the invariant D scale).
        Base::set_metadata(
            &e,
            pool::INTERNAL_DECIMALS,
            String::from_str(&e, "Spreadless LP"),
            String::from_str(&e, "SLP"),
        );
        capped::set_cap(&e, lp_max_supply);
    }
}

// All other entrypoints are the `LiquidityPoolInterface` (see `interface.rs`);
// implementing it here makes any signature drift a compile error.
#[contractimpl(contracttrait)]
impl LiquidityPoolInterface for LiquidityPool {
    #[when_not_paused]
    fn deposit(e: Env, to: Address, amounts_in: Vec<i128>, min_lp_out: i128) -> i128 {
        to.require_auth();

        let mut pool = pool::read_pool(&e);
        let n = pool.tokens.len();
        let nn = n as usize;
        if amounts_in.len() != n {
            panic_with_error!(&e, Error::AmountsLengthMismatch);
        }

        // Normalize inputs to internal (9-dec) balances.
        let mut amounts_array = [0u64; MAX_TOKENS];
        let mut i = 0u32;
        while i < n {
            let raw = amounts_in.get(i).unwrap();
            let amt = match pool.tokens.get(i).unwrap().to_internal(raw) {
                Some(a) => a,
                None => panic_with_error!(&e, Error::InvalidAmount),
            };
            amounts_array[i as usize] = amt;
            i += 1;
        }
        let amounts_int = pool::NormalizedAmounts::new(amounts_array, nn)
            .unwrap_or_else(|| panic_with_error!(&e, Error::MathError));
        if !amounts_int.any_positive() {
            panic_with_error!(&e, Error::ZeroDeposit);
        }

        let total_supply = Base::total_supply(&e);
        let supply_u64 = if total_supply == 0 {
            0
        } else {
            unwrap_u64(&e, total_supply)
        };

        if total_supply == 0 {
            // First deposit: every token must be funded; LP minted = invariant D.
            if amounts_int.contains_zero() {
                panic_with_error!(&e, Error::FirstDepositNotFull);
            }
        }
        let lp_out = match pool::deposit_exact_tokens_in(
            &e,
            &pool,
            e.ledger().timestamp(),
            &amounts_int,
            supply_u64,
        ) {
            Some(l) => l,
            None => panic_with_error!(&e, Error::MathError),
        };

        if lp_out == 0 || (lp_out as i128) < min_lp_out {
            panic_with_error!(&e, Error::SlippageExceeded);
        }
        capped::check_cap(&e, lp_out as i128, total_supply);

        // Pull tokens in and credit reserves (enforcing per-token caps).
        let contract = e.current_contract_address();
        let mut i = 0u32;
        while i < n {
            let amt_int = amounts_int.get(i as usize).unwrap();
            if amt_int > 0 {
                let mut t = pool.tokens.get(i).unwrap();
                // For >9-dec tokens this is the input truncated to 9-dec
                // precision, so no sub-precision dust is stranded in the pool.
                let transfer_in = t.to_raw(amt_int);
                let received_int = transfer_in_exact(&e, &t, &to, &contract, transfer_in);
                let new_reserve = match t.reserve.checked_add(received_int) {
                    Some(r) if r <= t.max_cap => r,
                    _ => panic_with_error!(&e, Error::CapExceeded),
                };
                t.reserve = new_reserve;
                pool.tokens.set(i, t);
            }
            i += 1;
        }

        Base::mint(&e, &to, lp_out as i128);
        pool::write_pool(&e, &pool);
        pool::extend_instance_ttl(&e);

        Deposit {
            to,
            amounts_in,
            lp_minted: lp_out as i128,
        }
        .publish(&e);

        lp_out as i128
    }

    /// Burn `lp_amount` shares and withdraw a proportional slice of every
    /// reserve. Returns the raw amounts paid out, in token order.
    #[when_not_paused]
    fn withdraw(e: Env, to: Address, lp_amount: i128, min_amounts_out: Vec<i128>) -> Vec<i128> {
        to.require_auth();

        let mut pool = pool::read_pool(&e);
        let n = pool.tokens.len();
        if min_amounts_out.len() != n {
            panic_with_error!(&e, Error::AmountsLengthMismatch);
        }
        if lp_amount <= 0 {
            panic_with_error!(&e, Error::InvalidAmount);
        }

        // Proportional share is taken against the supply *before* burning.
        let supply_u64 = match u64::try_from(Base::total_supply(&e)) {
            Ok(s) if s > 0 => s,
            _ => panic_with_error!(&e, Error::MathError),
        };
        let lp_u64 = unwrap_u64(&e, lp_amount);

        let quote = pool::withdraw_proportional(&pool, supply_u64, lp_u64)
            .unwrap_or_else(|| panic_with_error!(&e, Error::MathError));

        // Burn the caller's shares. We use the low-level `update` (which still
        // checks the holder's balance) rather than `Base::burn`, because the
        // latter calls `to.require_auth()` again in this same frame and the
        // top-level `require_auth` above already authorizes the withdrawal.
        Base::update(&e, Some(&to), None, lp_amount);

        let contract = e.current_contract_address();
        let mut amounts_out = Vec::new(&e);
        let mut i = 0u32;
        while i < n {
            let mut t = pool.tokens.get(i).unwrap();
            let out_int = quote.amounts_out[i as usize];
            let out_raw = t.to_raw(out_int);
            if out_raw < min_amounts_out.get(i).unwrap() {
                panic_with_error!(&e, Error::SlippageExceeded);
            }
            if out_raw > 0 {
                let sent_int = transfer_out_exact(&e, &t, &contract, &to, out_raw);
                t.reserve = match t.reserve.checked_sub(sent_int) {
                    Some(r) => r,
                    None => panic_with_error!(&e, Error::MathError),
                };
                pool.tokens.set(i, t);
            }
            amounts_out.push_back(out_raw);
            i += 1;
        }

        pool::write_pool(&e, &pool);
        pool::extend_instance_ttl(&e);

        Withdraw {
            to,
            lp_burned: lp_amount,
            amounts_out: amounts_out.clone(),
        }
        .publish(&e);

        amounts_out
    }

    /// Burn `lp_amount` shares and withdraw a single token. The burned share
    /// lowers the stable invariant, and the selected token pays swap fees on the
    /// imbalanced portion of the exit.
    #[when_not_paused]
    fn withdraw_one_token(
        e: Env,
        to: Address,
        lp_amount: i128,
        token_out: Address,
        min_amount_out: i128,
    ) -> i128 {
        to.require_auth();

        let mut pool = pool::read_pool(&e);
        if lp_amount <= 0 {
            panic_with_error!(&e, Error::InvalidAmount);
        }
        let token_index = pool::token_index(&pool, &token_out)
            .unwrap_or_else(|| panic_with_error!(&e, Error::UnknownToken));

        let supply_u64 = match u64::try_from(Base::total_supply(&e)) {
            Ok(s) if s > 0 => s,
            _ => panic_with_error!(&e, Error::MathError),
        };
        let lp_u64 = unwrap_u64(&e, lp_amount);

        let quote = pool::withdraw_one_token(
            &e,
            &pool,
            e.ledger().timestamp(),
            token_index,
            lp_u64,
            supply_u64,
        )
        .unwrap_or_else(|| panic_with_error!(&e, Error::MathError));

        let mut token = pool.tokens.get(token_index as u32).unwrap();
        let out_raw = token.to_raw(quote.amount_out);
        if out_raw < min_amount_out {
            panic_with_error!(&e, Error::SlippageExceeded);
        }

        // See `withdraw` above for why exits burn through `Base::update`.
        Base::update(&e, Some(&to), None, lp_amount);

        if out_raw > 0 {
            let contract = e.current_contract_address();
            let sent_int = transfer_out_exact(&e, &token, &contract, &to, out_raw);
            token.reserve = match token.reserve.checked_sub(sent_int) {
                Some(r) => r,
                None => panic_with_error!(&e, Error::MathError),
            };
            pool.tokens.set(token_index as u32, token);
        }

        pool::write_pool(&e, &pool);
        pool::extend_instance_ttl(&e);

        WithdrawOneToken {
            to,
            token_out,
            lp_burned: lp_amount,
            amount_out: out_raw,
        }
        .publish(&e);

        out_raw
    }

    /// Swap an exact `amount_in` of `token_in` for `token_out`, requiring at
    /// least `min_out` back. The swap fee is charged on the output; the
    /// protocol's cut of it is routed to the beneficiary and the rest stays in
    /// the pool for LPs. Returns the amount of `token_out` sent to `to`.
    #[when_not_paused]
    fn swap_exact_in(
        e: Env,
        to: Address,
        token_in: Address,
        token_out: Address,
        amount_in: i128,
        min_out: i128,
    ) -> i128 {
        to.require_auth();

        let mut pool = pool::read_pool(&e);
        let (i, j) = swap_indices(&e, &pool, &token_in, &token_out);

        let t_in = pool.tokens.get(i as u32).unwrap();
        let amount_in_int = t_in
            .to_internal(amount_in)
            .filter(|a| *a > 0)
            .unwrap_or_else(|| panic_with_error!(&e, Error::InvalidAmount));
        // Pull exactly what we credit (lossless for <= 9-decimal tokens).
        let in_raw = t_in.to_raw(amount_in_int);

        let quote = pool::swap_exact_in(&e, &pool, e.ledger().timestamp(), i, j, amount_in_int)
            .unwrap_or_else(|| panic_with_error!(&e, Error::MathError));

        let amount_out = pool.tokens.get(j as u32).unwrap().to_raw(quote.net_out);
        if amount_out < min_out {
            panic_with_error!(&e, Error::SlippageExceeded);
        }

        commit_swap(
            &e,
            &mut pool,
            &to,
            SwapCommit {
                token_in_index: i,
                token_out_index: j,
                in_raw,
                net_out: quote.net_out,
                protocol: quote.protocol,
            },
        );
        pool::write_pool(&e, &pool);
        pool::extend_instance_ttl(&e);

        Swap {
            to,
            token_in,
            token_out,
            amount_in: in_raw,
            amount_out,
        }
        .publish(&e);

        amount_out
    }

    /// Swap `token_in` for an exact `amount_out` of `token_out`, spending at
    /// most `max_in`. Returns the amount of `token_in` taken from `to`.
    #[when_not_paused]
    fn swap_exact_out(
        e: Env,
        to: Address,
        token_in: Address,
        token_out: Address,
        amount_out: i128,
        max_in: i128,
    ) -> i128 {
        to.require_auth();

        let mut pool = pool::read_pool(&e);
        let (i, j) = swap_indices(&e, &pool, &token_in, &token_out);

        let net_out = pool
            .tokens
            .get(j as u32)
            .unwrap()
            .to_internal(amount_out)
            .filter(|a| *a > 0)
            .unwrap_or_else(|| panic_with_error!(&e, Error::InvalidAmount));

        let quote = pool::swap_exact_out(&e, &pool, e.ledger().timestamp(), i, j, net_out)
            .unwrap_or_else(|| panic_with_error!(&e, Error::MathError));

        // Round the charged input up so any sub-precision dust favours the pool.
        let in_raw = pool
            .tokens
            .get(i as u32)
            .unwrap()
            .to_raw_up(quote.amount_in);
        if in_raw > max_in {
            panic_with_error!(&e, Error::SlippageExceeded);
        }

        commit_swap(
            &e,
            &mut pool,
            &to,
            SwapCommit {
                token_in_index: i,
                token_out_index: j,
                in_raw,
                net_out,
                protocol: quote.protocol,
            },
        );
        pool::write_pool(&e, &pool);
        pool::extend_instance_ttl(&e);

        Swap {
            to,
            token_in,
            token_out,
            amount_in: in_raw,
            amount_out,
        }
        .publish(&e);

        in_raw
    }

    /// Current reserves in raw token units, in token order.
    fn get_reserves(e: Env) -> Vec<i128> {
        let pool = pool::read_pool(&e);
        let mut out = Vec::new(&e);
        for t in pool.tokens.iter() {
            out.push_back(t.to_raw(t.reserve));
        }
        out
    }

    /// The pool's token addresses, in token order.
    fn get_tokens(e: Env) -> Vec<Address> {
        let pool = pool::read_pool(&e);
        let mut out = Vec::new(&e);
        for t in pool.tokens.iter() {
            out.push_back(t.token);
        }
        out
    }

    /// The current amplification *factor* (effective A), reflecting any ramp in
    /// progress at the current ledger time.
    fn get_amp(e: Env) -> u32 {
        let pool = pool::read_pool(&e);
        (pool::current_amp(&pool, e.ledger().timestamp()) / AMP_PRECISION) as u32
    }

    // --- admin (owner-gated via `#[only_owner]`; ownership itself is managed by
    //     the `Ownable` impl below) ---

    /// Start (or replace) a linear amplification ramp toward `target_factor`
    /// over `duration` seconds. The ramp begins from the current interpolated
    /// factor, so there is no discontinuity. `duration == 0` applies it at once.
    #[only_owner]
    fn set_amp_ramp(e: Env, target_factor: u32, duration: u64) {
        if !pool::is_valid_amp_factor(target_factor) {
            panic_with_error!(&e, Error::InvalidAmpFactor);
        }
        let mut pool = pool::read_pool(&e);
        let now = e.ledger().timestamp();
        let current_factor = (pool::current_amp(&pool, now) / AMP_PRECISION) as u32;
        pool.amp_initial_factor = current_factor;
        pool.amp_target_factor = target_factor;
        pool.ramp_start_ts = now;
        pool.ramp_stop_ts = now + duration;
        pool::write_pool(&e, &pool);
        pool::extend_instance_ttl(&e);
    }

    /// Set the swap fee (1e9 == 100%), within the configured fee range.
    #[only_owner]
    fn set_swap_fee(e: Env, swap_fee: u64) {
        if !pool::is_valid_swap_fee(swap_fee) {
            panic_with_error!(&e, Error::InvalidSwapFee);
        }
        let mut pool = pool::read_pool(&e);
        pool.swap_fee = swap_fee;
        pool::write_pool(&e, &pool);
        pool::extend_instance_ttl(&e);
    }

    /// Set the protocol's cut of the swap fee (1e9 == 100% of the swap fee).
    #[only_owner]
    fn set_protocol_fee(e: Env, protocol_fee: u64) {
        if !pool::is_valid_protocol_fee(protocol_fee) {
            panic_with_error!(&e, Error::InvalidProtocolFee);
        }
        let mut pool = pool::read_pool(&e);
        pool.protocol_fee = protocol_fee;
        pool::write_pool(&e, &pool);
        pool::extend_instance_ttl(&e);
    }

    /// Set the address that receives the protocol fee.
    #[only_owner]
    fn set_beneficiary(e: Env, beneficiary: Address) {
        let mut pool = pool::read_pool(&e);
        pool.beneficiary = beneficiary;
        pool::write_pool(&e, &pool);
        pool::extend_instance_ttl(&e);
    }

    /// Set the cap on total LP-share supply (the pool's own token).
    #[only_owner]
    fn set_max_supply(e: Env, max_supply: i128) {
        capped::set_cap(&e, max_supply);
        pool::extend_instance_ttl(&e);
    }

    /// Set the per-token reserve cap (in `token`'s raw units). Must be >= the
    /// current reserve and within the safe math range.
    #[only_owner]
    fn set_token_cap(e: Env, token: Address, max_cap: i128) {
        let mut pool = pool::read_pool(&e);
        let i = pool::token_index(&pool, &token)
            .unwrap_or_else(|| panic_with_error!(&e, Error::UnknownToken));
        let mut t = pool.tokens.get(i as u32).unwrap();
        let cap_int = match pool::to_internal(max_cap, t.scaling_factor, t.scaling_up) {
            Some(c) if c >= t.reserve => c,
            _ => panic_with_error!(&e, Error::InvalidCap),
        };
        t.max_cap = cap_int;
        pool.tokens.set(i as u32, t);
        pool::write_pool(&e, &pool);
        pool::extend_instance_ttl(&e);
    }

    /// Pause the pool: blocks deposit/withdraw/swap until unpaused.
    #[only_owner]
    fn pause(e: Env) {
        pausable::pause(&e);
    }

    /// Resume a paused pool.
    #[only_owner]
    fn unpause(e: Env) {
        pausable::unpause(&e);
    }

    /// Whether the pool is currently paused.
    fn paused(e: Env) -> bool {
        pausable::paused(&e)
    }
}

/// Convert an i128 (e.g. an LP balance) into the u64 the math layer works in,
/// trapping if it doesn't fit.
fn unwrap_u64(e: &Env, value: i128) -> u64 {
    match u64::try_from(value) {
        Ok(v) => v,
        Err(_) => panic_with_error!(e, Error::BalanceTooLarge),
    }
}

/// Resolve a (token_in, token_out) address pair to distinct pool token indices.
fn swap_indices(e: &Env, pool: &Pool, token_in: &Address, token_out: &Address) -> (usize, usize) {
    let i = pool::token_index(pool, token_in)
        .unwrap_or_else(|| panic_with_error!(e, Error::UnknownToken));
    let j = pool::token_index(pool, token_out)
        .unwrap_or_else(|| panic_with_error!(e, Error::UnknownToken));
    if i == j {
        panic_with_error!(e, Error::SameToken);
    }
    (i, j)
}

/// Move the tokens and update reserves for a swap: pull `in_raw` of `token_in`
/// from `to`, pay `net_out` (internal) of `token_out` to `to`, route `protocol`
/// (internal) to the beneficiary, and keep the rest. Reserves are updated from
/// actual raw balance deltas so low-decimal rounding dust stays accounted for.
struct SwapCommit {
    token_in_index: usize,
    token_out_index: usize,
    in_raw: i128,
    net_out: u64,
    protocol: u64,
}

fn commit_swap(e: &Env, pool: &mut Pool, to: &Address, swap: SwapCommit) {
    let beneficiary = pool.beneficiary.clone();
    let mut t_in = pool.tokens.get(swap.token_in_index as u32).unwrap();
    let mut t_out = pool.tokens.get(swap.token_out_index as u32).unwrap();

    let out_raw = t_out.to_raw(swap.net_out);
    let protocol_raw = t_out.to_raw(swap.protocol);

    // The amount that physically leaves the out reserve is the user's net output
    // plus the protocol cut; the LP portion of the fee stays in the reserve. The
    // reserve is debited below from the *actual* transfer deltas (which also
    // guards against an underflow), so no separate pre-check is needed here.
    let contract = e.current_contract_address();
    let actual_in_int = transfer_in_exact(e, &t_in, to, &contract, swap.in_raw);
    let actual_net_out_int = transfer_out_exact(e, &t_out, &contract, to, out_raw);
    let actual_protocol_int = transfer_out_exact(e, &t_out, &contract, &beneficiary, protocol_raw);

    t_in.reserve = t_in
        .reserve
        .checked_add(actual_in_int)
        .filter(|r| *r <= t_in.max_cap)
        .unwrap_or_else(|| panic_with_error!(e, Error::CapExceeded));
    t_out.reserve = t_out
        .reserve
        .checked_sub(
            actual_net_out_int
                .checked_add(actual_protocol_int)
                .unwrap_or_else(|| panic_with_error!(e, Error::MathError)),
        )
        .unwrap_or_else(|| panic_with_error!(e, Error::MathError));
    pool.tokens.set(swap.token_in_index as u32, t_in);
    pool.tokens.set(swap.token_out_index as u32, t_out);
}

/// Pull an inbound token amount and verify the pool received exactly that raw
/// amount. This rejects fee-on-transfer or otherwise non-standard tokens before
/// reserves are persisted or any swap output is paid.
fn transfer_in_exact(
    e: &Env,
    token: &PoolToken,
    from: &Address,
    pool: &Address,
    amount: i128,
) -> u64 {
    if amount == 0 {
        return 0;
    }

    let client = token::Client::new(e, &token.token);
    let before = client.balance(pool);
    client.transfer(from, pool, &amount);
    let after = client.balance(pool);

    match after.checked_sub(before) {
        Some(delta) if delta == amount => raw_delta_to_internal(e, token, delta),
        _ => panic_with_error!(e, Error::TransferAmountMismatch),
    }
}

/// Send a raw token amount from the pool and return the actual internal reserve
/// decrease. The raw delta checks keep reserves tied to the token contract's
/// balance accounting and ensure the recipient actually received the quoted
/// amount.
fn transfer_out_exact(
    e: &Env,
    token: &PoolToken,
    pool: &Address,
    to: &Address,
    amount: i128,
) -> u64 {
    if amount == 0 {
        return 0;
    }

    let client = token::Client::new(e, &token.token);
    let pool_before = client.balance(pool);
    let to_before = client.balance(to);
    client.transfer(pool, to, &amount);
    let pool_after = client.balance(pool);
    let to_after = client.balance(to);

    match (
        pool_before.checked_sub(pool_after),
        to_after.checked_sub(to_before),
    ) {
        (Some(pool_delta), Some(to_delta)) if pool_delta == amount && to_delta == amount => {
            raw_delta_to_internal(e, token, pool_delta)
        }
        _ => panic_with_error!(e, Error::TransferAmountMismatch),
    }
}

fn raw_delta_to_internal(e: &Env, token: &PoolToken, delta: i128) -> u64 {
    token
        .to_internal(delta)
        .unwrap_or_else(|| panic_with_error!(e, Error::InvalidAmount))
}

// --- LP-share token ---
//
// The pool is its own SEP-41 LP token, backed by OpenZeppelin's fungible `Base`.
// The operations above mint/burn shares via `Base::mint` / `Base::update`; this
// `Base`-backed default impl supplies transfers, allowance, balances, and
// metadata. Direct burns are disabled below because LP exits must update pool
// reserves through `withdraw` or `withdraw_one_token`.

#[contractimpl(contracttrait)]
impl FungibleToken for LiquidityPool {
    type ContractType = Base;
}

#[contractimpl(contracttrait)]
impl FungibleBurnable for LiquidityPool {
    fn burn(e: &Env, _from: Address, _amount: i128) {
        panic_with_error!(e, Error::DirectLpBurnDisabled);
    }

    fn burn_from(e: &Env, _spender: Address, _from: Address, _amount: i128) {
        panic_with_error!(e, Error::DirectLpBurnDisabled);
    }
}

// 2-step ownership (get_owner, transfer_ownership, accept_ownership,
// renounce_ownership). The constructor seeds the owner via `ownable::set_owner`;
// these default impls are auth-enforced by OpenZeppelin.
#[contractimpl(contracttrait)]
impl Ownable for LiquidityPool {}
