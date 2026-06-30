// Stable-swap math: the StableSwap invariant plus the swap/join/exit formulas.
// The shared constants and the public surface live here; the implementation is
// split across submodules:
//
//   stable      - the invariant + swap/join/exit formulas (the `calc_*` fns)
//   lp_exit     - proportional and single-token LP exit quotes
//   fixed_math  - 9-decimal fixed-point helpers (mul/div/complement)
//   arithmetic  - u64 mul-div and the soroban_sdk::U256 host-integer wrappers

mod arithmetic;
pub(crate) mod fixed_math;
mod lp_exit;
mod stable;

pub(crate) use lp_exit::{proportional_amounts_out, single_token_amount_out};
pub use stable::{
    calc_in_given_out, calc_invariant, calc_out_given_in,
    calc_pool_token_out_given_exact_tokens_in, calc_pool_token_out_no_fee,
};

pub const AMP_PRECISION: u64 = 1_000;

pub const MIN_AMP: u16 = 1;
pub const MAX_AMP: u16 = 12000;

// Safe max balance supported by the stable math.
pub const MAX_SAFE_BALANCE: u64 = 3_000_000_000_000_000_000; // 3B

pub const DEFAULT_INV_THRESHOLD: u64 = 100;
pub const BALANCE_THRESHOLD: u64 = 1;

pub const MIN_TOKENS: usize = 2;
pub const MAX_TOKENS: usize = 5;
