// Quotes for pool operations that depend on stable math. This module owns the
// ordering: current amp, normalized reserves, invariant calculation, formula
// selection, and swap-fee accounting.

use soroban_sdk::Env;

use crate::math;

use super::fee;
use super::state::Pool;
use super::{current_amp, reserves, NormalizedAmounts};

pub struct ProportionalWithdrawQuote {
    pub amounts_out: [u64; math::MAX_TOKENS],
}

pub struct SingleTokenWithdrawQuote {
    pub amount_out: u64,
}

pub struct SwapExactInQuote {
    pub net_out: u64,
    pub protocol: u64,
}

pub struct SwapExactOutQuote {
    pub amount_in: u64,
    pub protocol: u64,
}

pub fn deposit_exact_tokens_in(
    e: &Env,
    pool: &Pool,
    now: u64,
    amounts_in: &NormalizedAmounts,
    pool_token_supply: u64,
) -> Option<u64> {
    let amp = current_amp(pool, now);
    if amounts_in.len() != pool.tokens.len() as usize {
        return None;
    }

    if pool_token_supply == 0 {
        return math::calc_invariant(e, amp, amounts_in.as_slice(), None).filter(|d| *d > 0);
    }

    let reserves = reserves(pool);
    let current_invariant = math::calc_invariant(e, amp, reserves.as_slice(), None)?;
    math::calc_pool_token_out_given_exact_tokens_in(
        e,
        amp,
        reserves.as_slice(),
        amounts_in.as_slice(),
        pool_token_supply,
        current_invariant,
        pool.swap_fee,
        None,
    )
}

pub fn withdraw_proportional(
    pool: &Pool,
    pool_token_supply: u64,
    pool_token_amount: u64,
) -> Option<ProportionalWithdrawQuote> {
    let reserves = reserves(pool);
    let amounts_out =
        math::proportional_amounts_out(reserves.as_slice(), pool_token_supply, pool_token_amount)?;

    Some(ProportionalWithdrawQuote { amounts_out })
}

pub fn withdraw_one_token(
    e: &Env,
    pool: &Pool,
    now: u64,
    token_index: usize,
    pool_token_amount: u64,
    pool_token_supply: u64,
) -> Option<SingleTokenWithdrawQuote> {
    let amp = current_amp(pool, now);
    let reserves = reserves(pool);
    let amount_out = math::single_token_amount_out(
        e,
        amp,
        reserves.as_slice(),
        token_index,
        pool_token_amount,
        pool_token_supply,
        pool.swap_fee,
    )?;

    Some(SingleTokenWithdrawQuote { amount_out })
}

pub fn swap_exact_in(
    e: &Env,
    pool: &Pool,
    now: u64,
    token_index_in: usize,
    token_index_out: usize,
    amount_in: u64,
) -> Option<SwapExactInQuote> {
    let amp = current_amp(pool, now);
    let reserves = reserves(pool);
    let invariant = math::calc_invariant(e, amp, reserves.as_slice(), None)?;
    let out_without_fee = math::calc_out_given_in(
        e,
        amp,
        reserves.as_slice(),
        token_index_in,
        token_index_out,
        amount_in,
        invariant,
    )?;
    let output_fee = fee::output_from_gross(pool.swap_fee, pool.protocol_fee, out_without_fee)?;

    Some(SwapExactInQuote {
        net_out: output_fee.net_out,
        protocol: output_fee.protocol,
    })
}

pub fn swap_exact_out(
    e: &Env,
    pool: &Pool,
    now: u64,
    token_index_in: usize,
    token_index_out: usize,
    net_out: u64,
) -> Option<SwapExactOutQuote> {
    let output_fee = fee::output_from_net(pool.swap_fee, pool.protocol_fee, net_out)?;

    let amp = current_amp(pool, now);
    let reserves = reserves(pool);
    let invariant = math::calc_invariant(e, amp, reserves.as_slice(), None)?;
    let amount_in = math::calc_in_given_out(
        e,
        amp,
        reserves.as_slice(),
        token_index_in,
        token_index_out,
        output_fee.gross_out,
        invariant,
    )?;

    Some(SwapExactOutQuote {
        amount_in,
        protocol: output_fee.protocol,
    })
}
