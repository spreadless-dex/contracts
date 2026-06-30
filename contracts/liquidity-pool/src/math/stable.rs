use soroban_sdk::{Env, U256};

use super::arithmetic::{
    checked_sub, div_up, mul_div_down, mul_div_up, mul_div_up_u64, to_u64, u256,
};
use super::fixed_math::{self, FixedComplement, FixedDiv, FixedMul};
use super::{AMP_PRECISION, BALANCE_THRESHOLD, DEFAULT_INV_THRESHOLD, MAX_TOKENS};

// StableMath._calculateInvariant
// Computes the invariant given the current balances, using the Newton-Raphson approximation.
// The amplification parameter equals: A n^(n-1)
// See: https://github.com/stabbleorg/balancer-v2-monorepo/blob/master/pkg/pool-stable/contracts/StableMath.sol#L57-L120
pub fn calc_invariant(
    e: &Env,
    amplification: u64,
    balances: &[u64],
    inv_threshold: Option<u64>,
) -> Option<u64> {
    // invariant                                                                                 //
    // D = invariant                                                  D^(n+1)                    //
    // A = amplification coefficient      A  n^n S + D = A D n^n + -----------                   //
    // S = sum of balances                                             n^n P                     //
    // P = product of balances                                                                   //
    // n = number of tokens                                                                      //

    // Always round down, to match Vyper's arithmetic (which always truncates).
    let sum: u64 = balances.iter().sum(); // S in the Curve version

    if sum == 0 {
        return Some(0);
    }

    let num_tokens = balances.len();
    if num_tokens > MAX_TOKENS {
        return None;
    }
    let num_tokens_u64 = num_tokens as u64;

    let zero = U256::from_u32(e, 0);
    let amp_precision = u256(e, AMP_PRECISION);
    let num_tokens_u256 = u256(e, num_tokens_u64);

    let amp_times_total = amplification.checked_mul(num_tokens_u64)?; // Ann in the Curve version
    let amp_times_total_u256 = u256(e, amp_times_total);

    let sum_u256 = u256(e, sum);
    let mut invariant = sum_u256.clone(); // D in the Curve version

    // Precompute balances[i] * num_tokens
    let mut balances_times = [0u64; MAX_TOKENS];
    for (i, &balance) in balances.iter().enumerate() {
        balances_times[i] = balance.checked_mul(num_tokens_u64)?;
    }

    let threshold = inv_threshold.unwrap_or(DEFAULT_INV_THRESHOLD);
    for _ in 0..64 {
        let mut p = invariant.clone();

        for &balance_times in balances_times[..num_tokens].iter() {
            // (p * invariant) / (balances[i] * num_tokens)
            let balance_times = u256(e, balance_times);
            p = mul_div_down(&p, &invariant, &balance_times, &zero)?;
        }

        let prev_invariant = invariant.clone(); // Dprev in the Curve version

        // numerator = (amp_times_total * sum / amp_precision) + p * n
        let numerator = mul_div_down(&amp_times_total_u256, &sum_u256, &amp_precision, &zero)?
            .add(&p.mul(&num_tokens_u256));

        // denominator = ((amp_times_total - amp_precision) * invariant / amp_precision) + (n + 1) * p
        let amp_minus = u256(e, amp_times_total.checked_sub(AMP_PRECISION)?);
        let denominator = mul_div_down(&amp_minus, &invariant, &amp_precision, &zero)?
            .add(&u256(e, num_tokens_u64.saturating_add(1)).mul(&p));

        invariant = mul_div_down(&numerator, &invariant, &denominator, &zero)?;

        let invariant_u64 = to_u64(&invariant)?;
        let prev_invariant_u64 = to_u64(&prev_invariant)?;

        if invariant_u64 > prev_invariant_u64 {
            if invariant_u64.saturating_sub(prev_invariant_u64) <= threshold {
                return Some(invariant_u64);
            }
        } else if prev_invariant_u64.saturating_sub(invariant_u64) <= threshold {
            return Some(invariant_u64);
        }
    }

    None
}

// Computes how many tokens can be taken out of a pool if `token_amount_in` are sent, given the current balances.
// The amplification parameter equals: A n^(n-1)
// See: https://github.com/stabbleorg/balancer-v2-monorepo/blob/master/pkg/pool-stable/contracts/StableMath.sol#L124-L159
pub fn calc_out_given_in(
    e: &Env,
    amplification: u64,
    balances: &[u64],
    token_index_in: usize,
    token_index_out: usize,
    token_amount_in: u64,
    invariant: u64,
) -> Option<u64> {
    /**************************************************************************************************************
    // outGivenIn token x for y - polynomial equation to solve                                                   //
    // ay = amount out to calculate                                                                              //
    // by = balance token out                                                                                    //
    // y = by - ay (finalBalanceOut)                                                                             //
    // D = invariant                                               D                     D^(n+1)                 //
    // A = amplification coefficient               y^2 + ( S + ----------  - D) * y -  ------------- = 0         //
    // n = number of tokens                                    (A * n^n)               A * n^2n * P              //
    // S = sum of final balances but y                                                                           //
    // P = product of final balances but y                                                                       //
     **************************************************************************************************************/
    // Amount out, so we round down overall.

    let num_tokens = balances.len();
    if num_tokens > MAX_TOKENS {
        return None;
    }

    let mut new_balances = [0u64; MAX_TOKENS];
    for i in 0..num_tokens {
        if i == token_index_in {
            new_balances[i] = balances[i].checked_add(token_amount_in)?;
        } else {
            new_balances[i] = balances[i];
        }
    }
    let new_balances = &new_balances[..num_tokens];

    let balance_out = *balances.get(token_index_out)?;

    let final_balance_out = get_token_balance_given_invariant_n_all_other_balances(
        e,
        amplification,
        new_balances,
        invariant,
        balance_out,
    )?;

    balance_out.checked_sub(final_balance_out)?.checked_sub(1)
}

// Computes how many tokens must be sent to a pool if `token_amount_out` are sent given the
// current balances, using the Newton-Raphson approximation.
// The amplification parameter equals: A n^(n-1)
// See: https://github.com/stabbleorg/balancer-v2-monorepo/blob/master/pkg/pool-stable/contracts/StableMath.sol#L164-L199
pub fn calc_in_given_out(
    e: &Env,
    amplification: u64,
    balances: &[u64],
    token_index_in: usize,
    token_index_out: usize,
    token_amount_out: u64,
    invariant: u64,
) -> Option<u64> {
    /**************************************************************************************************************
    // inGivenOut token x for y - polynomial equation to solve                                                   //
    // ax = amount in to calculate                                                                               //
    // bx = balance token in                                                                                     //
    // x = bx + ax (finalBalanceIn)                                                                              //
    // D = invariant                                                D                     D^(n+1)                //
    // A = amplification coefficient               x^2 + ( S + ----------  - D) * x -  ------------- = 0         //
    // n = number of tokens                                     (A * n^n)               A * n^2n * P             //
    // S = sum of final balances but x                                                                           //
    // P = product of final balances but x                                                                       //
     **************************************************************************************************************/
    // Amount in, so we round up overall.
    let num_tokens = balances.len();
    if num_tokens > MAX_TOKENS {
        return None;
    }

    let mut new_balances = [0u64; MAX_TOKENS];
    for i in 0..num_tokens {
        if i == token_index_out {
            new_balances[i] = balances[i].checked_sub(token_amount_out)?;
        } else {
            new_balances[i] = balances[i];
        }
    }
    let new_balances = &new_balances[..num_tokens];

    let balance_in = *balances.get(token_index_in)?;

    let final_balance_in = get_token_balance_given_invariant_n_all_other_balances(
        e,
        amplification,
        new_balances,
        invariant,
        balance_in,
    )?;

    final_balance_in.checked_sub(balance_in)?.checked_add(1)
}

// See: https://github.com/stabbleorg/balancer-v2-monorepo/blob/master/pkg/pool-stable/contracts/StableMath.sol#L201-L255
#[allow(clippy::too_many_arguments)]
pub fn calc_pool_token_out_given_exact_tokens_in(
    e: &Env,
    amplification: u64,
    balances: &[u64],
    amounts_in: &[u64],
    pool_token_supply: u64,
    current_invariant: u64,
    swap_fee: u64,
    inverse_threshold: Option<u64>,
) -> Option<u64> {
    // LP out, so we round down overall.

    let num_tokens = balances.len();
    if num_tokens > MAX_TOKENS {
        return None;
    }

    // First loop calculates the sum of all token balances, which will be used to calculate
    // the current weights of each token, relative to this sum
    let sum: u64 = balances.iter().sum();

    // Calculate the weighted balance ratio without considering fees
    let mut balance_ratios_with_fee = [0u64; MAX_TOKENS];
    // The weighted sum of token balance ratios with fee
    let mut invariant_ratio_with_fees = 0;
    for i in 0..num_tokens {
        let current_weight = balances[i].div_down(sum)?;
        balance_ratios_with_fee[i] = balances[i]
            .checked_add(amounts_in[i])?
            .div_down(balances[i])?;
        invariant_ratio_with_fees = balance_ratios_with_fee[i]
            .mul_down(current_weight)?
            .checked_add(invariant_ratio_with_fees)?;
    }

    // Second loop calculates new amounts in, taking into account the fee on the percentage excess
    let mut new_balances = [0u64; MAX_TOKENS];
    for i in 0..num_tokens {
        // Check if the balance ratio is greater than the ideal ratio to charge fees or not
        let amount_in_without_fee = if balance_ratios_with_fee[i] > invariant_ratio_with_fees {
            let non_taxable_amount =
                balances[i].mul_down(invariant_ratio_with_fees.checked_sub(fixed_math::ONE)?)?;
            let taxable_amount = amounts_in[i].checked_sub(non_taxable_amount)?;

            taxable_amount
                .mul_down(swap_fee.complement())?
                .checked_add(non_taxable_amount)?
        } else {
            amounts_in[i]
        };

        new_balances[i] = balances[i].checked_add(amount_in_without_fee)?;
    }
    let new_balances = &new_balances[..num_tokens];

    let new_invariant = calc_invariant(e, amplification, new_balances, inverse_threshold)?;
    let invariant_ratio = new_invariant.div_down(current_invariant)?;

    // If the invariant didn't increase for any reason, we simply don't mint LP
    if invariant_ratio > fixed_math::ONE {
        pool_token_supply.mul_down(invariant_ratio.saturating_sub(fixed_math::ONE))
    } else {
        Some(0)
    }
}

// See: https://github.com/stabbleorg/balancer-v2-monorepo/blob/master/pkg/pool-stable/contracts/StableMath.sol#L354-L395
#[allow(clippy::too_many_arguments)]
pub fn calc_token_out_given_exact_pool_token_in(
    e: &Env,
    amplification: u64,
    balances: &[u64],
    token_index: usize,
    amount_in: u64,
    pool_token_supply: u64,
    current_invariant: u64,
    swap_fee: u64,
) -> Option<u64> {
    // Token out, so we round down overall.

    let new_invariant = mul_div_up_u64(
        pool_token_supply.checked_sub(amount_in)?,
        current_invariant,
        pool_token_supply,
    )?;

    let balance = *balances.get(token_index)?;

    // Calculate amount out without fee
    let new_balance = get_token_balance_given_invariant_n_all_other_balances(
        e,
        amplification,
        balances,
        new_invariant,
        balance,
    )?;
    let amount_out_without_fee = balance.checked_sub(new_balance)?;

    // First calculate the sum of all token balances, which will be used to calculate
    // the current weight of each token
    let sum: u64 = balances.iter().sum();

    // We can now compute how much excess balance is being withdrawn as a result of the virtual swaps, which result
    // in swap fees.
    let current_weight = balance.div_down(sum)?;
    let taxable_percentage = current_weight.complement();

    // Swap fees are typically charged on 'token in', but there is no 'token in' here, so we apply it
    // to 'token out'. This results in slightly larger price impact. Fees are rounded up.
    let taxable_amount = amount_out_without_fee.mul_up(taxable_percentage)?;
    let non_taxable_amount = amount_out_without_fee.saturating_sub(taxable_amount);

    taxable_amount
        .mul_down(swap_fee.complement())?
        .checked_add(non_taxable_amount)
}

// This function calculates the balance of a given token (token_index)
// given all the other balances and the invariant
// See: https://github.com/stabbleorg/balancer-v2-monorepo/blob/master/pkg/pool-stable/contracts/StableMath.sol#L399-L449
fn get_token_balance_given_invariant_n_all_other_balances(
    e: &Env,
    amplification: u64,
    balances: &[u64],
    invariant: u64,
    balance: u64, // balance of a given token (token_index)
) -> Option<u64> {
    // Rounds result up overall

    let num_tokens = balances.len() as u64;

    let zero = U256::from_u32(e, 0);
    let one = U256::from_u32(e, 1);
    let amp_precision = u256(e, AMP_PRECISION);

    let amp_times_total = u256(e, amplification.checked_mul(num_tokens)?);

    let invariant = u256(e, invariant);

    let mut sum = balances[0];
    let mut p = u256(e, balances[0].checked_mul(num_tokens)?);
    for &balance in balances.iter().skip(1) {
        let p_i = u256(e, balance.checked_mul(num_tokens)?);
        p = mul_div_down(&p, &p_i, &invariant, &zero)?;
        sum = sum.checked_add(balance)?;
    }

    // No need to use safe math, based on the loop above `sum` is greater than or equal to `balances[token_index]`
    sum = sum.saturating_sub(balance);
    let sum = u256(e, sum);

    let invariant_2 = invariant.mul(&invariant);
    // We remove the balance from c by multiplying it
    let c = mul_div_up(
        &invariant_2,
        &amp_precision,
        &amp_times_total.mul(&p),
        &zero,
        &one,
    )?
    .mul(&u256(e, balance));
    let b = mul_div_down(&invariant, &amp_precision, &amp_times_total, &zero)?.add(&sum);

    // We iterate to find the balance
    // We multiply the first iteration outside the loop with the invariant to set the value of the
    // initial approximation.
    let mut token_balance = div_up(&invariant_2.add(&c), &invariant.add(&b), &zero, &one)?;

    for _ in 0..64 {
        let prev_token_balance = token_balance.clone();

        // token_balance * 2 + b - invariant
        let denominator = checked_sub(&token_balance.shl(1).add(&b), &invariant)?;
        token_balance = div_up(
            &token_balance.mul(&token_balance).add(&c),
            &denominator,
            &zero,
            &one,
        )?;

        let token_balance_u64 = to_u64(&token_balance)?;
        let prev_token_balance_u64 = to_u64(&prev_token_balance)?;

        if token_balance_u64 > prev_token_balance_u64 {
            if token_balance_u64.saturating_sub(prev_token_balance_u64) <= BALANCE_THRESHOLD {
                return Some(token_balance_u64);
            }
        } else if prev_token_balance_u64.saturating_sub(token_balance_u64) <= BALANCE_THRESHOLD {
            return Some(token_balance_u64);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::Env;

    #[test]
    fn test_calc_out_given_in() {
        let e = Env::default();

        let balances = [
            776199829833940141u64,
            2206504616663253113,
            1763368950384576155,
            38416709841306561,
            18833762826780,
        ];
        let amplification = 500_000;
        calc_invariant(&e, amplification, &balances, None).unwrap();

        let balances = [1332693902458055177u64, 534042038714371533, 93673549035235];
        let amplification = 10_000;
        calc_invariant(&e, amplification, &balances, None).unwrap();

        let balances = [2397586296768312160u64, 2300831385038136337, 1410688950371];
        let amplification = 1_000;
        calc_invariant(&e, amplification, &balances, None).unwrap();

        let amplification = 5_000_000;
        let balances = [40_000_000_000_000_000u64, 60_000_000_000_000_000];
        let invariant = calc_invariant(&e, amplification, &balances, None).unwrap();
        assert_eq!(invariant, 99999583421855646);

        let token_amount_in = 100_000_000_000_000;
        let token_a_out = calc_out_given_in(
            &e,
            amplification,
            &balances,
            1,
            0,
            token_amount_in,
            invariant,
        )
        .unwrap();
        let token_b_out = calc_out_given_in(
            &e,
            amplification,
            &balances,
            0,
            1,
            token_amount_in,
            invariant,
        )
        .unwrap();
        assert_eq!(token_a_out, 99991271119067);
        assert_eq!(token_b_out, 100008628389994);

        let amplification = 750_000;
        let balances = [
            40_000_000_000_000_000u64,
            50_000_000_000_000_000,
            60_000_000_000_000_000,
        ];
        let invariant = calc_invariant(&e, amplification, &balances, None).unwrap();
        assert_eq!(invariant, 149997226126050479);

        let amplification = 150_000;
        let balances = [
            40_000_000_000_000_000u64,
            50_000_000_000_000_000,
            60_000_000_000_000_000,
            70_000_000_000_000_000,
        ];
        let invariant = calc_invariant(&e, amplification, &balances, None).unwrap();
        assert_eq!(invariant, 219967475585041316);

        let amplification = 5_000_000;
        let balances = [894_520_800_000_000u64, 467_581_800_000_000];
        let invariant = calc_invariant(&e, amplification, &balances, None).unwrap();

        let token_amount_in = 1_000_000_000_000;
        let token_amount_out = calc_out_given_in(
            &e,
            amplification,
            &balances,
            0,
            1,
            token_amount_in,
            invariant,
        )
        .unwrap();
        assert_eq!(token_amount_out, 999845351779);

        let token_amount_in = 1_000_000_000;
        let token_amount_out = calc_out_given_in(
            &e,
            amplification,
            &balances,
            0,
            1,
            token_amount_in,
            invariant,
        )
        .unwrap();
        assert_eq!(token_amount_out, 999845869);

        let token_amount_in = 1_000_000;
        let token_amount_out = calc_out_given_in(
            &e,
            amplification,
            &balances,
            0,
            1,
            token_amount_in,
            invariant,
        )
        .unwrap();
        assert_eq!(token_amount_out, 999845);
    }

    #[test]
    fn test_calc_pool_token_out_given_exact_tokens_in() {
        let e = Env::default();

        let amplification = 5_000_000;
        let balances = [894_520_800_000_000u64, 467_581_800_000_000];
        let invariant = calc_invariant(&e, amplification, &balances, None).unwrap();

        let amounts_in = [1_000_000_000_000_000u64, 1_000_000_000_000_000];
        let amount_out = calc_pool_token_out_given_exact_tokens_in(
            &e,
            amplification,
            &balances,
            &amounts_in,
            invariant,
            invariant,
            100_000,
            None,
        )
        .unwrap();
        assert_eq!(amount_out, 1999977982041509);

        let amounts_in = [0u64, 2_000_000_000_000];
        let amount_out = calc_pool_token_out_given_exact_tokens_in(
            &e,
            amplification,
            &balances,
            &amounts_in,
            invariant,
            invariant,
            100_000,
            None,
        )
        .unwrap();
        assert_eq!(amount_out, 2000047447155);

        let amounts_in = [1_000_000_000_000u64, 1_000_000_000_000];
        let amount_out = calc_pool_token_out_given_exact_tokens_in(
            &e,
            amplification,
            &balances,
            &amounts_in,
            invariant,
            invariant,
            100_000,
            None,
        )
        .unwrap();
        assert!(amount_out < 2000047447155);
        assert_eq!(amount_out, 1999994325732);

        let amounts_in = [2_000_000_000_000u64, 0];
        let amount_out = calc_pool_token_out_given_exact_tokens_in(
            &e,
            amplification,
            &balances,
            &amounts_in,
            invariant,
            invariant,
            100_000,
            None,
        )
        .unwrap();
        assert!(amount_out < 1999994325732);
        assert_eq!(amount_out, 1999802271357);
        let amount_out = calc_pool_token_out_given_exact_tokens_in(
            &e,
            amplification,
            &balances,
            &amounts_in,
            invariant,
            invariant,
            150_000,
            None,
        )
        .unwrap();
        assert!(amount_out < 1999802271357);
        let amount_out = calc_pool_token_out_given_exact_tokens_in(
            &e,
            amplification,
            &balances,
            &amounts_in,
            invariant,
            invariant,
            50_000,
            None,
        )
        .unwrap();
        assert!(amount_out > 1999802271357);

        // balanced deposit
        let amounts_in = [1_313_441_146_063u64, 686_558_853_937];
        let amount_out = calc_pool_token_out_given_exact_tokens_in(
            &e,
            amplification,
            &balances,
            &amounts_in,
            invariant,
            invariant,
            100_000,
            None,
        )
        .unwrap();
        assert_eq!(amount_out, 1999977980679);
        let amount_out = calc_pool_token_out_given_exact_tokens_in(
            &e,
            amplification,
            &balances,
            &amounts_in,
            invariant,
            invariant,
            150_000,
            None,
        )
        .unwrap();
        assert_eq!(amount_out, 1999977980679);
        let amount_out = calc_pool_token_out_given_exact_tokens_in(
            &e,
            amplification,
            &balances,
            &amounts_in,
            invariant,
            invariant,
            50_000,
            None,
        )
        .unwrap();
        assert_eq!(amount_out, 1999977980679);
        let amount_out = calc_pool_token_out_given_exact_tokens_in(
            &e,
            amplification,
            &balances,
            &amounts_in,
            invariant,
            invariant,
            300_000,
            None,
        )
        .unwrap();
        assert_eq!(amount_out, 1999977980679);
    }
}
