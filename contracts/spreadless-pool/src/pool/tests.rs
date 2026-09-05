use soroban_sdk::{testutils::Address as _, Address, Env, Vec};

use super::amp::{is_valid_amp_factor, ramp_amp};
use super::fee::{
    is_valid_protocol_fee, is_valid_swap_fee, output_from_gross, output_from_net, protocol_share,
};
use super::normalized::NormalizedAmounts;
use super::quote::{
    deposit_exact_tokens_in, swap_exact_in, swap_exact_out, withdraw_one_token,
    withdraw_proportional,
};
use super::scaling::{from_internal, from_internal_up, scaling_for, to_internal};
use super::state::{Pool, PoolToken};
use super::AmpControl;
use crate::math::{AMP_PRECISION, MAX_SAFE_BALANCE};

const ONE_MILLION_TOKENS: u64 = 1_000_000_000_000_000;

fn pool_with(e: &Env, reserve_0: u64, reserve_1: u64, swap_fee: u64, protocol_fee: u64) -> Pool {
    let mut tokens = Vec::new(e);
    tokens.push_back(PoolToken {
        token: Address::generate(e),
        decimals: 9,
        scaling_factor: 1,
        scaling_up: true,
        reserve: reserve_0,
        max_cap: MAX_SAFE_BALANCE,
    });
    tokens.push_back(PoolToken {
        token: Address::generate(e),
        decimals: 9,
        scaling_factor: 1,
        scaling_up: true,
        reserve: reserve_1,
        max_cap: MAX_SAFE_BALANCE,
    });
    Pool {
        tokens,
        protocol_controller: Address::generate(e),
        amp_control: AmpControl::ProtocolManaged,
        amp_initial_factor: 100,
        amp_target_factor: 100,
        ramp_start_ts: 0,
        ramp_stop_ts: 0,
        swap_fee,
        protocol_fee,
        beneficiary: Address::generate(e),
    }
}

fn normalized(values: &[u64]) -> NormalizedAmounts {
    let mut fixed = [0_u64; 5];
    fixed[..values.len()].copy_from_slice(values);
    NormalizedAmounts::new(fixed, values.len()).unwrap()
}

#[test]
fn scaling_derives_factors_for_common_token_decimals() {
    assert_eq!(scaling_for(6), Some((1_000, true)));
    assert_eq!(scaling_for(7), Some((100, true)));
    assert_eq!(scaling_for(9), Some((1, true)));
    assert_eq!(scaling_for(18), Some((1_000_000_000, false)));
    assert_eq!(scaling_for(29), None);
}

#[test]
fn scaling_roundtrips_and_uses_directional_rounding() {
    let (factor, scaling_up) = scaling_for(7).unwrap();
    let raw = 12_345_678_i128;
    let internal = to_internal(raw, factor, scaling_up).unwrap();
    assert_eq!(internal, 1_234_567_800);
    assert_eq!(from_internal(internal, factor, scaling_up), raw);
    assert_eq!(from_internal(101, factor, scaling_up), 1);
    assert_eq!(from_internal_up(101, factor, scaling_up), 2);

    let (factor, scaling_up) = scaling_for(18).unwrap();
    let raw = 1_234_567_000_000_000_000_i128;
    assert_eq!(to_internal(raw, factor, scaling_up), Some(1_234_567_000));
    assert_eq!(
        to_internal(raw + factor as i128 - 1, factor, scaling_up),
        Some(1_234_567_000)
    );
}

#[test]
fn scaling_rejects_negative_overflowing_and_unsafe_balances() {
    let (factor, scaling_up) = scaling_for(9).unwrap();
    assert_eq!(to_internal(-1, factor, scaling_up), None);
    assert_eq!(
        to_internal(MAX_SAFE_BALANCE as i128, factor, scaling_up),
        Some(MAX_SAFE_BALANCE)
    );
    assert_eq!(
        to_internal(MAX_SAFE_BALANCE as i128 + 1, factor, scaling_up),
        None
    );

    let (factor, scaling_up) = scaling_for(6).unwrap();
    assert_eq!(to_internal(i128::MAX, factor, scaling_up), None);
}

#[test]
fn amp_ramp_clamps_and_interpolates_in_both_directions() {
    assert_eq!(ramp_amp(5_000, 5_000, 0, 0, 100), 5_000 * AMP_PRECISION);
    assert_eq!(ramp_amp(1_000, 5_000, 100, 100, 50), 5_000 * AMP_PRECISION);
    assert_eq!(ramp_amp(1_000, 5_000, 100, 200, 50), 1_000 * AMP_PRECISION);
    assert_eq!(ramp_amp(1_000, 5_000, 100, 200, 150), 3_000 * AMP_PRECISION);
    assert_eq!(ramp_amp(5_000, 1_000, 100, 200, 150), 3_000 * AMP_PRECISION);
    assert_eq!(ramp_amp(1_000, 5_000, 100, 200, 999), 5_000 * AMP_PRECISION);
}

#[test]
fn amp_factor_validation_includes_both_documented_boundaries() {
    assert!(!is_valid_amp_factor(0));
    assert!(is_valid_amp_factor(1));
    assert!(is_valid_amp_factor(50_000));
    assert!(!is_valid_amp_factor(50_001));
}

#[test]
fn fee_validation_includes_both_documented_boundaries() {
    assert!(!is_valid_swap_fee(9_999));
    assert!(is_valid_swap_fee(10_000));
    assert!(is_valid_swap_fee(10_000_000));
    assert!(!is_valid_swap_fee(10_000_001));
    assert!(is_valid_protocol_fee(0));
    assert!(is_valid_protocol_fee(1_000_000_000));
    assert!(!is_valid_protocol_fee(1_000_000_001));
}

#[test]
fn output_fee_quotes_split_the_fee_without_changing_user_output() {
    let from_gross = output_from_gross(10_000_000, 500_000_000, 1_000_000_000).unwrap();
    assert_eq!(from_gross.gross_out, 1_000_000_000);
    assert_eq!(from_gross.net_out, 990_000_000);
    assert_eq!(from_gross.protocol, 5_000_000);

    let from_net = output_from_net(10_000_000, 500_000_000, 990_000_000).unwrap();
    assert_eq!(from_net.gross_out, from_gross.gross_out);
    assert_eq!(from_net.net_out, from_gross.net_out);
    assert_eq!(from_net.protocol, from_gross.protocol);
    assert_eq!(protocol_share(3, 500_000_000), Some(1));
}

#[test]
fn first_deposit_quote_mints_the_invariant_without_protocol_lp() {
    let e = Env::default();
    let pool = pool_with(&e, 0, 0, 3_000_000, 500_000_000);
    let deposit = normalized(&[ONE_MILLION_TOKENS, ONE_MILLION_TOKENS]);

    let quote = deposit_exact_tokens_in(&e, &pool, 0, &deposit, 0).unwrap();

    assert_eq!(quote.lp_out, 2 * ONE_MILLION_TOKENS);
    assert_eq!(quote.protocol_lp, 0);
}

#[test]
fn balanced_followup_deposit_has_no_protocol_lp_fee() {
    let e = Env::default();
    let pool = pool_with(
        &e,
        ONE_MILLION_TOKENS,
        ONE_MILLION_TOKENS,
        3_000_000,
        500_000_000,
    );
    let deposit = normalized(&[100_000_000_000_000, 100_000_000_000_000]);

    let quote = deposit_exact_tokens_in(&e, &pool, 0, &deposit, 2 * ONE_MILLION_TOKENS).unwrap();

    assert_eq!(quote.lp_out, 200_000_000_000_000);
    assert_eq!(quote.protocol_lp, 0);
}

#[test]
fn proportional_withdraw_quote_rounds_down() {
    let e = Env::default();
    let pool = pool_with(&e, 5_000_000_000, 3_000_000_000, 3_000_000, 0);

    let quote = withdraw_proportional(&pool, 1_000_000_000, 333_333_333).unwrap();

    assert_eq!(&quote.amounts_out[..2], &[1_666_666_665, 999_999_999]);
}

#[test]
fn exact_input_and_output_quotes_are_consistent_after_fees() {
    let e = Env::default();
    e.cost_estimate().budget().reset_unlimited();
    let pool = pool_with(
        &e,
        ONE_MILLION_TOKENS,
        ONE_MILLION_TOKENS,
        3_000_000,
        500_000_000,
    );
    let amount_in = ONE_MILLION_TOKENS / 100;

    let exact_in = swap_exact_in(&e, &pool, 0, 0, 1, amount_in).unwrap();
    let exact_out = swap_exact_out(&e, &pool, 0, 0, 1, exact_in.net_out).unwrap();

    assert!(exact_in.net_out < amount_in);
    assert!(exact_in.protocol > 0);
    assert_eq!(exact_out.protocol, exact_in.protocol);
    assert!(exact_out.amount_in.abs_diff(amount_in) <= 2);
}

#[test]
fn single_token_withdraw_quote_routes_only_the_protocol_fee_cut() {
    let e = Env::default();
    e.cost_estimate().budget().reset_unlimited();
    let pool = pool_with(
        &e,
        ONE_MILLION_TOKENS,
        ONE_MILLION_TOKENS,
        3_000_000,
        500_000_000,
    );

    let quote = withdraw_one_token(
        &e,
        &pool,
        0,
        0,
        ONE_MILLION_TOKENS / 2,
        2 * ONE_MILLION_TOKENS,
    )
    .unwrap();

    assert!(quote.amount_out > 0);
    assert!(quote.amount_out < ONE_MILLION_TOKENS / 2);
    assert!(quote.protocol > 0);
}
