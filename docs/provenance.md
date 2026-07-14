# Provenance: Translated, Adapted, and New

Spreadless is a Soroban port of the [Stabble](https://github.com/stabbleorg)
stable-swap program (Solana). Stabble's math in turn follows Balancer's
`StableMath` and the Curve StableSwap invariant. This document states
explicitly which parts of this repository are a faithful translation of the
upstream implementation, which parts were adapted to the Soroban platform, and
which parts are new — so a reviewer can tell at a glance what is
battle-tested math carried over and what needs fresh scrutiny.

## Translated as-is (same formulas, same semantics, same constants)

| Area | Detail |
| --- | --- |
| Invariant `D` | Newton–Raphson solve of the StableSwap invariant (`math/stable.rs::calc_invariant`), amplification convention `A·n^(n-1)`, `AMP_PRECISION = 1000` |
| Swap solvers | `calc_out_given_in` / `calc_in_given_out` solve the same per-token quadratic via the same iteration (`get_token_balance_given_invariant_n_all_other_balances`), with the same round-in-the-pool's-favor adjustments (`-1` on outputs, `+1` on inputs) |
| Deposit math | `calc_pool_token_out_given_exact_tokens_in`: balanced portion joins fee-free, the imbalanced (taxable) portion pays the swap fee — the Balancer join convention. First deposit mints LP equal to `D` |
| Single-token exit math | `calc_token_out_given_exact_pool_token_in` with the swap fee applied to the taxable share of the output |
| Fee model | Swap fee charged on the **output** amount (`out × complement(fee)`), rate scale `1e9 == 100%`, bounds 0.001%–1% (`10_000`–`10_000_000`) |
| Accounting scale | All math at a common 9-decimal internal precision (`ONE = 1e9`); per-token static scaling factors derived from token decimals; no rate providers |
| Amplification ramp | Linear interpolation between `ramp_start_ts`/`ramp_stop_ts`, `MIN_AMP = 1`, `MAX_AMP = 12000` |
| Domain limits | 2–5 tokens per pool, `u64` internal balances, `MAX_SAFE_BALANCE = 3e18`, per-token `max_caps`, LP `max_supply` |
| Test vectors | The numeric vectors in `math/stable.rs` (`test_calc_out_given_in`, `test_calc_pool_token_out_given_exact_tokens_in`) reproduce the upstream test suite's expected values bit-for-bit — the translation-correctness check for the port |

## Adapted to Soroban (same behavior, different mechanism)

| Area | Upstream (Solana) | Here (Soroban) |
| --- | --- | --- |
| Wide arithmetic | Native big-integer types | `soroban_sdk::U256` host objects, with loop-invariant terms hoisted out of the Newton iterations (`math/arithmetic.rs`) |
| Error handling | Program errors | `Option`-based math that fails closed: any non-convergence or overflow returns `None` and surfaces as `Error::MathError`; out-of-domain inputs never produce a wrong number (see `out_of_domain_inputs_return_none`) |
| State | Program accounts + PDAs | One `Pool` struct in instance storage with TTL extension (`pool/state.rs`) |
| Authorization | PDA signers | `require_auth` plus OpenZeppelin Stellar `Ownable` (two-step transfer, as upstream's `pending_owner`) and `#[only_owner]` |
| Pause | `is_active` flag | OpenZeppelin `pausable` (`pause`/`unpause`/`paused`) |
| LP shares | SPL token mint | The pool contract **is** its own SEP-41 token (OpenZeppelin fungible `Base`, 9 decimals); direct `burn`/`burn_from` are disabled so exits always pass through the pool's accounting |

## New in Spreadless (no upstream equivalent)

- **No shared vault.** Upstream uses a Balancer-V2-style vault holding every
  pool's funds; here each pool holds its own token balances, and reserves are
  reconciled against actual token-contract balances on every transfer.
- **Transfer-delta verification.** Every transfer in/out checks the token
  contract's balance delta and rejects fee-on-transfer or otherwise
  non-standard tokens (`Error::TransferAmountMismatch`) instead of silently
  mis-crediting reserves.
- **Exact-output swaps on-chain.** Upstream exposes exact-in only (the
  exact-out math exists but is unexposed); `swap_exact_out` with a `max_in`
  bound is a first-class entrypoint here.
- **Single-token withdrawals on-chain.** Upstream's withdraw entrypoint is
  proportional-only; `withdraw_one_token` exposes the single-token exit math
  with the fee on the imbalanced portion.
- **Protocol fee.** Upstream's pool has no protocol-fee field (its vault
  beneficiary is paid on swaps only). Here `protocol_fee` is a configurable
  cut of the swap fee, applied *wherever* a swap fee is charged — swaps, the
  imbalanced portion of deposits, and single-token withdrawals — paid in the
  output token (swaps, single-token exits) or freshly minted LP (deposits).
  The trader's/LP's own payout is never reduced by it; the cut comes out of
  the fee.
- **Contract events** (`deposit`, `withdraw`, `withdraw_one_token`, `swap`)
  for off-chain indexers, alongside the SEP-41 token events.

## How correctness is checked

Two independent lines of evidence, both in `contracts/liquidity-pool/src`:

1. **Agreement with upstream** — the translated math reproduces the upstream
   test vectors exactly (`math/stable.rs` tests).
2. **Properties of the curve itself** — independent of any reference values:
   the invariant never decreases across a swap (`invariant_never_decreases_after_swap`),
   slippage is quantified in basis points and bounded per trade size
   (`balanced_stable_pair_slippage_below_expected_bps`), higher amplification
   strictly reduces slippage and beats constant-product pricing
   (`higher_amplification_reduces_slippage_near_balance`), exact-in and
   exact-out are mutually consistent
   (`exact_in_and_exact_out_quotes_are_consistent`), and randomized
   property-based tests sweep pool shapes, amplifications, and trade sizes
   across the supported domain (`math/proptests.rs`). Contract-level tests
   assert executed swaps equal the math-module quotes exactly, that
   `min_out`/`max_in` violations reject with `SlippageExceeded` while moving
   nothing, and that the invariant recomputed from public reserves never
   decreases (`test.rs`).

Testnet execution evidence lives in
[testnet-swap-evidence.md](testnet-swap-evidence.md).
