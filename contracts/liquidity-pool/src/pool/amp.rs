// Amplification coefficient, with a linear ramp over time. The value returned is
// `factor * AMP_PRECISION` — the `amplification` argument expected by
// `math::calc_*` (effective A = factor).

use crate::math::{AMP_PRECISION, MAX_AMP, MIN_AMP};

use super::state::Pool;

/// Interpolate the amplification factor linearly between `initial_factor` and
/// `target_factor` over `[start_ts, stop_ts]`; outside the window it clamps to
/// the endpoints, and a degenerate window (`stop_ts <= start_ts`) is treated as
/// static at the target. Result is `factor * AMP_PRECISION`.
pub fn ramp_amp(
    initial_factor: u32,
    target_factor: u32,
    start_ts: u64,
    stop_ts: u64,
    now: u64,
) -> u64 {
    let a0 = initial_factor as i128 * AMP_PRECISION as i128;
    let a1 = target_factor as i128 * AMP_PRECISION as i128;
    let amp = if stop_ts <= start_ts || now >= stop_ts {
        a1
    } else if now <= start_ts {
        a0
    } else {
        let elapsed = (now - start_ts) as i128;
        let duration = (stop_ts - start_ts) as i128;
        a0 + (a1 - a0) * elapsed / duration
    };
    amp as u64
}

pub fn current_amp(pool: &Pool, now: u64) -> u64 {
    ramp_amp(
        pool.amp_initial_factor,
        pool.amp_target_factor,
        pool.ramp_start_ts,
        pool.ramp_stop_ts,
        now,
    )
}

pub fn is_valid_amp_factor(factor: u32) -> bool {
    factor >= MIN_AMP as u32 && factor <= MAX_AMP as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn amp_static_and_ramp() {
        // static: initial == target -> factor * AMP_PRECISION
        assert_eq!(ramp_amp(5000, 5000, 0, 0, 100), 5_000_000);
        // degenerate window (stop <= start) -> target
        assert_eq!(ramp_amp(1000, 5000, 100, 100, 50), 5_000_000);
        // before / after the window -> endpoints
        assert_eq!(ramp_amp(1000, 5000, 100, 200, 50), 1_000_000);
        assert_eq!(ramp_amp(1000, 5000, 100, 200, 999), 5_000_000);
        // midpoint, ramp up and ramp down
        assert_eq!(ramp_amp(1000, 5000, 100, 200, 150), 3_000_000);
        assert_eq!(ramp_amp(5000, 1000, 100, 200, 150), 3_000_000);
    }

    #[test]
    fn amp_factor_validation() {
        assert!(!is_valid_amp_factor(0));
        assert!(is_valid_amp_factor(1));
        assert!(is_valid_amp_factor(12_000));
        assert!(!is_valid_amp_factor(12_001));
    }
}
