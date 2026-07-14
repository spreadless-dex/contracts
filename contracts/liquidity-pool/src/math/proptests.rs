// Property-based tests for the stable-swap math, complementing the
// deterministic vectors in `stable.rs`. Each case draws a random pool shape
// (2-5 tokens), balance distribution, amplification, trade size, and trade
// direction from the supported domain, then checks the properties a correct
// StableSwap must satisfy for *every* configuration — the "a formula could be
// subtly wrong only for some reserve configuration" class of bug.
//
// Supported domain exercised here (all amounts on the internal 9-dec scale):
//   balances   10,000 .. 100,000,000 tokens per reserve (ratios up to 1e4)
//   amp factor 1 ..= 12,000 (MIN_AMP..=MAX_AMP)
//   trade size 0.1% ..= 30% of the input-token reserve
// Convergence is asserted, not assumed: a `None` from the math inside this
// domain fails the test.

use proptest::prelude::*;
use soroban_sdk::Env;
use std::vec::Vec as StdVec;

use super::{calc_in_given_out, calc_invariant, calc_out_given_in, AMP_PRECISION};

const MIN_BALANCE: u64 = 10_000_000_000_000; // 10,000 tokens
const MAX_BALANCE: u64 = 100_000_000_000_000_000; // 100,000,000 tokens

fn arb_balances() -> impl Strategy<Value = StdVec<u64>> {
    proptest::collection::vec(MIN_BALANCE..=MAX_BALANCE, 2..=5)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn swap_quotes_preserve_the_invariant(
        balances in arb_balances(),
        amp_factor in 1u64..=12_000,
        size_bps in 10u64..=3_000,
        dir_seed in any::<usize>(),
    ) {
        let n = balances.len();
        let i = dir_seed % n;
        let j = (i + 1 + (dir_seed / n) % (n - 1)) % n;
        prop_assert!(i != j);

        let amplification = amp_factor * AMP_PRECISION;
        let amount_in = (balances[i] as u128 * size_bps as u128 / 10_000) as u64;

        let e = Env::default();
        e.cost_estimate().budget().reset_unlimited();

        let d_before = calc_invariant(&e, amplification, &balances, None)
            .expect("invariant must converge in the supported domain");
        let out = calc_out_given_in(&e, amplification, &balances, i, j, amount_in, d_before)
            .expect("exact-in quote must converge in the supported domain");

        // A swap can never pay out the entire reserve of the output token.
        prop_assert!(out < balances[j]);

        // The invariant never decreases, and only grows by rounding dust.
        // The decrease allowance is one Newton convergence threshold (the
        // two D computations may each stop within 100 units of the fixpoint);
        // the increase bound scales with D to stay meaningful for the larger
        // random pools while still excluding any real value leak.
        let mut after = balances.clone();
        after[i] += amount_in;
        after[j] -= out;
        let d_after = calc_invariant(&e, amplification, &after, None)
            .expect("post-swap invariant must converge");
        let delta = d_after as i128 - d_before as i128;
        prop_assert!(
            delta >= -100,
            "invariant decreased by {} (D {} -> {})",
            -delta,
            d_before,
            d_after
        );
        prop_assert!(
            delta <= (d_before / 1_000_000 + 10_000) as i128,
            "invariant grew beyond rounding: +{} on D {}",
            delta,
            d_before
        );

        // Output is monotonically increasing in the input, and concave
        // (doubling the trade never doubles the output): price impact grows
        // with trade size, never shrinks.
        let out_double =
            calc_out_given_in(&e, amplification, &balances, i, j, amount_in * 2, d_before)
                .expect("doubled exact-in quote must converge");
        prop_assert!(out_double >= out);
        prop_assert!(out_double <= out.saturating_mul(2).saturating_add(10));

        // The exact-out solver is the inverse of the exact-in solver:
        // quoting the input needed for `out` recovers `amount_in` within
        // rounding (sub-basis-point, plus a small absolute allowance for the
        // steep price region of heavily imbalanced pools).
        let recovered_in =
            calc_in_given_out(&e, amplification, &balances, i, j, out, d_before)
                .expect("exact-out quote must converge in the supported domain");
        let drift = recovered_in.abs_diff(amount_in);
        prop_assert!(
            drift <= amount_in / 10_000 + 2_000,
            "exact-in/exact-out roundtrip drifted by {} on input {}",
            drift,
            amount_in
        );
    }
}
