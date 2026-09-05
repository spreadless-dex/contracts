#![no_std]

//! The Spreadless pool contract's entrypoints, minus the deploy-time
//! constructor. The contract implements this trait via
//! `#[contractimpl(contracttrait)]`, so the interface and implementation cannot
//! drift (a mismatch won't compile), and the same macro generates the
//! `SpreadlessPoolInterfaceClient` used by other contracts.
//!
//! The constructor is separate (constructors can't be trait methods), exposed as
//! the inherent `__constructor(owner, protocol_controller, tokens, amp_factor,
//! amp_control, swap_fee, protocol_fee, beneficiary, max_caps, lp_max_supply,
//! lp_name, lp_symbol)`.
//!
//! ## Conventions
//! - **Amounts** are in each token's own raw on-chain units (`i128`, must be
//!   nonnegative). Balances are tracked internally at 9 decimals, so tokens with MORE
//!   than 9 decimals lose sub-9-decimal precision (truncated toward the pool).
//!   Stellar SAC assets are 7 decimals → lossless.
//! - **Fees** (`swap_fee`, `protocol_fee`) are fixed-point where `1e9` == 100%.
//!   `swap_fee` range: 10_000 (0.001%) ..= 10_000_000 (1%). `protocol_fee` is a
//!   cut OF the swap fee, range 0 ..= 1e9. It applies wherever a swap fee is
//!   charged: swaps, the imbalanced portion of a deposit, and single-token
//!   withdrawals. The cut is paid in the output token for swaps and single-token
//!   withdrawals, and as freshly minted LP shares for deposits (a join has no
//!   single output token). The rest of each fee stays in the pool for LPs;
//!   proportional withdrawals are fee-free.
//! - **Amplification** is an integer factor `A` in `[1, 50000]`. Its control is
//!   irreversibly either locked or delegated to the protocol controller.
//! - **LP shares** are the pool contract's own SEP-41 token (9 decimals).
//! - **Token order / indices**: `tokens` is sorted ascending at init; reserves,
//!   `amounts_in`, and `min_amounts_out` are all in that order.
//! - Failures revert with a typed `Error` (codes 1..=22, see `error.rs`);
//!   missing authorization reverts with a host auth error.

#![allow(dead_code)] // trait methods aren't "used" on a plain host build

use soroban_sdk::{contracttrait, contracttype, Address, Env, Vec};

/// Determines whether the pool's amplification factor is permanently fixed or
/// delegated to the immutable protocol controller.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[contracttype]
pub enum AmpControl {
    Locked,
    ProtocolManaged,
}

/// The contract's entrypoints; see the module docs for conventions.
#[contracttrait]
pub trait SpreadlessPoolInterface {
    // --- liquidity (require `to`'s auth; deposits are blocked while paused) ---

    /// Add liquidity with an exact `amounts_in` (one per token, in token order)
    /// and mint LP shares to `to`. Returns the LP shares minted.
    ///
    /// * First deposit (supply == 0): every amount must be > 0; LP minted equals
    ///   the StableSwap invariant `D` of the deposited balances.
    /// * Later deposits may be proportional, unbalanced, or single-sided (zeros
    ///   allowed); the swap fee applies only to the imbalanced portion.
    /// * When a `protocol_fee` is set, its cut of that imbalance fee is minted as
    ///   additional LP shares to the beneficiary; the depositor's shares are
    ///   unaffected.
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
    /// the raw amount paid. When a `protocol_fee` is set, its cut of that swap
    /// fee is paid to the beneficiary in `token_out`; the caller's payout is
    /// unaffected.
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

    /// Immutable address authorized to perform protocol administration.
    fn get_protocol_controller(e: Env) -> Address;

    /// Immutable amplification-control mode selected at pool creation.
    fn get_amp_control(e: Env) -> AmpControl;

    fn get_swap_fee(e: Env) -> u64;

    fn get_protocol_fee(e: Env) -> u64;

    fn get_beneficiary(e: Env) -> Address;

    fn get_max_supply(e: Env) -> i128;

    /// Whether the pool is paused (deposits and swaps blocked; exits remain open).
    fn paused(e: Env) -> bool;

    // --- protocol administration (requires protocol-controller auth) ---

    /// Start/replace a linear amp ramp toward `target_factor` (in `[1,50000]`)
    /// on a protocol-managed pool. `duration == 0` applies it at once.
    /// Reverts: `InvalidAmpFactor`, `AmpControlLocked`.
    fn set_amp_ramp(e: Env, target_factor: u32, duration: u64);

    /// Set the protocol's cut of the swap fee (in `[0, 1e9]`).
    /// Reverts: `InvalidProtocolFee`.
    fn set_protocol_fee(e: Env, protocol_fee: u64);

    /// Set the address that receives the protocol fee.
    fn set_beneficiary(e: Env, beneficiary: Address);

    /// Protocol-controller pause path. Uses the same shared pause flag as the
    /// owner path.
    fn protocol_pause(e: Env);

    /// Protocol-controller unpause path.
    fn protocol_unpause(e: Env);

    // --- pool-owner administration ---

    /// Set the swap fee (1e9 == 100%, in `[10_000, 10_000_000]`).
    /// Reverts: `InvalidSwapFee`.
    fn set_swap_fee(e: Env, swap_fee: u64);

    /// Set the cap on total LP-share supply.
    fn set_max_supply(e: Env, max_supply: i128);

    /// Set a token's reserve cap (raw units); must be >= its current reserve.
    /// Reverts: `UnknownToken`, `InvalidCap`.
    fn set_token_cap(e: Env, token: Address, max_cap: i128);

    /// Pause the pool (blocks deposits and swaps, but never withdrawals).
    fn pause(e: Env);

    /// Resume a paused pool.
    fn unpause(e: Env);
}

// ---------------------------------------------------------------------------
// Also exposed by the contract (and on `SpreadlessPoolClient`) from OpenZeppelin,
// not redeclared above to avoid drift:
//
// 2-step ownership (stellar_access::ownable::Ownable):
//   get_owner(e) -> Option<Address>
//   transfer_ownership(e, new_owner: Address, live_until_ledger: u32)  // owner auth
//   accept_ownership(e)                                                // pending-owner auth
//   renounce_ownership(e)                                              // always reverts
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
// LP exits must use `withdraw`/`withdraw_one_token`; direct burns are disabled
// so total supply cannot be reduced without updating reserves.
// ---------------------------------------------------------------------------
