// Boundary scaling: raw i128 token amounts <-> normalized u64 internal balances.
// All internal balances live at INTERNAL_DECIMALS (9), matching `math::fixed_math`
// (ONE = 1e9) and the invariant `D`. The const assert statically pins the two
// together so they can never drift apart.

use crate::math::MAX_SAFE_BALANCE;

pub const INTERNAL_DECIMALS: u32 = 9;
const _: () = assert!(10u64.pow(INTERNAL_DECIMALS) == crate::math::fixed_math::ONE);

/// Derive `(scaling_factor, scaling_up)` for a token with the given `decimals`.
/// Returns `None` only for absurd decimal counts that overflow the factor.
pub fn scaling_for(decimals: u32) -> Option<(u64, bool)> {
    if decimals <= INTERNAL_DECIMALS {
        Some((10u64.checked_pow(INTERNAL_DECIMALS - decimals)?, true))
    } else {
        Some((10u64.checked_pow(decimals - INTERNAL_DECIMALS)?, false))
    }
}

/// Raw token amount (i128) -> normalized internal balance (u64 @ 9-dec).
/// Rounds down (truncates sub-precision dust for tokens with > 9 decimals).
/// Returns `None` if the amount is negative or exceeds `MAX_SAFE_BALANCE`.
pub fn to_internal(raw: i128, scaling_factor: u64, scaling_up: bool) -> Option<u64> {
    if raw < 0 {
        return None;
    }
    let factor = scaling_factor as i128;
    let scaled = if scaling_up {
        raw.checked_mul(factor)?
    } else {
        raw / factor
    };
    let scaled = u64::try_from(scaled).ok()?;
    if scaled > MAX_SAFE_BALANCE {
        None
    } else {
        Some(scaled)
    }
}

/// Normalized internal balance (u64 @ 9-dec) -> raw token amount (i128).
/// For tokens with <= 9 decimals this rounds down (any sub-precision remainder
/// stays in the pool). Always fits in i128 for in-range balances.
pub fn from_internal(internal: u64, scaling_factor: u64, scaling_up: bool) -> i128 {
    if scaling_up {
        (internal / scaling_factor) as i128
    } else {
        (internal as i128) * (scaling_factor as i128)
    }
}

/// Like `from_internal` but rounds up. Used when charging a computed input
/// amount (e.g. exact-out swaps), so any sub-precision remainder favours the
/// pool rather than the caller. Exact for tokens with > 9 decimals.
pub fn from_internal_up(internal: u64, scaling_factor: u64, scaling_up: bool) -> i128 {
    if scaling_up {
        // ceil(internal / scaling_factor); the addition can't overflow u64 for
        // in-range balances (internal <= MAX_SAFE_BALANCE, factor <= 1e9).
        internal.div_ceil(scaling_factor) as i128
    } else {
        (internal as i128) * (scaling_factor as i128)
    }
}
