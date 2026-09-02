// LP exit math. Proportional exits are invariant-independent; single-token
// exits use the stable invariant and charge swap-style fees on the virtual
// imbalance. This is the contract-facing seam for burning LP shares.

use soroban_sdk::Env;

use super::arithmetic::mul_div_down_u64;
use super::stable;
use super::MAX_TOKENS;

// Proportional amounts out to return for an exact LP amount in. Rounds down,
// so the pool never pays out more than the burned shares back.
pub(crate) fn proportional_amounts_out(
    balances: &[u64],
    pool_token_supply: u64,
    pool_token_amount: u64,
) -> Option<[u64; MAX_TOKENS]> {
    if balances.len() > MAX_TOKENS {
        return None;
    }
    let mut amounts_out = [0u64; MAX_TOKENS];
    for (i, &balance) in balances.iter().enumerate() {
        amounts_out[i] = mul_div_down_u64(balance, pool_token_amount, pool_token_supply)?;
    }
    Some(amounts_out)
}

// Returns `(net_out, fee)` — see `calc_token_out_given_exact_pool_token_in`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn single_token_amount_out(
    e: &Env,
    amplification: u64,
    balances: &[u64],
    token_index: usize,
    pool_token_amount: u64,
    pool_token_supply: u64,
    swap_fee: u64,
) -> Option<(u64, u64)> {
    let current_invariant = stable::calc_invariant(e, amplification, balances, None)?;
    stable::calc_token_out_given_exact_pool_token_in(
        e,
        amplification,
        balances,
        token_index,
        pool_token_amount,
        pool_token_supply,
        current_invariant,
        swap_fee,
    )
}
