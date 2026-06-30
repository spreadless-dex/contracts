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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scaling_derivation() {
        assert_eq!(scaling_for(7), Some((100, true))); // Stellar classic asset (SAC)
        assert_eq!(scaling_for(6), Some((1_000, true)));
        assert_eq!(scaling_for(9), Some((1, true)));
        assert_eq!(scaling_for(18), Some((1_000_000_000, false)));
    }

    #[test]
    fn scale_roundtrip_le9() {
        let (f, up) = scaling_for(6).unwrap();
        let raw = 1_234_567i128; // 1.234567 of a 6-dec token
        let internal = to_internal(raw, f, up).unwrap();
        assert_eq!(internal, 1_234_567_000); // @9-dec
        assert_eq!(from_internal(internal, f, up), raw);
    }

    #[test]
    fn scale_roundtrip_gt9_truncates() {
        let (f, up) = scaling_for(18).unwrap();
        let raw = 1_234_567_000_000_000_000i128; // 1.234567 of an 18-dec token
        let internal = to_internal(raw, f, up).unwrap();
        assert_eq!(internal, 1_234_567_000); // @9-dec
        assert_eq!(from_internal(internal, f, up), raw);
        // sub-9-decimal dust is truncated on the way in (favours the pool)
        let dusty = raw + 999_999_999;
        assert_eq!(to_internal(dusty, f, up).unwrap(), 1_234_567_000);
    }

    #[test]
    fn to_internal_bounds() {
        let (f, up) = scaling_for(9).unwrap();
        assert_eq!(to_internal(-1, f, up), None); // negative
        assert_eq!(
            to_internal(MAX_SAFE_BALANCE as i128, f, up),
            Some(MAX_SAFE_BALANCE)
        );
        assert_eq!(to_internal(MAX_SAFE_BALANCE as i128 + 1, f, up), None); // over ceiling
        let (f6, up6) = scaling_for(6).unwrap();
        assert_eq!(to_internal(i128::MAX, f6, up6), None); // multiply overflow
    }
}
