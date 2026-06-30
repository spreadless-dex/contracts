// LP exit math. Proportional exits are invariant-independent; single-token
// exits use the stable invariant and charge swap-style fees on the virtual
// imbalance. This is the contract-facing seam for burning LP shares.

use soroban_sdk::Env;

use super::arithmetic::mul_div_down_u64;
use super::stable;
use super::MAX_TOKENS;

// Proportional amounts out to return for an exact LP amount in. Rounds down,
// so the pool never pays out more than the burned shares back.
// See: stabbleorg/amm-sdk base_pool_math.rs (Balancer BasePoolMath._computeProportionalAmountsOut)
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

#[allow(clippy::too_many_arguments)]
pub(crate) fn single_token_amount_out(
    e: &Env,
    amplification: u64,
    balances: &[u64],
    token_index: usize,
    pool_token_amount: u64,
    pool_token_supply: u64,
    swap_fee: u64,
) -> Option<u64> {
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

#[cfg(test)]
mod tests {
    use super::*;

    // Reference-parity test ported from stabbleorg/amm-sdk (libraries/math,
    // base_pool_math). Same inputs and exact expected outputs as upstream.
    #[test]
    fn test_proportional_amounts_out() {
        let balances = [5_000_000_000u64, 3_000_000_000];
        let pool_token_supply = 1_000_000_000;

        let amounts_out =
            proportional_amounts_out(&balances, pool_token_supply, 100_000_000).unwrap();
        assert_eq!(amounts_out[0], 500000000);
        assert_eq!(amounts_out[1], 300000000);

        let amounts_out =
            proportional_amounts_out(&balances, pool_token_supply, 333_333_333).unwrap();
        assert_eq!(amounts_out[0], 1666666665);
        assert_eq!(amounts_out[1], 999999999);

        let amounts_out =
            proportional_amounts_out(&balances, pool_token_supply, 777_777_777).unwrap();
        assert_eq!(amounts_out[0], 3888888885);
        assert_eq!(amounts_out[1], 2333333331);
    }
}
