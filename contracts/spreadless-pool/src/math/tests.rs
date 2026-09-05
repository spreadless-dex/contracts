use soroban_sdk::Env;

use super::arithmetic::{checked_sum, mul_div_down_u64, mul_div_up_u64};
use super::lp_exit::proportional_amounts_out;
use super::{
    calc_in_given_out, calc_invariant, calc_out_given_in,
    calc_pool_token_out_given_exact_tokens_in, AMP_PRECISION,
};

const ONE_MILLION_TOKENS: u64 = 1_000_000_000_000_000;

#[test]
fn integer_mul_div_has_explicit_pool_favouring_rounding() {
    assert_eq!(mul_div_down_u64(10, 2, 3), Some(6));
    assert_eq!(mul_div_up_u64(10, 2, 3), Some(7));
    assert_eq!(mul_div_down_u64(10, 0, 3), Some(0));
    assert_eq!(mul_div_up_u64(10, 0, 3), Some(0));
    assert_eq!(mul_div_down_u64(10, 2, 0), None);
    assert_eq!(mul_div_up_u64(10, 2, 0), None);
    assert_eq!(mul_div_down_u64(u64::MAX, u64::MAX, 1), None);
    assert_eq!(mul_div_up_u64(u64::MAX, u64::MAX, 1), None);
}

#[test]
fn checked_sum_fails_closed_on_overflow() {
    assert_eq!(checked_sum(&[1, 2, 3]), Some(6));
    assert_eq!(checked_sum(&[]), Some(0));
    assert_eq!(checked_sum(&[u64::MAX, 1]), None);
}

#[test]
fn proportional_exit_rounds_down_for_every_reserve() {
    let balances = [5_000_000_000_u64, 3_000_000_000];
    let supply = 1_000_000_000;

    let tenth = proportional_amounts_out(&balances, supply, 100_000_000).unwrap();
    assert_eq!(&tenth[..2], &[500_000_000, 300_000_000]);

    let third = proportional_amounts_out(&balances, supply, 333_333_333).unwrap();
    assert_eq!(&third[..2], &[1_666_666_665, 999_999_999]);

    assert_eq!(proportional_amounts_out(&balances, 0, 1), None);
}

#[test]
fn stable_math_matches_reference_vectors() {
    let e = Env::default();
    e.cost_estimate().budget().reset_unlimited();

    let balances = [40_000_000_000_000_000_u64, 60_000_000_000_000_000];
    let amplification = 5_000_000;
    let invariant = calc_invariant(&e, amplification, &balances, None).unwrap();
    assert_eq!(invariant, 99_999_583_421_855_646);
    assert_eq!(
        calc_out_given_in(
            &e,
            amplification,
            &balances,
            1,
            0,
            100_000_000_000_000,
            invariant,
        ),
        Some(99_991_271_119_067)
    );
    assert_eq!(
        calc_out_given_in(
            &e,
            amplification,
            &balances,
            0,
            1,
            100_000_000_000_000,
            invariant,
        ),
        Some(100_008_628_389_994)
    );

    let balances = [894_520_800_000_000_u64, 467_581_800_000_000];
    let invariant = calc_invariant(&e, amplification, &balances, None).unwrap();
    assert_eq!(
        calc_out_given_in(&e, amplification, &balances, 0, 1, 1_000_000_000, invariant,),
        Some(999_845_869)
    );
}

#[test]
fn deposit_math_matches_reference_vectors() {
    let e = Env::default();
    e.cost_estimate().budget().reset_unlimited();
    let amplification = 5_000_000;
    let balances = [894_520_800_000_000_u64, 467_581_800_000_000];
    let invariant = calc_invariant(&e, amplification, &balances, None).unwrap();

    assert_eq!(
        calc_pool_token_out_given_exact_tokens_in(
            &e,
            amplification,
            &balances,
            &[1_000_000_000_000_000, 1_000_000_000_000_000],
            invariant,
            invariant,
            100_000,
            None,
        ),
        Some(1_999_977_982_041_509)
    );
    assert_eq!(
        calc_pool_token_out_given_exact_tokens_in(
            &e,
            amplification,
            &balances,
            &[0, 2_000_000_000_000],
            invariant,
            invariant,
            100_000,
            None,
        ),
        Some(2_000_047_447_155)
    );
}

#[test]
fn swaps_preserve_the_invariant_across_pool_shapes() {
    let pools: &[&[u64]] = &[
        &[ONE_MILLION_TOKENS, ONE_MILLION_TOKENS],
        &[1_600_000_000_000_000, 400_000_000_000_000],
        &[1_980_000_000_000_000, 20_000_000_000_000],
        &[ONE_MILLION_TOKENS, ONE_MILLION_TOKENS, ONE_MILLION_TOKENS],
        &[
            ONE_MILLION_TOKENS,
            ONE_MILLION_TOKENS,
            ONE_MILLION_TOKENS,
            ONE_MILLION_TOKENS,
            ONE_MILLION_TOKENS,
        ],
    ];

    for balances in pools {
        // This exact 10_000-unit rounding bound was calibrated by the
        // historical vectors over this amplification ladder. The property
        // test separately covers the expanded range through 50_000.
        for amp_factor in [1_u64, 10, 100, 1_000, 12_000] {
            for size_bps in [1_u64, 100, 1_000, 3_000] {
                let e = Env::default();
                e.cost_estimate().budget().reset_unlimited();
                let i = 0;
                let j = balances.len() - 1;
                let amount_in = (balances[i] as u128 * size_bps as u128 / 10_000) as u64;
                let amplification = amp_factor * AMP_PRECISION;
                let before = calc_invariant(&e, amplification, balances, None).unwrap();
                let out = calc_out_given_in(&e, amplification, balances, i, j, amount_in, before)
                    .unwrap();
                assert!(out < balances[j]);

                let mut after = balances.to_vec();
                after[i] += amount_in;
                after[j] -= out;
                let after_invariant = calc_invariant(&e, amplification, &after, None).unwrap();
                assert!(after_invariant >= before);
                assert!(after_invariant - before <= 10_000);
            }
        }
    }
}

#[test]
fn higher_amplification_reduces_slippage_near_balance() {
    let balances = [ONE_MILLION_TOKENS, ONE_MILLION_TOKENS];
    let amount_in = ONE_MILLION_TOKENS / 100;
    let mut previous_out = {
        let reserve = ONE_MILLION_TOKENS as u128;
        (reserve * amount_in as u128 / (reserve + amount_in as u128)) as u64
    };

    for amp_factor in [1_u64, 10, 100, 1_000, 12_000, 50_000] {
        let e = Env::default();
        let amplification = amp_factor * AMP_PRECISION;
        let invariant = calc_invariant(&e, amplification, &balances, None).unwrap();
        let out =
            calc_out_given_in(&e, amplification, &balances, 0, 1, amount_in, invariant).unwrap();
        assert!(out > previous_out, "amp {amp_factor}");
        previous_out = out;
    }
}

#[test]
fn exact_in_and_exact_out_quotes_are_consistent() {
    let pools: &[&[u64]] = &[
        &[ONE_MILLION_TOKENS, ONE_MILLION_TOKENS],
        &[1_600_000_000_000_000, 400_000_000_000_000],
        &[1_980_000_000_000_000, 20_000_000_000_000],
    ];

    for balances in pools {
        for amp_factor in [1_u64, 100, 12_000, 50_000] {
            let e = Env::default();
            e.cost_estimate().budget().reset_unlimited();
            let amount_in = balances[0] / 100;
            let amplification = amp_factor * AMP_PRECISION;
            let invariant = calc_invariant(&e, amplification, balances, None).unwrap();
            let out =
                calc_out_given_in(&e, amplification, balances, 0, 1, amount_in, invariant).unwrap();
            let recovered =
                calc_in_given_out(&e, amplification, balances, 0, 1, out, invariant).unwrap();
            assert!(recovered.abs_diff(amount_in) <= amount_in / 10_000 + 2_000);
        }
    }
}

#[test]
fn out_of_domain_math_fails_closed() {
    let e = Env::default();
    let six = [ONE_MILLION_TOKENS; 6];
    assert_eq!(calc_invariant(&e, 100 * AMP_PRECISION, &six, None), None);
    assert_eq!(calc_invariant(&e, 100 * AMP_PRECISION, &[], None), Some(0));

    let balances = [ONE_MILLION_TOKENS, ONE_MILLION_TOKENS];
    let invariant = calc_invariant(&e, 100 * AMP_PRECISION, &balances, None).unwrap();
    assert_eq!(
        calc_in_given_out(
            &e,
            100 * AMP_PRECISION,
            &balances,
            0,
            1,
            balances[1] + 1,
            invariant,
        ),
        None
    );
}
