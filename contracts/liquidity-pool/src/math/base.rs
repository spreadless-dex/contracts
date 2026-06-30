// Base pool math, ported from Stabble (a Balancer-V2 fork): `base_pool_math.rs`.
// Proportional join/exit amounts shared by every pool type, independent of the
// stable-swap invariant in `stable.rs`.
//
// Not wired into the contract yet — `contract.rs::withdraw` computes the
// proportional slice inline with `mul_div_down_u64`. These are kept for upstream
// numerical parity (and their reference test below), matching the rest of the
// not-yet-wired `math` surface.

use super::arithmetic::{mul_div_down_u64, mul_div_up_u64};
use super::MAX_TOKENS;

// Proportional amounts in to provide for an exact pool-token (LP) amount out.
// Rounds up, so the joiner always backs the minted shares in full (favours the pool).
// See: stabbleorg/amm-sdk base_pool_math.rs (Balancer BasePoolMath._computeProportionalAmountsIn)
pub fn compute_proportional_amounts_in(
    balances: &[u64],
    pool_token_supply: u64,
    pool_token_amount: u64,
) -> Option<[u64; MAX_TOKENS]> {
    // amounts_in[i] = balances[i] * pool_token_amount / pool_token_supply, rounded up
    if balances.len() > MAX_TOKENS {
        return None;
    }
    let mut amounts_in = [0u64; MAX_TOKENS];
    for (i, &balance) in balances.iter().enumerate() {
        amounts_in[i] = mul_div_up_u64(balance, pool_token_amount, pool_token_supply)?;
    }
    Some(amounts_in)
}

// Proportional amounts out to return for an exact pool-token (LP) amount in.
// Rounds down, so the pool never pays out more than the burned shares back (favours the pool).
// See: stabbleorg/amm-sdk base_pool_math.rs (Balancer BasePoolMath._computeProportionalAmountsOut)
pub fn compute_proportional_amounts_out(
    balances: &[u64],
    pool_token_supply: u64,
    pool_token_amount: u64,
) -> Option<[u64; MAX_TOKENS]> {
    // amounts_out[i] = balances[i] * pool_token_amount / pool_token_supply, rounded down
    if balances.len() > MAX_TOKENS {
        return None;
    }
    let mut amounts_out = [0u64; MAX_TOKENS];
    for (i, &balance) in balances.iter().enumerate() {
        amounts_out[i] = mul_div_down_u64(balance, pool_token_amount, pool_token_supply)?;
    }
    Some(amounts_out)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Reference-parity test ported from stabbleorg/amm-sdk (libraries/math,
    // base_pool_math). Same inputs and exact expected outputs as upstream. The
    // balances are multiples of the supply, so every division is exact and the
    // round-up (`in`) and round-down (`out`) paths agree.
    #[test]
    fn test_compute_proportional_amounts() {
        let balances = [5_000_000_000u64, 3_000_000_000];
        let pool_token_supply = 1_000_000_000;

        let amounts_in =
            compute_proportional_amounts_in(&balances, pool_token_supply, 100_000_000).unwrap();
        assert_eq!(amounts_in[0], 500000000);
        assert_eq!(amounts_in[1], 300000000);

        let amounts_out =
            compute_proportional_amounts_out(&balances, pool_token_supply, 100_000_000).unwrap();
        assert_eq!(amounts_out[0], 500000000);
        assert_eq!(amounts_out[1], 300000000);

        let amounts_in =
            compute_proportional_amounts_in(&balances, pool_token_supply, 333_333_333).unwrap();
        assert_eq!(amounts_in[0], 1666666665);
        assert_eq!(amounts_in[1], 999999999);

        let amounts_out =
            compute_proportional_amounts_out(&balances, pool_token_supply, 333_333_333).unwrap();
        assert_eq!(amounts_out[0], 1666666665);
        assert_eq!(amounts_out[1], 999999999);

        let amounts_in =
            compute_proportional_amounts_in(&balances, pool_token_supply, 777_777_777).unwrap();
        assert_eq!(amounts_in[0], 3888888885);
        assert_eq!(amounts_in[1], 2333333331);

        let amounts_out =
            compute_proportional_amounts_out(&balances, pool_token_supply, 777_777_777).unwrap();
        assert_eq!(amounts_out[0], 3888888885);
        assert_eq!(amounts_out[1], 2333333331);
    }
}
