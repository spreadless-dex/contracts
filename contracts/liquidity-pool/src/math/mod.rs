// Stable-swap math, ported from Stabble (a Balancer-V2 fork): `stable_math.rs`,
// `fixed_math.rs`, and the `bn` U192 big-integer crate. The shared constants and
// the public surface live here; the implementation is split across submodules:
//
//   stable      - the invariant + swap/join/exit formulas (the `calc_*` fns)
//   base        - proportional join/exit amounts shared by every pool type
//   fixed_math  - 9-decimal fixed-point helpers (mul/div/complement)
//   arithmetic  - u64 mul-div and the soroban_sdk::U256 host-integer wrappers
//
// Several entry points aren't wired into the contract yet, so unused-code and
// unused-re-export warnings are silenced for the whole module tree.
#![allow(dead_code, unused_imports)]

mod arithmetic;
mod base;
pub(crate) mod fixed_math;
mod stable;

pub(crate) use arithmetic::mul_div_down_u64;
pub use base::{compute_proportional_amounts_in, compute_proportional_amounts_out};
pub use stable::{
    calc_in_given_out, calc_invariant, calc_out_given_in,
    calc_pool_token_out_given_exact_tokens_in, calc_token_out_given_exact_pool_token_in,
};

pub const AMP_PRECISION: u64 = 1_000;

pub const MIN_AMP: u16 = 1;
pub const MAX_AMP: u16 = 12000;

pub const MIN_SWAP_FEE: u64 = 10_000; // 0.001%
pub const MAX_SWAP_FEE: u64 = 10_000_000; // 1%

// Safe max balance supported by the stable math.
pub const MAX_SAFE_BALANCE: u64 = 3_000_000_000_000_000_000; // 3B

pub const DEFAULT_INV_THRESHOLD: u64 = 100;
pub const BALANCE_THRESHOLD: u64 = 1;

pub const MIN_TOKENS: usize = 2;
pub const MAX_TOKENS: usize = 5;
