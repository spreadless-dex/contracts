// Amplification coefficient, with a linear ramp over time. The value returned is
// `factor * AMP_PRECISION` — the `amplification` argument expected by
// `math::calc_*` (effective A = factor).

use crate::math::{AMP_PRECISION, MAX_AMP, MIN_AMP};

use super::state::Pool;

/// Shortest allowed amplification ramp, and the cooldown between ramp starts
/// (one day). Together with `MAX_AMP_CHANGE` this bounds how fast `A` can move,
/// so LPs can't be sandwiched around an abrupt amplification change.
pub const MIN_RAMP_DURATION: u64 = 86_400;

/// A single ramp may move `A` by at most this factor, up or down.
pub const MAX_AMP_CHANGE: u64 = 10;

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

/// Whether a ramp from `current_factor` to `target_factor` stays within
/// `MAX_AMP_CHANGE` in either direction.
pub fn is_valid_amp_change(current_factor: u32, target_factor: u32) -> bool {
    let (current, target) = (current_factor as u64, target_factor as u64);
    target <= current * MAX_AMP_CHANGE && current <= target * MAX_AMP_CHANGE
}
