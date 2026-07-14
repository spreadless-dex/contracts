use soroban_sdk::{Env, U256};

use super::arithmetic::{
    checked_sub, checked_sum, div_up, mul_div_down, mul_div_up, mul_div_up_u64, to_u64, u256,
};
use super::fixed_math::{self, FixedComplement, FixedDiv, FixedMul};
use super::{AMP_PRECISION, BALANCE_THRESHOLD, DEFAULT_INV_THRESHOLD, MAX_TOKENS};

// Computes the invariant given the current balances, using the Newton-Raphson approximation.
// The amplification parameter equals: A n^(n-1)
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
    let sum = checked_sum(balances)?; // S in the Curve version

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

    // Loop-invariant terms, hoisted out of the Newton iteration below so they
    // aren't re-allocated as host U256 objects on every pass:
    //   balances[i] * num_tokens (the per-token divisor), as U256
    //   amp_times_total - amp_precision        (the `invariant` coefficient)
    //   n + 1                                  (the `p` coefficient)
    //   amp_times_total * sum / amp_precision  (the constant numerator term)
    let mut balances_times = [0u64; MAX_TOKENS];
    for (i, &balance) in balances.iter().enumerate() {
        balances_times[i] = balance.checked_mul(num_tokens_u64)?;
    }
    let balances_times_u256: [U256; MAX_TOKENS] =
        core::array::from_fn(|i| u256(e, balances_times[i]));
    let amp_minus = u256(e, amp_times_total.checked_sub(AMP_PRECISION)?);
    let n_plus_1 = u256(e, num_tokens_u64.saturating_add(1));
    let numerator_const = mul_div_down(&amp_times_total_u256, &sum_u256, &amp_precision, &zero)?;

    let threshold = inv_threshold.unwrap_or(DEFAULT_INV_THRESHOLD);
    for _ in 0..64 {
        let mut p = invariant.clone();
        for balance_times in balances_times_u256[..num_tokens].iter() {
            // (p * invariant) / (balances[i] * num_tokens)
            p = mul_div_down(&p, &invariant, balance_times, &zero)?;
        }

        let prev_invariant = invariant.clone(); // Dprev in the Curve version

        // numerator   = amp_times_total * sum / amp_precision + p * n
        let numerator = numerator_const.add(&p.mul(&num_tokens_u256));
        // denominator = (amp_times_total - amp_precision) * invariant / amp_precision + (n + 1) * p
        let denominator =
            mul_div_down(&amp_minus, &invariant, &amp_precision, &zero)?.add(&n_plus_1.mul(&p));

        invariant = mul_div_down(&numerator, &invariant, &denominator, &zero)?;

        let invariant_u64 = to_u64(&invariant)?;
        let prev_invariant_u64 = to_u64(&prev_invariant)?;
        if invariant_u64.abs_diff(prev_invariant_u64) <= threshold {
            return Some(invariant_u64);
        }
    }

    None
}

// Copy `balances` into a fixed-capacity array with entry `index` set to `value`.
// Used to form the post-trade balances for a single token. The caller obtains
// `value` via `balances.get(index)?`, so `index < balances.len() <= MAX_TOKENS`.
fn balances_with(balances: &[u64], index: usize, value: u64) -> [u64; MAX_TOKENS] {
    let mut new_balances = [0u64; MAX_TOKENS];
    new_balances[..balances.len()].copy_from_slice(balances);
    new_balances[index] = value;
    new_balances
}

// LP shares to mint for a deposit that grew the invariant from `current` to
// `new`: `supply * (new / current - 1)`, rounded down. If the invariant didn't
// increase (for any reason) no LP is minted.
fn lp_from_invariant_ratio(
    pool_token_supply: u64,
    new_invariant: u64,
    current_invariant: u64,
) -> Option<u64> {
    let invariant_ratio = new_invariant.div_down(current_invariant)?;
    if invariant_ratio > fixed_math::ONE {
        pool_token_supply.mul_down(invariant_ratio.saturating_sub(fixed_math::ONE))
    } else {
        Some(0)
    }
}

// Computes how many tokens can be taken out of a pool if `token_amount_in` are sent, given the current balances.
// The amplification parameter equals: A n^(n-1)
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

    let balance_in_new = balances.get(token_index_in)?.checked_add(token_amount_in)?;
    let new_balances = balances_with(balances, token_index_in, balance_in_new);
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

    let balance_out_new = balances
        .get(token_index_out)?
        .checked_sub(token_amount_out)?;
    let new_balances = balances_with(balances, token_index_out, balance_out_new);
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
    let sum = checked_sum(balances)?;

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
    lp_from_invariant_ratio(pool_token_supply, new_invariant, current_invariant)
}

// LP that would be minted for a deposit if NO swap fee were charged on the
// imbalanced portion — i.e. the full deposit's invariant growth. The difference
// from `calc_pool_token_out_given_exact_tokens_in` is the swap fee expressed in
// LP shares, which sizes the deposit's protocol fee. Only called when a protocol
// fee is configured (it costs an extra invariant solve), so it is kept separate.
pub fn calc_pool_token_out_no_fee(
    e: &Env,
    amplification: u64,
    balances: &[u64],
    amounts_in: &[u64],
    pool_token_supply: u64,
    current_invariant: u64,
    inverse_threshold: Option<u64>,
) -> Option<u64> {
    let num_tokens = balances.len();
    if num_tokens > MAX_TOKENS {
        return None;
    }

    let mut new_balances = [0u64; MAX_TOKENS];
    for i in 0..num_tokens {
        new_balances[i] = balances[i].checked_add(amounts_in[i])?;
    }

    let new_invariant = calc_invariant(
        e,
        amplification,
        &new_balances[..num_tokens],
        inverse_threshold,
    )?;
    lp_from_invariant_ratio(pool_token_supply, new_invariant, current_invariant)
}

// Returns `(net_out, fee)`: the amount paid to the caller after the swap fee on
// the imbalanced (virtual-swap) portion, and the fee withheld. The fee already
// has to be computed to find `net_out`, so returning it is free; the caller
// decides how to split it between LPs and the protocol.
#[allow(clippy::too_many_arguments)]
pub(super) fn calc_token_out_given_exact_pool_token_in(
    e: &Env,
    amplification: u64,
    balances: &[u64],
    token_index: usize,
    amount_in: u64,
    pool_token_supply: u64,
    current_invariant: u64,
    swap_fee: u64,
) -> Option<(u64, u64)> {
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
    let sum = checked_sum(balances)?;

    // We can now compute how much excess balance is being withdrawn as a result of the virtual swaps, which result
    // in swap fees.
    let current_weight = balance.div_down(sum)?;
    let taxable_percentage = current_weight.complement();

    // Swap fees are typically charged on 'token in', but there is no 'token in' here, so we apply it
    // to 'token out'. This results in slightly larger price impact. Fees are rounded up.
    let taxable_amount = amount_out_without_fee.mul_up(taxable_percentage)?;
    let non_taxable_amount = amount_out_without_fee.saturating_sub(taxable_amount);

    let net_out = taxable_amount
        .mul_down(swap_fee.complement())?
        .checked_add(non_taxable_amount)?;
    let fee = amount_out_without_fee.checked_sub(net_out)?;
    Some((net_out, fee))
}

// This function calculates the balance of a given token (token_index)
// given all the other balances and the invariant
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
        if token_balance_u64.abs_diff(prev_token_balance_u64) <= BALANCE_THRESHOLD {
            return Some(token_balance_u64);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::Env;
    use std::vec::Vec as StdVec;

    // ------------------------------------------------------------------
    // Test-vector provenance
    //
    // The exact numeric vectors in `test_calc_out_given_in` and
    // `test_calc_pool_token_out_given_exact_tokens_in` (balances, invariants,
    // and expected outputs such as 999_845_869) are carried over from the
    // upstream Stabble stable-swap math test suite (the Solana program this
    // module ports; its math itself follows Balancer's StableMath / Curve's
    // StableSwap). Reproducing the upstream expected values bit-for-bit is the
    // translation-correctness check for the port: an independent, audited
    // implementation computed the same numbers from the same inputs.
    //
    // The tests below that (invariant preservation, slippage bounds, the
    // amplification ladder, exact-in/exact-out consistency, and the property
    // tests in `math::proptests`) are new for this repository and verify the
    // behavior of the curve itself rather than agreement with upstream.
    // ------------------------------------------------------------------

    /// All balances are on the internal 9-decimal scale: 1e15 == 1,000,000.0
    /// tokens. Amplification arguments are raw (`factor * AMP_PRECISION`).
    const ONE_MILLION_TOKENS: u64 = 1_000_000_000_000_000;

    /// Post-swap balance vector for a math-level swap: token `i` grows by
    /// `amount_in`, token `j` shrinks by `amount_out`.
    fn balances_after_swap(
        balances: &[u64],
        i: usize,
        j: usize,
        amount_in: u64,
        amount_out: u64,
    ) -> StdVec<u64> {
        let mut v = balances.to_vec();
        v[i] = v[i].checked_add(amount_in).unwrap();
        v[j] = v[j].checked_sub(amount_out).unwrap();
        v
    }

    /// Trade-size ladder used by the table-driven tests, in basis points of
    /// the input-token reserve: 0.01%, 1%, 10%, 30%.
    const TRADE_SIZE_BPS: &[u64] = &[1, 100, 1_000, 3_000];

    /// Pool shapes: 2-5 tokens, balanced and (heavily) imbalanced.
    const POOLS: &[&[u64]] = &[
        // 2 tokens, balanced
        &[ONE_MILLION_TOKENS, ONE_MILLION_TOKENS],
        // 2 tokens, 4:1
        &[1_600_000_000_000_000, 400_000_000_000_000],
        // 2 tokens, 99:1 (heavily imbalanced)
        &[1_980_000_000_000_000, 20_000_000_000_000],
        // 3 tokens, balanced
        &[ONE_MILLION_TOKENS, ONE_MILLION_TOKENS, ONE_MILLION_TOKENS],
        // 3 tokens, 20:5:1
        &[
            2_000_000_000_000_000,
            500_000_000_000_000,
            100_000_000_000_000,
        ],
        // 4 tokens, mildly imbalanced
        &[
            800_000_000_000_000,
            1_200_000_000_000_000,
            900_000_000_000_000,
            1_100_000_000_000_000,
        ],
        // 5 tokens, balanced
        &[
            ONE_MILLION_TOKENS,
            ONE_MILLION_TOKENS,
            ONE_MILLION_TOKENS,
            ONE_MILLION_TOKENS,
            ONE_MILLION_TOKENS,
        ],
        // 5 tokens, spread over two orders of magnitude
        &[
            2_000_000_000_000_000,
            1_000_000_000_000_000,
            500_000_000_000_000,
            100_000_000_000_000,
            50_000_000_000_000,
        ],
    ];

    /// Amplification factors (multiplied by AMP_PRECISION when calling the
    /// math), spanning the full supported range [MIN_AMP, MAX_AMP].
    const AMP_FACTORS: &[u64] = &[1, 10, 100, 1_000, 12_000];

    #[test]
    fn test_calc_out_given_in() {
        let e = Env::default();

        // The first three scenarios are upstream convergence vectors for
        // extreme balance spreads; assert the invariant value itself so a
        // subtly wrong (but converging) formula cannot slip through.
        let balances = [
            776199829833940141u64,
            2206504616663253113,
            1763368950384576155,
            38416709841306561,
            18833762826780,
        ];
        let amplification = 500_000;
        let invariant = calc_invariant(&e, amplification, &balances, None).unwrap();
        assert_eq!(invariant, 1913135429164488420);

        let balances = [1332693902458055177u64, 534042038714371533, 93673549035235];
        let amplification = 10_000;
        let invariant = calc_invariant(&e, amplification, &balances, None).unwrap();
        assert_eq!(invariant, 520894402283561740);

        let balances = [2397586296768312160u64, 2300831385038136337, 1410688950371];
        let amplification = 1_000;
        let invariant = calc_invariant(&e, amplification, &balances, None).unwrap();
        assert_eq!(invariant, 231343844339109190);

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

    // ------------------------------------------------------------------
    // Invariant preservation
    // ------------------------------------------------------------------

    // The invariant D never decreases across a swap, for every pool shape,
    // amplification, and trade size in the supported domain.
    //
    // Note the property is *non-decrease*, not equality: `calc_out_given_in`
    // deliberately rounds the output down (the trailing `- 1`), so the pool
    // keeps the rounding dust and D drifts *up* by a few units per swap. At
    // the contract level the swap fee (which stays in the pool) pushes D up
    // further — asserting strict equality would reject correct behavior.
    // Here at the math level there is no fee, so D must also stay *nearly*
    // equal: the increase is bounded to rounding/convergence noise.
    #[test]
    fn invariant_never_decreases_after_swap() {
        // Bound on the D increase, on the internal 9-decimal scale (1e9 ==
        // one token). The worst case observed across this whole table is
        // +8_733 units — 0.0000087 tokens, on the 99:1 pool at amp 12000
        // trading 30% of the reserve — so 10_000 asserts "the swap leaked
        // nothing beyond rounding" while leaving no room for a real leak.
        const INCREASE_TOL: u64 = 10_000;

        for &balances in POOLS {
            let n = balances.len();
            for &amp_factor in AMP_FACTORS {
                let amplification = amp_factor * AMP_PRECISION;
                for &size_bps in TRADE_SIZE_BPS {
                    // Trade both directions: into the last token and into the
                    // first, which for imbalanced pools covers both "drain the
                    // scarce token" and "pile onto the plentiful one".
                    for (i, j) in [(0, n - 1), (n - 1, 0)] {
                        let e = Env::default();
                        let amount_in = (balances[i] as u128 * size_bps as u128 / 10_000) as u64;

                        let d_before = calc_invariant(&e, amplification, balances, None).unwrap();
                        let out = calc_out_given_in(
                            &e,
                            amplification,
                            balances,
                            i,
                            j,
                            amount_in,
                            d_before,
                        )
                        .unwrap();
                        assert!(out < balances[j], "swap may not drain the reserve");

                        let after = balances_after_swap(balances, i, j, amount_in, out);
                        let d_after = calc_invariant(&e, amplification, &after, None).unwrap();

                        // No tolerance on the downside: the output's
                        // round-down (`- 1`) guarantees the pool keeps the
                        // dust, so D must not drop by even one unit.
                        assert!(
                            d_after >= d_before,
                            "D decreased: {} -> {} (pool {:?}, amp {}, {} bps)",
                            d_before,
                            d_after,
                            balances,
                            amp_factor,
                            size_bps
                        );
                        assert!(
                            d_after - d_before <= INCREASE_TOL,
                            "D grew beyond rounding: {} -> {} (+{}) (pool {:?}, amp {}, {} bps)",
                            d_before,
                            d_after,
                            d_after - d_before,
                            balances,
                            amp_factor,
                            size_bps
                        );
                    }
                }
            }
        }
    }

    // ------------------------------------------------------------------
    // Quantified slippage
    // ------------------------------------------------------------------

    /// Execution slippage of a quote in hundredths of a basis point
    /// (1 cbp == 0.0001%): how far `out` fell short of the 1:1 ideal.
    /// This is the pure curve effect — the math layer charges no fee.
    fn slippage_cbps(amount_in: u64, amount_out: u64) -> u64 {
        assert!(amount_out <= amount_in, "balanced-pool quote above par");
        ((amount_in - amount_out) as u128 * 1_000_000 / amount_in as u128) as u64
    }

    // On a balanced two-token pool at the production amplification (100, the
    // factor the testnet pool runs), curve slippage stays far below one basis
    // point for everyday trade sizes and stays bounded even at 30% of the
    // reserve. The expected outputs are exact; the cbp ceilings are the
    // human-readable claim.
    #[test]
    fn balanced_stable_pair_slippage_below_expected_bps() {
        let e = Env::default();
        let balances = [ONE_MILLION_TOKENS, ONE_MILLION_TOKENS];
        let amplification = 100 * AMP_PRECISION;
        let invariant = calc_invariant(&e, amplification, &balances, None).unwrap();

        // (trade size in bps of the reserve, expected out, max slippage in cbp)
        let cases: &[(u64, u64, u64)] = &[
            (1, 99_999_900_988, 1),              // 0.01% trade: < 0.01 bp
            (10, 999_990_099_097, 10),           // 0.1% trade:  ~0.09 bp
            (100, 9_999_009_901_969, 100),       // 1% trade:    ~0.99 bp
            (1_000, 99_900_110_864_756, 1_000),  // 10% trade:   ~9.98 bp
            (3_000, 299_026_254_209_234, 3_300), // 30% trade:  ~32.45 bp
        ];

        let mut previous_cbps = 0;
        for &(size_bps, expected_out, max_cbps) in cases {
            let amount_in = (ONE_MILLION_TOKENS as u128 * size_bps as u128 / 10_000) as u64;
            let out = calc_out_given_in(&e, amplification, &balances, 0, 1, amount_in, invariant)
                .unwrap();
            let cbps = slippage_cbps(amount_in, out);
            assert_eq!(out, expected_out, "out for {} bps trade", size_bps);
            assert!(
                cbps <= max_cbps,
                "slippage {} cbp above ceiling {} for {} bps trade",
                cbps,
                max_cbps,
                size_bps
            );
            // Price impact grows monotonically with trade size.
            assert!(cbps >= previous_cbps);
            previous_cbps = cbps;
        }
    }

    // ------------------------------------------------------------------
    // Amplification ladder
    // ------------------------------------------------------------------

    // Near balance, a higher amplification factor gives strictly lower
    // slippage, and every rung of the ladder beats the constant-product
    // (x*y=k) price for the same trade.
    #[test]
    fn higher_amplification_reduces_slippage_near_balance() {
        let balances = [ONE_MILLION_TOKENS, ONE_MILLION_TOKENS];
        let amount_in = ONE_MILLION_TOKENS / 100; // 1% of the reserve

        // Constant-product reference on the same reserves:
        // out = R * in / (R + in), a ~99 bps hit for a 1% trade.
        let r = ONE_MILLION_TOKENS as u128;
        let cp_out = (r * amount_in as u128 / (r + amount_in as u128)) as u64;

        let mut previous_out = cp_out;
        for &amp_factor in AMP_FACTORS {
            let e = Env::default();
            let amplification = amp_factor * AMP_PRECISION;
            let invariant = calc_invariant(&e, amplification, &balances, None).unwrap();
            let out = calc_out_given_in(&e, amplification, &balances, 0, 1, amount_in, invariant)
                .unwrap();
            assert!(
                out > previous_out,
                "amp {} must out-price the previous rung: {} <= {}",
                amp_factor,
                out,
                previous_out
            );
            previous_out = out;
        }

        // Endpoints, pinned exactly: constant product loses ~99 bps on this
        // trade; the ladder runs 49.75 bp (amp 1) down to 0.08 bp (amp 12000).
        assert_eq!(cp_out, 9_900_990_099_009);
        assert_eq!(previous_out, 9_999_991_666_532); // out at amp 12000
    }

    // ------------------------------------------------------------------
    // Exact-in / exact-out consistency
    // ------------------------------------------------------------------

    // Quoting an input for the output of the opposite quote returns to the
    // starting amount within rounding: the two Newton solvers describe the
    // same curve. Both quotes round in the pool's favor, so the recovered
    // input may sit a few units above or below the original, never far.
    #[test]
    fn exact_in_and_exact_out_quotes_are_consistent() {
        // Absolute drift bound in 9-dec units. The worst case observed is
        // 801 units (0.0000008 tokens), on the 99:1 pool at amp 12000 where
        // the scarce-token price is steepest; 1_000 keeps the bound tight.
        const ROUNDTRIP_TOL: u64 = 1_000;

        for &balances in POOLS {
            let n = balances.len();
            for &amp_factor in AMP_FACTORS {
                let amplification = amp_factor * AMP_PRECISION;
                for &size_bps in &[10u64, 100, 1_000] {
                    let e = Env::default();
                    let amount_in = (balances[0] as u128 * size_bps as u128 / 10_000) as u64;
                    let invariant = calc_invariant(&e, amplification, balances, None).unwrap();

                    let out = calc_out_given_in(
                        &e,
                        amplification,
                        balances,
                        0,
                        n - 1,
                        amount_in,
                        invariant,
                    )
                    .unwrap();
                    let recovered_in =
                        calc_in_given_out(&e, amplification, balances, 0, n - 1, out, invariant)
                            .unwrap();

                    let drift = recovered_in.abs_diff(amount_in);
                    assert!(
                        drift <= ROUNDTRIP_TOL,
                        "roundtrip drift {} (in {}, recovered {}) (pool {:?}, amp {}, {} bps)",
                        drift,
                        amount_in,
                        recovered_in,
                        balances,
                        amp_factor,
                        size_bps
                    );
                }
            }
        }
    }

    // Out-of-domain inputs fail closed (None -> contract MathError) instead
    // of returning a wrong number: too many tokens, a swap that would require
    // more than the whole reserve, and a zero-token pool.
    #[test]
    fn out_of_domain_inputs_return_none() {
        let e = Env::default();

        // More tokens than MAX_TOKENS.
        let six = [ONE_MILLION_TOKENS; 6];
        assert_eq!(calc_invariant(&e, 100 * AMP_PRECISION, &six, None), None);

        // Requesting more out than the reserve holds.
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
                invariant
            ),
            None
        );

        // An empty pool has invariant 0 (guarded before any iteration).
        assert_eq!(calc_invariant(&e, 100 * AMP_PRECISION, &[], None), Some(0));
    }
}
