# Spreadless

Spreadless is a Soroban liquidity-pool contract for swapping between correlated
assets with low slippage. It supports 2 or more tokens, mints its own SEP-41 LP
share token, and keeps pool accounting in one contract.

The contract is implemented in Rust with `soroban-sdk` 26 and OpenZeppelin
Stellar helpers for ownership, pause control, and token behavior.

## How It Works

The pool tracks a sorted list of token contracts and a normalized reserve for
each token. Token amounts are accepted in each token's raw on-chain units, then
converted into an internal 9-decimal accounting scale before the pool math runs.
SAC assets with 7 decimals convert losslessly; tokens with more than 9 decimals
are truncated to the pool's precision.

Pricing uses an amplified invariant. The amplification factor `A` controls how
closely the pool behaves like a flat-price market around balance:

- Higher `A` gives lower slippage near balanced reserves, but makes imbalance
  sharper once the pool moves away from balance.
- Lower `A` gives more conservative pricing and lets prices move sooner as
  reserves diverge.
- The owner can ramp `A` linearly over time so parameter changes do not create
  an instant price jump.

LP shares are the pool contract's own SEP-41 token:

- The first deposit must include every pool token and mints LP shares equal to
  the initial invariant.
- Later deposits may be balanced, imbalanced, or single-sided. Only the
  imbalanced portion is charged the swap fee before LP shares are minted.
- Proportional withdrawals burn LP shares and return the same share of every
  reserve.
- Single-token withdrawals burn LP shares, reduce the invariant, and charge the
  swap fee on the imbalanced exit.
- Direct LP burns are disabled. Liquidity must exit through `withdraw` or
  `withdraw_one_token` so reserves and supply stay synchronized.

Swaps support both exact-input and exact-output flows. The swap fee is charged on
the output amount. A configured protocol-fee share of that fee is sent to the
beneficiary, while the rest remains in the pool for LPs.

The protocol-fee cut applies wherever a swap fee is charged: swaps, the
imbalanced portion of a deposit, and single-token withdrawals. It is paid to the
beneficiary in the output token for swaps and single-token withdrawals, and as
freshly minted LP shares for deposits (a join has no single output token). The
remainder of each fee stays in the pool for LPs, and proportional withdrawals are
fee-free.

The contract verifies token balance deltas during transfers. Fee-on-transfer or
otherwise non-standard token behavior is rejected instead of being silently
credited to reserves.

## Features

- Multi-asset pool with 2 to 5 tokens.
- Canonical token ordering enforced at construction.
- Per-token reserve caps and total LP supply cap.
- Exact-input and exact-output swaps.
- Balanced, imbalanced, and single-sided deposits.
- Proportional and single-token withdrawals.
- Output-fee accounting with optional protocol-fee beneficiary.
- Time-based amplification ramps.
- Owner-gated admin controls with two-step ownership.
- Pause and unpause controls for liquidity and swap operations.
- SEP-41 LP token with transfers and allowances.

## Contract Entrypoints

Liquidity operations:

- `deposit(to, amounts_in, min_lp_out) -> i128`
- `withdraw(to, lp_amount, min_amounts_out) -> Vec<i128>`
- `withdraw_one_token(to, lp_amount, token_out, min_amount_out) -> i128`

Swap operations:

- `swap_exact_in(to, token_in, token_out, amount_in, min_out) -> i128`
- `swap_exact_out(to, token_in, token_out, amount_out, max_in) -> i128`

Views:

- `get_reserves() -> Vec<i128>`
- `get_tokens() -> Vec<Address>`
- `get_amp() -> u32`
- `paused() -> bool`

Admin operations:

- `set_amp_ramp(target_factor, duration)`
- `set_swap_fee(swap_fee)`
- `set_protocol_fee(protocol_fee)`
- `set_beneficiary(beneficiary)`
- `set_max_supply(max_supply)`
- `set_token_cap(token, max_cap)`
- `pause()`
- `unpause()`

The pool also exposes OpenZeppelin ownership methods and SEP-41 LP-token
methods such as `balance`, `total_supply`, `approve`, `transfer`, and
`transfer_from`.

## Parameters

Constructor arguments:

- `owner`: address authorized for admin operations.
- `tokens`: sorted token contract addresses. The constructor rejects duplicates
  and unsorted input.
- `amp_factor`: amplification factor, from `1` to `12000`.
- `swap_fee`: fixed-point fee where `1_000_000_000` is 100%. Allowed range:
  `10_000` to `10_000_000`, or 0.001% to 1%.
- `protocol_fee`: share of the swap fee routed to the beneficiary, also using
  `1_000_000_000` as 100%.
- `beneficiary`: address that receives the protocol-fee share.
- `max_caps`: per-token reserve caps in raw token units.
- `lp_max_supply`: cap on total LP share supply.

All amount vectors use the pool token order returned by `get_tokens()`.

## Repository Layout

```text
.
├── contracts
│   └── liquidity-pool
│       ├── src
│       │   ├── contract.rs     # entrypoints, transfer checks, LP token impl
│       │   ├── interface.rs    # public interface documentation
│       │   ├── math            # invariant, swap, deposit, withdraw math
│       │   └── pool            # state, scaling, fees, quotes, amp ramps
│       └── Cargo.toml
├── Cargo.toml
├── Makefile
└── README.md
```

## Development

Install the pinned Rust toolchain and Soroban wasm target:

```sh
make setup
```

Build the contract:

```sh
make build
```

Generate TypeScript bindings from the built wasm:

```sh
make bindings
```

Run the test suite:

```sh
make test
```

Run formatting and lint checks:

```sh
make fmt-check
make lint
```

Build an optimized wasm:

```sh
make optimize
```

## Deploy

The Makefile includes a 2-token deployment template. `TOKEN_A` and `TOKEN_B`
must be SEP-41-compatible token contract addresses in strictly ascending order.

```sh
make deploy \
  OWNER=<owner-address> \
  TOKEN_A=<first-token-contract> \
  TOKEN_B=<second-token-contract> \
  BENEFICIARY=<fee-beneficiary> \
  AMP_FACTOR=100 \
  SWAP_FEE=100000 \
  PROTOCOL_FEE=0
```

Useful deployment variables:

- `NETWORK`: Stellar CLI network name. Defaults to `testnet`.
- `SOURCE`: Stellar CLI key name used to deploy. Defaults to `default`.
- `MAX_CAP`: per-token cap used by the template.
- `LP_MAX_SUPPLY`: total LP-share supply cap.
- `STELLAR`: CLI binary. Set to `soroban` if using an older install.

Create and fund a deployment identity for the configured network:

```sh
make keys SOURCE=default NETWORK=testnet
```

## Safety Notes

- Keep token order stable. Amount arrays, reserve arrays, caps, and minimums all
  follow the sorted order returned by `get_tokens()`.
- Set slippage limits on every deposit, withdrawal, and swap.
- Confirm token decimals before deployment. The contract supports tokens that
  can be represented in the internal 9-decimal scale.
- Reserve caps and LP supply caps are enforced on-chain.
- Pausing blocks deposits, withdrawals, and swaps, but does not disable view or
  admin methods.
- There is no minimum-liquidity lock. The usual first-depositor inflation attack
  is mitigated structurally instead: reserves are tracked internally (direct
  token donations do not change them), the first deposit must fund every token,
  and the LP shares minted on it equal the invariant `D` rather than a
  manipulable share price. A dust-sized first deposit is still discouraged, as it
  can make early share-math rounding coarse.
