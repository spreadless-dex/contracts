// Fixed-point helpers, scaled so that 1.0 == ONE. Pure u64/u128 arithmetic so
// the module is self-contained under Soroban's `no_std` runtime.

// 100% in fixed-point. Matches the pool's internal 9-decimal balance scale
// (ONE = 10^INTERNAL_DECIMALS) and the fee-rate scale.
pub const ONE: u64 = 1_000_000_000;

const ONE_128: u128 = ONE as u128;

pub trait FixedMul {
    fn mul_down(self, other: u64) -> Option<u64>;
    fn mul_up(self, other: u64) -> Option<u64>;
}

pub trait FixedDiv {
    fn div_down(self, other: u64) -> Option<u64>;
    fn div_up(self, other: u64) -> Option<u64>;
}

pub trait FixedComplement {
    fn complement(self) -> u64;
}

impl FixedMul for u64 {
    fn mul_down(self, other: u64) -> Option<u64> {
        let product = (self as u128).checked_mul(other as u128)?;
        u64::try_from(product / ONE_128).ok()
    }

    fn mul_up(self, other: u64) -> Option<u64> {
        let product = (self as u128).checked_mul(other as u128)?;
        if product == 0 {
            Some(0)
        } else {
            // ceil(product / ONE)
            u64::try_from((product - 1) / ONE_128 + 1).ok()
        }
    }
}

impl FixedDiv for u64 {
    fn div_down(self, other: u64) -> Option<u64> {
        if other == 0 {
            return None;
        }
        if self == 0 {
            return Some(0);
        }
        let inflated = (self as u128).checked_mul(ONE_128)?;
        u64::try_from(inflated / other as u128).ok()
    }

    fn div_up(self, other: u64) -> Option<u64> {
        if other == 0 {
            return None;
        }
        if self == 0 {
            return Some(0);
        }
        let inflated = (self as u128).checked_mul(ONE_128)?;
        // ceil(inflated / other)
        u64::try_from((inflated - 1) / other as u128 + 1).ok()
    }
}

impl FixedComplement for u64 {
    fn complement(self) -> u64 {
        ONE.saturating_sub(self)
    }
}
