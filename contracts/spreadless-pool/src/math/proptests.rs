use proptest::prelude::*;
use soroban_sdk::Env;
use std::vec::Vec as StdVec;

use super::{calc_in_given_out, calc_invariant, calc_out_given_in, AMP_PRECISION};

const MIN_BALANCE: u64 = 10_000_000_000_000;
const MAX_BALANCE: u64 = 100_000_000_000_000_000;

fn arbitrary_balances() -> impl Strategy<Value = StdVec<u64>> {
    proptest::collection::vec(MIN_BALANCE..=MAX_BALANCE, 2..=5)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn swap_quotes_preserve_curve_properties(
        balances in arbitrary_balances(),
        amp_factor in 1_u64..=50_000,
        size_bps in 10_u64..=3_000,
        direction_seed in any::<usize>(),
    ) {
        let token_count = balances.len();
        let token_in = direction_seed % token_count;
        let token_out =
            (token_in + 1 + (direction_seed / token_count) % (token_count - 1)) % token_count;
        let amplification = amp_factor * AMP_PRECISION;
        let amount_in =
            (balances[token_in] as u128 * size_bps as u128 / 10_000) as u64;
        let e = Env::default();
        e.cost_estimate().budget().reset_unlimited();

        let invariant_before = calc_invariant(&e, amplification, &balances, None)
            .expect("supported balances must converge");
        let amount_out = calc_out_given_in(
            &e,
            amplification,
            &balances,
            token_in,
            token_out,
            amount_in,
            invariant_before,
        )
        .expect("supported exact-input quote must converge");
        prop_assert!(amount_out < balances[token_out]);

        let mut balances_after = balances.clone();
        balances_after[token_in] += amount_in;
        balances_after[token_out] -= amount_out;
        let invariant_after = calc_invariant(&e, amplification, &balances_after, None)
            .expect("post-swap balances must converge");
        let invariant_delta = invariant_after as i128 - invariant_before as i128;
        prop_assert!(invariant_delta >= -100);
        prop_assert!(
            invariant_delta <= (invariant_before / 1_000_000 + 10_000) as i128
        );

        let doubled_out = calc_out_given_in(
            &e,
            amplification,
            &balances,
            token_in,
            token_out,
            amount_in * 2,
            invariant_before,
        )
        .expect("doubled exact-input quote must converge");
        prop_assert!(doubled_out >= amount_out);
        prop_assert!(doubled_out <= amount_out.saturating_mul(2).saturating_add(10));

        let recovered_input = calc_in_given_out(
            &e,
            amplification,
            &balances,
            token_in,
            token_out,
            amount_out,
            invariant_before,
        )
        .expect("inverse exact-output quote must converge");
        prop_assert!(
            recovered_input.abs_diff(amount_in) <= amount_in / 10_000 + 2_000
        );
    }
}
