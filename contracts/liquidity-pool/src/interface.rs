//! Black-box interface for the `LiquidityPool` contract.
//!
//! This is a *reference* trait — the contract exposes exactly these entrypoints
//! (via its inherent `impl LiquidityPool`); it does not literally implement this
//! trait. Read it to write tests without touching the implementation.
//!
//! Tests don't call the trait — they use the macro-generated
//! `LiquidityPoolClient`, whose methods mirror these but (a) drop the leading
//! `Env` and (b) take args by reference. For example:
//!     fn deposit(e: Env, to: Address, amounts_in: Vec<i128>, min_lp_out: i128) -> i128
//! becomes, on the client:
//!     client.deposit(&to, &amounts_in, &min_lp_out) -> i128
//!
//! Construct one in a test with the constructor args as a tuple, IN ORDER:
//!     let id = env.register(LiquidityPool, (owner, tokens, amp_factor, swap_fee,
//!         protocol_fee, beneficiary, max_caps, lp_max_supply));
//!     let client = LiquidityPoolClient::new(&env, &id);
//!
//! ## Conventions
//! - **Amounts** are in each token's own raw on-chain units (`i128`, must be
//!   nonnegative). Balances are tracked internally at 9 decimals, so tokens with MORE
//!   than 9 decimals lose sub-9-decimal precision (truncated toward the pool).
//!   Stellar SAC assets are 7 decimals → lossless.
//! - **Fees** (`swap_fee`, `protocol_fee`) are fixed-point where `1e9` == 100%.
//!   `swap_fee` range: 10_000 (0.001%) ..= 10_000_000 (1%). `protocol_fee` is a
//!   cut OF the swap fee, range 0 ..= 1e9. The protocol cut is taken on swaps
//!   only; the swap fee charged on the imbalanced portion of a deposit or a
//!   single-token withdrawal stays entirely in the pool for LPs (no protocol
//!   cut).
//! - **Amplification** is an integer factor `A` in `[1, 12000]`.
//! - **LP shares** are the pool contract's own SEP-41 token (9 decimals).
//! - **Token order / indices**: `tokens` is sorted ascending at init; reserves,
//!   `amounts_in`, and `min_amounts_out` are all in that order.
//! - Failures revert with a typed `Error` (codes 1..=20, see `error.rs`);
//!   missing authorization reverts with a host auth error.

// The contract implements this trait (for wasm + tests), but a plain host
// `cargo build`/`cargo check` doesn't "use" it, which trips dead-code. Silence
// it — the wasm and test builds are warning-clean.
#![allow(dead_code)]

use soroban_sdk::{contracttrait, Address, Env, Vec};

/// The contract's entrypoints — everything except the deploy-time constructor.
/// `LiquidityPool` implements this via `#[contractimpl(contracttrait)]`, so the
/// implementation and this interface cannot drift (a mismatch won't compile).
///
/// The constructor is exposed separately as the contract's inherent
/// `__constructor(owner, tokens, amp_factor, swap_fee, protocol_fee,
/// beneficiary, max_caps, lp_max_supply)` — see the module docs above for its
/// parameters and validation (constructors can't be trait methods).
#[contracttrait]
pub trait LiquidityPoolInterface {
    // --- liquidity (require `to`'s auth; blocked while paused) ---

    /// Add liquidity with an exact `amounts_in` (one per token, in token order)
    /// and mint LP shares to `to`. Returns the LP shares minted.
    ///
    /// * First deposit (supply == 0): every amount must be > 0; LP minted equals
    ///   the StableSwap invariant `D` of the deposited balances.
    /// * Later deposits may be proportional, unbalanced, or single-sided (zeros
    ///   allowed); the swap fee applies only to the imbalanced portion.
    ///
    /// Reverts: `AmountsLengthMismatch`, `InvalidAmount`, `ZeroDeposit`,
    /// `FirstDepositNotFull`, `SlippageExceeded` (minted < `min_lp_out`),
    /// `CapExceeded`, `MathError`, `TransferAmountMismatch`.
    fn deposit(e: Env, to: Address, amounts_in: Vec<i128>, min_lp_out: i128) -> i128;

    /// Burn `lp_amount` of `to`'s LP shares and pay out a PROPORTIONAL slice of
    /// every reserve (rounded down, favouring the pool). Returns the raw amounts
    /// paid, in token order.
    ///
    /// Reverts: `AmountsLengthMismatch`, `InvalidAmount` (`lp_amount <= 0`),
    /// `SlippageExceeded` (a payout < its `min_amounts_out`), `MathError`,
    /// `TransferAmountMismatch`, insufficient-balance (host) if `to` lacks the
    /// shares.
    fn withdraw(e: Env, to: Address, lp_amount: i128, min_amounts_out: Vec<i128>) -> Vec<i128>;

    /// Burn `lp_amount` of `to`'s LP shares and pay out a single token. The
    /// stable invariant is reduced by the burned share, then the selected token
    /// is withdrawn with swap-fee treatment on the imbalanced portion. Returns
    /// the raw amount paid.
    ///
    /// Reverts: `UnknownToken`, `InvalidAmount`, `SlippageExceeded` (payout <
    /// `min_amount_out`), `MathError`, `TransferAmountMismatch`,
    /// insufficient-balance (host) if `to` lacks the shares.
    fn withdraw_one_token(
        e: Env,
        to: Address,
        lp_amount: i128,
        token_out: Address,
        min_amount_out: i128,
    ) -> i128;

    // --- swaps (require `to`'s auth; blocked while paused) ---

    /// Swap an exact `amount_in` of `token_in` for `token_out`, paid to `to`,
    /// requiring at least `min_out`. Returns the amount of `token_out` sent.
    ///
    /// The swap fee is charged on the OUTPUT: `to` receives
    /// `out_without_fee * (1 - swap_fee)`. The `protocol_fee` cut of that fee is
    /// sent to the beneficiary (in `token_out`); the remainder stays in the pool
    /// (raising the invariant for LPs).
    ///
    /// Reverts: `UnknownToken`, `SameToken`, `InvalidAmount`,
    /// `SlippageExceeded` (out < `min_out`), `CapExceeded`, `MathError`,
    /// `TransferAmountMismatch`.
    fn swap_exact_in(
        e: Env,
        to: Address,
        token_in: Address,
        token_out: Address,
        amount_in: i128,
        min_out: i128,
    ) -> i128;

    /// Swap `token_in` for an EXACT `amount_out` of `token_out` paid to `to`,
    /// spending at most `max_in`. Returns the amount of `token_in` taken.
    ///
    /// Reverts: `UnknownToken`, `SameToken`, `InvalidAmount`,
    /// `SlippageExceeded` (required input > `max_in`), `MathError`,
    /// `TransferAmountMismatch`.
    fn swap_exact_out(
        e: Env,
        to: Address,
        token_in: Address,
        token_out: Address,
        amount_out: i128,
        max_in: i128,
    ) -> i128;

    // --- views (no auth) ---

    /// Current reserves in raw token units, in token order.
    fn get_reserves(e: Env) -> Vec<i128>;

    /// The pool's token addresses, in token order (sorted ascending).
    fn get_tokens(e: Env) -> Vec<Address>;

    /// Current amplification factor `A`, reflecting any ramp in progress.
    fn get_amp(e: Env) -> u32;

    /// Whether the pool is paused (deposit/withdraw/swap blocked).
    fn paused(e: Env) -> bool;

    // --- admin (ALL require the OWNER's auth) ---

    /// Start/replace a linear amp ramp toward `target_factor` (in `[1,12000]`)
    /// over `duration` seconds, starting from the current factor (no jump);
    /// `duration == 0` applies it at once. Reverts: `InvalidAmpFactor`.
    fn set_amp_ramp(e: Env, target_factor: u32, duration: u64);

    /// Set the swap fee (1e9 == 100%, in `[10_000, 10_000_000]`).
    /// Reverts: `InvalidSwapFee`.
    fn set_swap_fee(e: Env, swap_fee: u64);

    /// Set the protocol's cut of the swap fee (in `[0, 1e9]`).
    /// Reverts: `InvalidProtocolFee`.
    fn set_protocol_fee(e: Env, protocol_fee: u64);

    /// Set the address that receives the protocol fee.
    fn set_beneficiary(e: Env, beneficiary: Address);

    /// Set the cap on total LP-share supply.
    fn set_max_supply(e: Env, max_supply: i128);

    /// Set a token's reserve cap (raw units); must be >= its current reserve.
    /// Reverts: `UnknownToken`, `InvalidCap`.
    fn set_token_cap(e: Env, token: Address, max_cap: i128);

    /// Pause the pool (blocks deposit/withdraw/swap).
    fn pause(e: Env);

    /// Resume a paused pool.
    fn unpause(e: Env);
}

// ---------------------------------------------------------------------------
// Also exposed by the contract (and on `LiquidityPoolClient`), from OpenZeppelin
// — not redeclared above to avoid drift, but available to tests:
//
// 2-step ownership (stellar_access::ownable::Ownable):
//   get_owner(e) -> Option<Address>
//   transfer_ownership(e, new_owner: Address, live_until_ledger: u32)  // owner auth
//   accept_ownership(e)                                                // pending-owner auth
//   renounce_ownership(e)                                              // owner auth
//
// SEP-41 LP-share token — the pool IS its own token, 9 decimals
// (stellar_tokens::fungible::{FungibleToken, FungibleBurnable}):
//   balance(e, id: Address) -> i128
//   total_supply(e) -> i128
//   allowance(e, owner: Address, spender: Address) -> i128
//   approve(e, owner: Address, spender: Address, amount: i128, live_until_ledger: u32)
//   transfer(e, from: Address, to: Address, amount: i128)
//   transfer_from(e, spender: Address, from: Address, to: Address, amount: i128)
//   burn(e, from: Address, amount: i128)                         // always reverts
//   burn_from(e, spender: Address, from: Address, amount: i128)   // always reverts
//   decimals(e) -> u32          // == 9
//   name(e) -> String
//   symbol(e) -> String
//
// LP exits must use `withdraw` or `withdraw_one_token`; direct burns are
// disabled so total supply cannot be reduced without updating reserves.
// ---------------------------------------------------------------------------
