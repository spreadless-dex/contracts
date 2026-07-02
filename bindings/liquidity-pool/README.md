# @spreadless-dex/sdk

TypeScript SDK for the [Spreadless](https://github.com/spreadless-dex/contracts)
`liquidity-pool` Soroban contract — a StableSwap-style AMM on Stellar for
swapping between correlated assets with low slippage.

The SDK is a thin, fully-typed client generated from the deployed contract. It
wraps every entrypoint (swaps, deposits, withdrawals, the SEP-41 LP token, and
admin controls) in an `async` method and talks to the network over Soroban RPC.

- [Install](#install)
- [Deployed contracts](#deployed-contracts)
- [Quick start](#quick-start)
- [How the client works](#how-the-client-works)
- [Recipes](#recipes)
- [Method reference](#method-reference)
- [Error codes](#error-codes)
- [Building from source](#building-from-source)

## Install

```sh
npm install @spreadless-dex/sdk
```

`@stellar/stellar-sdk` is a dependency and is re-exported, so you don't need to
install it separately — `Keypair`, `Address`, the `contract` helpers, and the
`rpc` client are all available from this package.

Requires Node 18+ (or any runtime with global `fetch` and `BigInt`).

## Deployed contracts

The SDK does **not** bundle a `networks` constant — you pass the contract ID,
network passphrase, and RPC URL yourself. The current **testnet** deployment:

| What | Value |
| --- | --- |
| Pool contract | `CCAD3EH4P74PVYL3IC6ND7RSV6NYYOMUMNKRNVBJYOVIZP7Z2QS5XTSN` |
| Network passphrase | `Test SDF Network ; September 2015` |
| RPC URL | `https://soroban-testnet.stellar.org` |

Pool tokens, in **canonical order** (this is the order every amount array uses —
see [Token order](#token-order-matters)):

| Index | Symbol | Token contract | Decimals |
| --- | --- | --- | --- |
| 0 | sDAI | `CBXN4CMLFVDNVFSGNXFGP5EWI77ISC5KH5UXSDBQETZCJHYHA3KEP4JJ` | 7 |
| 1 | sUSDT | `CB2NS6KYG5ZBHHVKXCHYWLRRH4AKFXNWRYNSQTKNFW23CAY4SGSQVG75` | 7 |
| 2 | SUSD (SAC) | `CDDE66QMXWVUVEHLA5IRUJBHPJK3RFH6JIXCIJ5S6HOAXAPYR2AIZUWD` | 7 |
| 3 | sUSDC | `CDKFYHC3EPRCZY4DIMCIBQ3PO5QPD6KZFFXNMLS4XENY2QNTZN2KLMRM` | 7 |

The three `s*` tokens are open-mint test tokens: anyone can call
`mint(to, amount)` to fund an account for testing. Always treat
[`deployments/testnet.json`](https://github.com/spreadless-dex/contracts/blob/main/deployments/testnet.json)
in the repo as the source of truth — addresses change when the pool is
redeployed. Don't hardcode the reserve order from this table; read it live with
`get_tokens()`.

## Quick start

### Read pool state (no signing needed)

Read-only calls just simulate against the RPC, so you can omit signing options:

```ts
import { Client } from "@spreadless-dex/sdk";

const pool = new Client({
  contractId: "CCAD3EH4P74PVYL3IC6ND7RSV6NYYOMUMNKRNVBJYOVIZP7Z2QS5XTSN",
  rpcUrl: "https://soroban-testnet.stellar.org",
  networkPassphrase: "Test SDF Network ; September 2015",
});

// Every method returns an AssembledTransaction; `.result` holds the simulated value.
const tokens = (await pool.get_tokens()).result;   // string[]  (token order)
const reserves = (await pool.get_reserves()).result; // bigint[]  (raw units, same order)
const amp = (await pool.get_amp()).result;          // number

console.log({ tokens, reserves, amp });
```

### Swap (signs and submits a transaction)

State-changing calls need a source account that can sign. The account you pass
as `to` provides the input funds and must authorize the transaction — so the
signer's public key must equal `to`.

```ts
import { Client, contract, Keypair } from "@spreadless-dex/sdk";

const networkPassphrase = "Test SDF Network ; September 2015";
const kp = Keypair.fromSecret(process.env.STELLAR_SECRET_KEY!);

const pool = new Client({
  contractId: "CCAD3EH4P74PVYL3IC6ND7RSV6NYYOMUMNKRNVBJYOVIZP7Z2QS5XTSN",
  rpcUrl: "https://soroban-testnet.stellar.org",
  networkPassphrase,
  publicKey: kp.publicKey(),
  ...contract.basicNodeSigner(kp, networkPassphrase),
});

const [DAI, USDT] = (await pool.get_tokens()).result;

// Swap exactly 10.0 sDAI (7 decimals) for USDT, reverting below 9.95 out.
const tx = await pool.swap_exact_in({
  to: kp.publicKey(),
  token_in: DAI,
  token_out: USDT,
  amount_in: 100_000_000n, // 10.0 * 10^7
  min_out: 99_500_000n,    // slippage floor, raw units of USDT
});

// tx.result is the *simulated* output. Nothing has been submitted yet:
console.log("quote:", tx.result);

// Sign and broadcast to make it real:
const { result } = await tx.signAndSend();
console.log("received:", result, "raw units of USDT");
```

## How the client works

A few things to internalize before using the rest of the API.

### Every method returns an `AssembledTransaction`

Calling a method builds and **simulates** a transaction against the RPC, then
returns an `AssembledTransaction<T>`:

- **Read/view methods** (`get_reserves`, `balance`, `paused`, …) — the answer is
  already in `.result`. You never need to sign or send.
- **Write methods** (`swap_*`, `deposit`, `withdraw*`, `approve`, `transfer`,
  admin setters, …) — `.result` holds the *predicted* result from simulation.
  Call `await tx.signAndSend()` to submit; its return also has a `.result` with
  the on-chain outcome.

```ts
const tx = await pool.deposit({ to, amounts_in, min_lp_out });
tx.result;                       // simulated LP shares you'd receive
const sent = await tx.signAndSend();
sent.result;                     // actual LP shares minted
```

### Numbers: `i128`/`u64`/`u128` are `bigint`

Contract integer types map to JS `bigint`. Pass amounts as bigint literals
(`100_000_000n`), not `number`. `u32` values (`amp`, `live_until_ledger`) are
plain `number`.

### Amounts are raw token units

There is no decimal handling. A token with 7 decimals means `1.0` token =
`10_000_000n`. Reserves, swap amounts, caps, and payouts are all in each token's
raw on-chain units. The LP share token (`SLP`) uses 9 decimals.

### Token order matters

Every array argument and return — `amounts_in`, `min_amounts_out`,
`get_reserves()` — is indexed by the pool's canonical token order, which you get
from `get_tokens()`. Index into that array rather than assuming a fixed layout,
since order can differ from display labels and changes between pools.

### Always set slippage bounds

`deposit` takes `min_lp_out`, `withdraw` takes `min_amounts_out`, swaps take
`min_out` / `max_in`. These are enforced on-chain and revert with
`SlippageExceeded` (error 14) if the trade moves against you past the bound. In
production, derive them from the simulated `.result` minus your tolerance.

## Recipes

### Add liquidity

`amounts_in` follows `get_tokens()` order. The **first** deposit into a pool must
fund every token; later deposits may be balanced, imbalanced, or single-sided.

```ts
const tx = await pool.deposit({
  to: kp.publicKey(),
  amounts_in: [100_000_000n, 100_000_000n, 100_000_000n, 100_000_000n],
  min_lp_out: 0n, // set from the simulated quote in production
});
const { result: lpMinted } = await tx.signAndSend();
```

### Remove liquidity

Proportional exit (fee-free) — returns a slice of every reserve:

```ts
const tx = await pool.withdraw({
  to: kp.publicKey(),
  lp_amount: lpMinted,
  min_amounts_out: [0n, 0n, 0n, 0n],
});
const { result: amountsOut } = await tx.signAndSend(); // bigint[], token order
```

Single-token exit (charges the swap fee on the imbalanced portion):

```ts
const tx = await pool.withdraw_one_token({
  to: kp.publicKey(),
  lp_amount: lpMinted,
  token_out: USDT,
  min_amount_out: 0n,
});
const { result } = await tx.signAndSend();
```

### Swap for an exact output

```ts
const tx = await pool.swap_exact_out({
  to: kp.publicKey(),
  token_in: DAI,
  token_out: USDT,
  amount_out: 100_000_000n, // want exactly 10.0 USDT out
  max_in: 101_000_000n,     // spend at most 10.1 DAI
});
const { result: amountIn } = await tx.signAndSend();
```

### Check an LP share balance

The pool contract is itself the SEP-41 LP token, so token methods live on the
same client:

```ts
const shares = (await pool.balance({ account: kp.publicKey() })).result;
const supply = (await pool.total_supply()).result;
```

## Method reference

Every method returns `Promise<AssembledTransaction<T>>`; `T` is listed below.
Inline JSDoc for each is available in your editor via `pool.` autocomplete.

### Views (read-only)

| Method | Returns | Description |
| --- | --- | --- |
| `get_tokens()` | `string[]` | Pool token addresses, in canonical order. |
| `get_reserves()` | `bigint[]` | Current reserves in raw units, token order. |
| `get_amp()` | `number` | Effective amplification factor (reflects any ramp). |
| `paused()` | `boolean` | Whether liquidity/swap ops are paused. |
| `get_owner()` | `string \| undefined` | Owner address, or `undefined` if renounced. |

### Swaps

| Method | Args | Returns |
| --- | --- | --- |
| `swap_exact_in` | `{ to, token_in, token_out, amount_in, min_out }` | `bigint` output sent |
| `swap_exact_out` | `{ to, token_in, token_out, amount_out, max_in }` | `bigint` input taken |

### Liquidity

| Method | Args | Returns |
| --- | --- | --- |
| `deposit` | `{ to, amounts_in, min_lp_out }` | `bigint` LP shares minted |
| `withdraw` | `{ to, lp_amount, min_amounts_out }` | `bigint[]` amounts out |
| `withdraw_one_token` | `{ to, lp_amount, token_out, min_amount_out }` | `bigint` amount out |

### LP token (SEP-41)

`balance` · `total_supply` · `decimals` · `name` · `symbol` · `allowance` ·
`approve` · `transfer` · `transfer_from` · `burn` · `burn_from`. Standard SEP-41
semantics. Note: direct LP `burn` outside `withdraw*` keeps supply and reserves
in sync only through the withdrawal paths — burning shares directly to reduce
your position is disabled (`DirectLpBurnDisabled`, error 20).

### Admin (owner only)

| Method | Args |
| --- | --- |
| `set_amp_ramp` | `{ target_factor, duration }` — linear A ramp over `duration` seconds |
| `set_swap_fee` | `{ swap_fee }` — `1_000_000_000` = 100% |
| `set_protocol_fee` | `{ protocol_fee }` — cut of the swap fee routed to beneficiary |
| `set_beneficiary` | `{ beneficiary }` |
| `set_max_supply` | `{ max_supply }` — LP supply cap |
| `set_token_cap` | `{ token, max_cap }` — per-token reserve cap, raw units |
| `pause` / `unpause` | — |

### Ownership (two-step)

`transfer_ownership({ new_owner, live_until_ledger })` →
`accept_ownership()` (called by the new owner) · `renounce_ownership()`.

## Error codes

Contract failures surface as a thrown error carrying a numeric code. The
pool-specific codes:

| Code | Name | Meaning |
| --- | --- | --- |
| 1 | `InvalidTokenCount` | Pool needs 2–5 tokens. |
| 2 | `TokensNotSorted` | Constructor tokens not strictly ascending. |
| 9 | `AmountsLengthMismatch` | Array length ≠ token count. |
| 10 | `InvalidAmount` | Negative or otherwise invalid amount. |
| 11 | `ZeroDeposit` | Deposit contributes nothing. |
| 12 | `FirstDepositNotFull` | First deposit must fund every token. |
| 13 | `MathError` | Invariant/quote math failed to converge. |
| 14 | `SlippageExceeded` | Result crossed your `min_*` / `max_in` bound. |
| 15 | `CapExceeded` | A reserve or LP-supply cap would be exceeded. |
| 17 | `UnknownToken` | Address is not a pool token. |
| 18 | `SameToken` | `token_in == token_out`. |
| 19 | `TransferAmountMismatch` | Token moved a different amount than expected (fee-on-transfer tokens are rejected). |
| 20 | `DirectLpBurnDisabled` | LP shares can only exit via `withdraw*`. |

The full list, plus the `Errors`, `PausableError` (1000–1001), `OwnableError`
(2100–2102), `RoleTransferError` (2200–2203), and `FungibleTokenError` (100–114)
maps, are exported for programmatic lookup:

```ts
import { Errors, FungibleTokenError } from "@spreadless-dex/sdk";
Errors[14];              // { message: "SlippageExceeded" }
FungibleTokenError[100]; // { message: "InsufficientBalance" }
```

## Building from source

This package is published pre-built; installing from npm is all you need to use
it. To rebuild it (e.g. after regenerating bindings from a new contract build):

```sh
npm install
npm run build   # tsc -> dist/
```

Bindings are regenerated from the compiled wasm via the repo's
`make bindings` target. See the [main repository](https://github.com/spreadless-dex/contracts)
for contract sources, the full protocol description, and deployment tooling.
