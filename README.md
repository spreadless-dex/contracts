# Spreadless

Spreadless is a Soroban pool protocol for swapping between correlated
assets with low slippage. It supports 2 or more tokens, mints its own SEP-41 LP
share token, and keeps pool accounting in one contract.

The Spreadless router contract permissionlessly deploys pools from a
governance-selected pool WASM hash and assigns each pool an on-chain
ID. The authenticated creator owns the new pool; identical token baskets are
intentionally allowed. The router is also each
pool's immutable protocol controller, while router ownership determines who
may exercise that authority.

The contract is implemented in Rust with `soroban-sdk` 26 and OpenZeppelin
Stellar helpers for ownership, pause control, and token behavior.

The pool math is a port of Stabble's Solana stable-swap program, which itself
follows Balancer's StableMath and the Curve StableSwap invariant.
[docs/provenance.md](docs/provenance.md) breaks down explicitly what was
translated as-is, what was adapted to Soroban, and what is new in this
repository; [docs/testnet-swap-evidence.md](docs/testnet-swap-evidence.md)
records executed testnet swap transactions with quotes, tolerances, and
observed slippage.

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
- At creation the creator permanently chooses `Locked` amplification or
  `ProtocolManaged` amplification. Locked amplification never changes;
  protocol-managed amplification can be changed only through router
  governance, either immediately or with a linear ramp.

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
- Locked or protocol-managed amplification.
- Separate pool-owner and protocol-governance authority.
- Non-renounceable, two-step ownership transfer.
- Owner and protocol pause controls that keep withdrawals open.
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
- `get_amp_control() -> AmpControl`
- `get_protocol_controller() -> Address`
- `get_swap_fee() -> u64`
- `get_protocol_fee() -> u64`
- `get_beneficiary() -> Address`
- `get_max_supply() -> i128`
- `paused() -> bool`

Pool-owner operations:

- `set_swap_fee(swap_fee)`
- `set_max_supply(max_supply)`
- `set_token_cap(token, max_cap)`
- `pause()`
- `unpause()`

Protocol-controller operations, normally invoked by router governance:

- `set_amp_ramp(target_factor, duration)`
- `set_protocol_fee(protocol_fee)`
- `set_beneficiary(beneficiary)`
- `protocol_pause()`
- `protocol_unpause()`

The pool also exposes OpenZeppelin's two-step ownership methods and SEP-41
LP-token methods such as `balance`, `total_supply`, `approve`, `transfer`, and
`transfer_from`. Ownership renunciation always reverts; control must be
transferred explicitly.

Router operations:

- `create_pool(creator, tokens, amp_factor, amp_control, swap_fee, max_caps, lp_max_supply, lp_name, lp_symbol)`
- `swap_exact_in(to, token_in, path, amount_in, min_out) -> i128`, where each
  path hop contains a registered `pool_id` and its `token_out`
- `next_pool_id()` and `pool_at(id)`
- `set_default_protocol_fee(new_fee)`
- `set_default_protocol_beneficiary(new_beneficiary)`
- `set_pool_protocol_fee(pool_id, new_fee)`
- `set_pool_beneficiary(pool_id, new_beneficiary)`
- `set_pool_amp_ramp(pool_id, target_factor, duration)`
- `pause_pool(pool_id)` and `unpause_pool(pool_id)`
- `set_pool_wasm_hash(new_hash)`

## Parameters

Constructor arguments:

- `owner`: address authorized for admin operations.
- `protocol_controller`: immutable protocol-governance contract address.
- `tokens`: sorted token contract addresses. The constructor rejects duplicates
  and unsorted input.
- `amp_factor`: amplification factor, from `1` to `50000`.
- `amp_control`: irreversible `Locked` or `ProtocolManaged` mode.
- `swap_fee`: fixed-point fee where `1_000_000_000` is 100%. Allowed range:
  `10_000` to `10_000_000`, or 0.001% to 1%.
- `protocol_fee`: share of the swap fee routed to the beneficiary, also using
  `1_000_000_000` as 100%.
- `beneficiary`: address that receives the protocol-fee share.
- `max_caps`: per-token reserve caps in raw token units.
- `lp_max_supply`: cap on total LP share supply.
- `lp_name`: SEP-41 name for the pool's LP-share token.
- `lp_symbol`: SEP-41 symbol (ticker) for the pool's LP-share token.

All amount vectors use the pool token order returned by `get_tokens()`.

## Repository Layout

```text
.
├── contracts
│   ├── spreadless-pool-interface # shared pool ABI and generated Rust client
│   ├── spreadless-pool
│       ├── src
│       │   ├── contract.rs     # entrypoints, transfer checks, LP token impl
│       │   ├── math            # invariant, swap, deposit, withdraw math
│       │   └── pool            # state, scaling, fees, quotes, amp ramps
│       └── Cargo.toml
│   └── spreadless-router       # deployer + registry + governance + routing
├── docs
│   ├── provenance.md           # translated vs adapted vs new, vs upstream
│   └── testnet-swap-evidence.md# recorded testnet swaps with slippage data
├── Cargo.toml
├── Makefile
└── README.md
```

## Development

Install the pinned Rust toolchain and Soroban wasm target:

```sh
make setup
```

Build both contracts:

```sh
make build
```

Generate TypeScript bindings from the built wasm:

```sh
make bindings
```

Run formatting and lint checks:

```sh
make fmt-check
make lint
```

Build optimized pool and router WASM files:

```sh
make optimize
```

Deploy a testnet pool with two open-mint test tokens and save the addresses:

```sh
make deploy-testnet SOURCE=<stellar-identity>
```

## Deploy

The Makefile includes a 2-token deployment template. It uploads the pool WASM,
deploys a router, then creates the pool through that router. `TOKEN_A` and
`TOKEN_B` must be SEP-41-compatible token addresses in strictly ascending order.

```sh
make deploy \
  OWNER=<owner-address> \
  TOKEN_A=<first-token-contract> \
  TOKEN_B=<second-token-contract> \
  BENEFICIARY=<fee-beneficiary> \
  AMP_FACTOR=100 \
  AMP_CONTROL=ProtocolManaged \
  SWAP_FEE=100000 \
  PROTOCOL_FEE=0 \
  LP_NAME='USD Stable LP' \
  LP_SYMBOL=usdSLP
```

Useful deployment variables:

- `NETWORK`: Stellar CLI network name. Defaults to `testnet`.
- `SOURCE`: Stellar CLI key name used to deploy. Defaults to `default`.
- `MAX_CAP`: per-token cap used by the template.
- `LP_MAX_SUPPLY`: total LP-share supply cap.
- `LP_NAME` / `LP_SYMBOL`: pool-specific SEP-41 metadata.
- `STELLAR`: CLI binary. Set to `soroban` if using an older install.

The router assigns monotonically increasing pool IDs. `next_pool_id()` returns
the ID that will be assigned next, while `pool_at(id)` resolves a registered
pool and refreshes that registry entry's TTL. `PoolCreated` events provide
off-chain discovery.

The router stores a default protocol fee and beneficiary. Pool creation copies
those values into the new pool; later default changes affect only future pools.
Router governance can update a registered pool's protocol fee, beneficiary,
delegated amplification, and pause state through ID-based proxy calls. Pool
configuration changes never alter router defaults. Updating the configured
pool WASM hash also affects only future creations. Pool creation does not seed
liquidity.

Create and fund a deployment identity for the configured network:

```sh
make keys SOURCE=default NETWORK=testnet
```

### Testnet Demo Deployment

For demo and integration testing, `make deploy-testnet` deploys:

- `sUSDC`: an uncapped SEP-41 test token with open `mint(to, amount)`.
- `sUSDT`: an uncapped SEP-41 test token with open `mint(to, amount)`.
- `sDAI`: an uncapped SEP-41 test token with open `mint(to, amount)`.
- A Spreadless router configured with the uploaded pool WASM hash.
- A Spreadless pool created through that router with the three token addresses.

The script mints an initial balance of each token to the deployer, seeds the
first pool deposit, and writes the resulting contract addresses to
`deployments/testnet.json`. This deployment is intentionally testnet-only; the
token `mint` entrypoint has no authorization.

The current checked-in testnet deployment also includes `SUSD`, a testnet
classic Stellar asset wrapped by SAC, in the active pool recorded in
`deployments/testnet.json`.

## Safety Notes

- Keep token order stable. Amount arrays, reserve arrays, caps, and minimums all
  follow the sorted order returned by `get_tokens()`.
- Set slippage limits on every deposit, withdrawal, and swap.
- Confirm token decimals before deployment. The contract supports tokens that
  can be represented in the internal 9-decimal scale.
- Reserve caps and LP supply caps are enforced on-chain.
- Pausing blocks deposits and swaps. Proportional and single-token withdrawals
  remain available so liquidity providers always retain an exit path. Either
  the pool owner or protocol governance may pause or unpause the shared flag.
- Factory and pool ownership cannot be renounced. Use the OpenZeppelin two-step
  ownership transfer, including when moving governance to a multisig wallet.
- There is no minimum-liquidity lock. The usual first-depositor inflation attack
  is mitigated structurally instead: reserves are tracked internally (direct
  token donations do not change them), the first deposit must fund every token,
  and the LP shares minted on it equal the invariant `D` rather than a
  manipulable share price. A dust-sized first deposit is still discouraged, as it
  can make early share-math rounding coarse.
