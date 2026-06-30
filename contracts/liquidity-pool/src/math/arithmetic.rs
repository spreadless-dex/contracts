// Low-level integer arithmetic shared by the stable-swap math.
//
// - u64 mul-div with explicit rounding (replaces Solana's `bn::safe_math`).
// - `soroban_sdk::U256` host-integer wrappers (replace the Solana `bn::U192`).
//   U256's own ops (`add`/`sub`/`mul`/`div`) trap on overflow or division-by-zero
//   rather than returning `Option`; these wrappers re-introduce the `Option`
//   semantics the original code relied on for cases that can legitimately occur
//   with edge inputs (division by zero, subtraction underflow). Overflow past
//   256 bits cannot occur for in-range balances (intermediates stay well under
//   2^160), so it is left to trap, which is the correct behaviour in a contract.

use soroban_sdk::{Env, U256};

// ceil(value * mul / div) using a u128 intermediate.
pub(super) fn mul_div_up_u64(value: u64, mul: u64, div: u64) -> Option<u64> {
    if div == 0 {
        return None;
    }
    let product = (value as u128).checked_mul(mul as u128)?;
    if product == 0 {
        return Some(0);
    }
    u64::try_from((product - 1) / div as u128 + 1).ok()
}

// floor(value * mul / div) using a u128 intermediate. Used for proportional
// withdrawals, where rounding down favours the pool. Re-exported by `math` for
// the contract layer, so it must be at least crate-visible.
pub(crate) fn mul_div_down_u64(value: u64, mul: u64, div: u64) -> Option<u64> {
    if div == 0 {
        return None;
    }
    let product = (value as u128).checked_mul(mul as u128)?;
    u64::try_from(product / div as u128).ok()
}

// Sum of a slice, returning None on overflow instead of trapping. The math
// guarantees each individual balance fits in u64 (<= MAX_SAFE_BALANCE), but the
// sum of post-deposit balances across up to MAX_TOKENS can exceed u64::MAX; this
// degrades that case to a clean `None` (-> MathError) like the other helpers.
pub(super) fn checked_sum(values: &[u64]) -> Option<u64> {
    values.iter().try_fold(0u64, |acc, &v| acc.checked_add(v))
}

pub(super) fn u256(e: &Env, value: u64) -> U256 {
    U256::from_u128(e, value as u128)
}

pub(super) fn to_u64(value: &U256) -> Option<u64> {
    value.to_u128().and_then(|v| u64::try_from(v).ok())
}

// floor(num1 * num2 / den)
pub(super) fn mul_div_down(num1: &U256, num2: &U256, den: &U256, zero: &U256) -> Option<U256> {
    if den == zero {
        return None;
    }
    Some(num1.mul(num2).div(den))
}

// ceil(num1 * num2 / den)
pub(super) fn mul_div_up(
    num1: &U256,
    num2: &U256,
    den: &U256,
    zero: &U256,
    one: &U256,
) -> Option<U256> {
    if den == zero {
        return None;
    }
    let product = num1.mul(num2);
    if &product == zero {
        return Some(zero.clone());
    }
    Some(product.sub(one).div(den).add(one))
}

// ceil(num / den)
pub(super) fn div_up(num: &U256, den: &U256, zero: &U256, one: &U256) -> Option<U256> {
    if den == zero {
        return None;
    }
    if num == zero {
        return Some(zero.clone());
    }
    Some(num.sub(one).div(den).add(one))
}

pub(super) fn checked_sub(a: &U256, b: &U256) -> Option<U256> {
    if a < b {
        return None;
    }
    Some(a.sub(b))
}
