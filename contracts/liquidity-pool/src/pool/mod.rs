// The pool's state model and the boundary layer between the contract's `i128`
// token amounts and the `u64` (9-decimal) world the math operates in. Split into:
//
//   state    - the persisted Pool/PoolToken types, instance storage, math bridges
//   fee      - swap-fee validation and output-fee quotes
//   normalized - fixed-capacity normalized amount vectors for math seams
//   scaling  - raw <-> internal (9-decimal) conversion
//   amp      - amplification, with a linear ramp over time
//   quote    - stable-math quotes for deposits, exits, and swaps
//
// Some helpers aren't wired into the contract yet, so unused-code and
// unused-re-export warnings are silenced for the module tree.
#![allow(dead_code, unused_imports)]

mod amp;
mod fee;
mod normalized;
mod quote;
mod scaling;
mod state;

pub use amp::{current_amp, is_valid_amp_factor, ramp_amp};
pub use fee::{is_valid_protocol_fee, is_valid_swap_fee};
pub use normalized::NormalizedAmounts;
pub use quote::{
    deposit_exact_tokens_in, swap_exact_in, swap_exact_out, withdraw_one_token,
    withdraw_proportional,
};
pub use scaling::{from_internal, scaling_for, to_internal, INTERNAL_DECIMALS};
pub use state::{
    extend_instance_ttl, has_pool, normalize_amounts, read_pool, reserves, token_index, write_pool,
    Pool, PoolToken,
};
