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
| Amplification ramp | Linear interpolation between `ramp_start_ts`/`ramp_stop_ts`, `MIN_AMP = 1`, `MAX_AMP = 50000` |
| Domain limits | 2–5 tokens per pool, `u64` internal balances, `MAX_SAFE_BALANCE = 3e18`, per-token `max_caps`, LP `max_supply` |

## Adapted to Soroban (same behavior, different mechanism)

| Area | Upstream (Solana) | Here (Soroban) |
| --- | --- | --- |
| Wide arithmetic | Native big-integer types | `soroban_sdk::U256` host objects, with loop-invariant terms hoisted out of the Newton iterations (`math/arithmetic.rs`) |
| Error handling | Program errors | `Option`-based math that fails closed: any non-convergence or overflow returns `None` and surfaces as `Error::MathError`; out-of-domain inputs never produce a wrong number (see `out_of_domain_inputs_return_none`) |
| State | Program accounts + PDAs | One `Pool` struct in instance storage with TTL extension (`pool/state.rs`) |
| Authorization | PDA signers | OpenZeppelin Stellar `Ownable` for the pool creator and factory governance, plus an immutable factory address as each pool's protocol controller |
| Pause | `is_active` flag | OpenZeppelin `pausable`; the pool owner and protocol controller share one flag that blocks deposits/swaps while keeping withdrawals open |
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
- **Factory governance.** Factory defaults seed protocol fee configuration for
  new pools without becoming shared runtime state. Registered pools are updated
  individually through factory-owner-only proxy calls.
- **Amplification authority.** Pool creators irreversibly choose `Locked` or
  `ProtocolManaged`; only the factory controller may ramp a managed pool.
- **Contract events** (`deposit`, `withdraw`, `withdraw_one_token`, `swap`)
  for off-chain indexers, alongside the SEP-41 token events.

## Execution evidence

Recorded testnet transactions, simulation quotes, and observed slippage live in
[testnet-swap-evidence.md](testnet-swap-evidence.md).
