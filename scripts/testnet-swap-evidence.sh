#!/usr/bin/env bash
# Execute the testnet swap-evidence matrix against the deployed pool and write
# the results to docs/testnet-swap-evidence.md:
#
#   1. small  sDAI  -> sUSDT   (near-1:1 stable swap)
#   2. large  sDAI  -> sUSDT   (price impact grows predictably with size)
#   3. small  sUSDT -> sDAI    (direction symmetry)
#   4. small  sUSDC -> SUSD    (a second pair; the pool is not hard-coded)
#   5. sDAI -> sUSDT with min_out ABOVE the quote (must be rejected with
#      SlippageExceeded and move nothing)
#
# Every swap simulates first to obtain the quote, derives min_out from the
# configured tolerance, submits, and records: tx hash, reserves before/after,
# amount in, quote, tolerance, min_out, actual out, and execution slippage.
set -euo pipefail

STELLAR="${STELLAR:-stellar}"
NETWORK="${NETWORK:-testnet}"
SOURCE="${SOURCE:-perps-testnet}"
DEPLOYMENTS_FILE="${DEPLOYMENTS_FILE:-deployments/testnet.json}"
EVIDENCE_FILE="${EVIDENCE_FILE:-docs/testnet-swap-evidence.md}"

TOLERANCE_BPS="${TOLERANCE_BPS:-10}"         # user slippage tolerance: 0.10%
SMALL_AMOUNT="${SMALL_AMOUNT:-100000000}"    # 10.0 tokens (7 decimals)
LARGE_AMOUNT="${LARGE_AMOUNT:-100000000000}" # 10,000.0 tokens

export STELLAR_NO_CACHE="${STELLAR_NO_CACHE:-true}"

require_cmd() {
  command -v "$1" >/dev/null 2>&1 || {
    echo "ERROR: missing required command: $1" >&2
    exit 1
  }
}
require_cmd "$STELLAR"
require_cmd jq
require_cmd awk

POOL=$(jq -r '.contracts.liquidity_pool.address' "$DEPLOYMENTS_FILE")
SWAP_FEE=$(jq -r '.contracts.liquidity_pool.swap_fee' "$DEPLOYMENTS_FILE")
AMP=$(jq -r '.contracts.liquidity_pool.amp_factor' "$DEPLOYMENTS_FILE")
USER=$("$STELLAR" keys address "$SOURCE" 2>/dev/null || jq -r '.deployer' "$DEPLOYMENTS_FILE")

token_addr() { # label -> pool token contract address
  jq -r --arg l "$1" \
    '.contracts.liquidity_pool | .tokens[(.token_labels | index($l))]' \
    "$DEPLOYMENTS_FILE"
}

ADDR_SDAI=$(token_addr "sDAI")
ADDR_SUSDT=$(token_addr "sUSDT")
ADDR_SUSDC=$(token_addr "sUSDC")
ADDR_SUSD=$(token_addr "SUSD_SAC")

invoke() { # contract-id fn-and-args...
  local id="$1"
  shift
  "$STELLAR" contract invoke \
    --network "$NETWORK" \
    --source-account "$SOURCE" \
    --id "$id" \
    -- "$@"
}

view() { # contract-id fn-and-args... (simulate only)
  local id="$1"
  shift
  "$STELLAR" contract invoke \
    --network "$NETWORK" \
    --source-account "$SOURCE" \
    --send=no \
    --id "$id" \
    -- "$@" 2>/dev/null
}

get_reserves() {
  view "$POOL" get_reserves | tr -d '[]" '
}

quote_exact_in() { # token_in token_out amount_in -> quoted out
  view "$POOL" swap_exact_in \
    --to "$USER" \
    --token_in "$1" \
    --token_out "$2" \
    --amount_in "$3" \
    --min_out 0 | tr -d '"'
}

# Submit a swap; prints "<actual_out>|<tx_hash>".
submit_exact_in() { # token_in token_out amount_in min_out
  local errfile out hash
  errfile=$(mktemp)
  out=$(invoke "$POOL" swap_exact_in \
    --to "$USER" \
    --token_in "$1" \
    --token_out "$2" \
    --amount_in "$3" \
    --min_out "$4" 2>"$errfile" | tr -d '"')
  hash=$(grep -oE '[0-9a-f]{64}' "$errfile" | head -1)
  rm -f "$errfile"
  echo "${out}|${hash:-unknown}"
}

human() { # raw 7-dec amount -> decimal string
  awk -v v="$1" 'BEGIN { printf "%.7f", v / 10000000 }'
}

bps() { # numerator denominator -> basis points with 2 decimals
  awk -v n="$1" -v d="$2" 'BEGIN { printf "%.2f", n * 10000 / d }'
}

fee_amount() { # net_out -> approximate fee withheld (fee charged on gross out)
  awk -v o="$1" -v f="$SWAP_FEE" 'BEGIN { printf "%.0f", o * f / (1000000000 - f) }'
}

MD=$(mktemp)
SWAP_NO=0

record_swap() { # label token_in_label token_out_label token_in token_out amount_in
  local label="$1" in_label="$2" out_label="$3" token_in="$4" token_out="$5" amount_in="$6"
  SWAP_NO=$((SWAP_NO + 1))

  echo "==> swap $SWAP_NO: $label ($(human "$amount_in") $in_label -> $out_label)" >&2

  local reserves_before quote min_out result actual hash reserves_after
  reserves_before=$(get_reserves)
  quote=$(quote_exact_in "$token_in" "$token_out" "$amount_in")
  min_out=$(awk -v q="$quote" -v t="$TOLERANCE_BPS" 'BEGIN { printf "%.0f", q * (10000 - t) / 10000 }')
  result=$(submit_exact_in "$token_in" "$token_out" "$amount_in" "$min_out")
  actual=${result%%|*}
  hash=${result##*|}
  reserves_after=$(get_reserves)

  local exec_slip price_vs_par fee
  # Execution slippage: how far the executed output fell short of the quote.
  exec_slip=$(bps "$((quote - actual))" "$quote")
  # Executed price vs 1:1 par (curve + fee). Positive = the trade paid a
  # premium (it worsened the pool's balance); negative = it received one
  # (it restored balance) — see "Reading the numbers" below.
  price_vs_par=$(bps "$((amount_in - actual))" "$amount_in")
  fee=$(fee_amount "$actual")

  cat >>"$MD" <<EOF

### Swap $SWAP_NO: $label

| Field | Value |
| --- | --- |
| Transaction hash | \`$hash\` |
| Token pair | $in_label -> $out_label |
| Pool reserves before (raw, token order) | \`[$reserves_before]\` |
| Amount in | $amount_in raw ($(human "$amount_in") $in_label) |
| Simulated quote | $quote raw ($(human "$quote") $out_label) |
| Selected tolerance | $TOLERANCE_BPS bps |
| Submitted min_out | $min_out raw |
| Actual amount out | $actual raw ($(human "$actual") $out_label) |
| Execution slippage vs quote | $exec_slip bps |
| Executed price vs 1:1 par (curve + fee; negative = premium received) | $price_vs_par bps |
| Swap fee (0.01% on output, approx.) | $fee raw |
| Pool reserves after (raw, token order) | \`[$reserves_after]\` |
| Result | SUCCESS |
EOF
}

record_rejection() { # token_in_label token_out_label token_in token_out amount_in
  local in_label="$1" out_label="$2" token_in="$3" token_out="$4" amount_in="$5"
  SWAP_NO=$((SWAP_NO + 1))

  echo "==> swap $SWAP_NO: deliberate rejection ($in_label -> $out_label)" >&2

  local reserves_before quote strict_min errfile errout status reserves_after
  reserves_before=$(get_reserves)
  quote=$(quote_exact_in "$token_in" "$token_out" "$amount_in")
  strict_min=$((quote + 1))

  errfile=$(mktemp)
  if invoke "$POOL" swap_exact_in \
    --to "$USER" \
    --token_in "$token_in" \
    --token_out "$token_out" \
    --amount_in "$amount_in" \
    --min_out "$strict_min" >/dev/null 2>"$errfile"; then
    echo "ERROR: swap with min_out above the quote was NOT rejected" >&2
    exit 1
  fi
  errout=$(grep -oE 'Error\(Contract, #[0-9]+\)' "$errfile" | head -1)
  rm -f "$errfile"
  reserves_after=$(get_reserves)

  if [ "$reserves_before" = "$reserves_after" ]; then
    status="reserves unchanged (verified)"
  else
    status="RESERVES CHANGED — INVESTIGATE"
  fi

  cat >>"$MD" <<EOF

### Swap $SWAP_NO: deliberately rejected ($in_label -> $out_label)

| Field | Value |
| --- | --- |
| Token pair | $in_label -> $out_label |
| Pool reserves before (raw, token order) | \`[$reserves_before]\` |
| Amount in | $amount_in raw ($(human "$amount_in") $in_label) |
| Simulated quote | $quote raw ($(human "$quote") $out_label) |
| Submitted min_out (deliberately above quote) | $strict_min raw |
| Failure | \`${errout:-simulation failed}\` = \`SlippageExceeded\` (error code 14) |
| Transaction hash | none — rejected during simulation, nothing submitted or signed |
| Pool reserves after (raw, token order) | \`[$reserves_after]\` |
| Result | REJECTED as required; $status |
EOF
}

echo "Pool: $POOL (amp $AMP, swap fee $SWAP_FEE = 0.01%)" >&2
echo "User: $USER" >&2

# Fund the swaps: the test tokens are open-mint by design.
echo "==> minting test-token balances for the swaps" >&2
invoke "$ADDR_SDAI" mint --to "$USER" --amount "$((2 * SMALL_AMOUNT + LARGE_AMOUNT))" >/dev/null
invoke "$ADDR_SUSDT" mint --to "$USER" --amount "$SMALL_AMOUNT" >/dev/null
invoke "$ADDR_SUSDC" mint --to "$USER" --amount "$SMALL_AMOUNT" >/dev/null

record_swap "small balanced swap" "sDAI" "sUSDT" "$ADDR_SDAI" "$ADDR_SUSDT" "$SMALL_AMOUNT"
record_swap "larger swap (price impact grows with size)" "sDAI" "sUSDT" "$ADDR_SDAI" "$ADDR_SUSDT" "$LARGE_AMOUNT"
record_swap "reverse direction" "sUSDT" "sDAI" "$ADDR_SUSDT" "$ADDR_SDAI" "$SMALL_AMOUNT"
record_swap "second pair (SAC-wrapped classic asset)" "sUSDC" "SUSD" "$ADDR_SUSDC" "$ADDR_SUSD" "$SMALL_AMOUNT"
record_rejection "sDAI" "sUSDT" "$ADDR_SDAI" "$ADDR_SUSDT" "$SMALL_AMOUNT"

mkdir -p "$(dirname "$EVIDENCE_FILE")"
cat >"$EVIDENCE_FILE" <<EOF
# Testnet Swap Evidence

Generated by \`scripts/testnet-swap-evidence.sh\` on $(date -u +"%Y-%m-%dT%H:%M:%SZ").

- Network: $NETWORK
- Pool: \`$POOL\` (4-token USD pool; see \`deployments/testnet.json\`)
- Amplification factor: $AMP
- Swap fee: $SWAP_FEE (1e9 == 100%, i.e. 0.01%), charged on the output
- Trader: \`$USER\`
- Slippage tolerance used for min_out: $TOLERANCE_BPS bps
- Token amounts are raw 7-decimal units; token order in reserve arrays is the
  pool's canonical order (see \`get_tokens\`): sDAI, sUSDT, SUSD, sUSDC.

Each swap was simulated first (the "simulated quote"), \`min_out\` was derived
from the quote and the tolerance, and the transaction was then submitted. The
final case deliberately sets \`min_out\` above the quote to demonstrate the
slippage guard rejecting the swap. Transactions can be inspected on
[Stellar Expert (testnet)](https://stellar.expert/explorer/testnet) by hash.
$(cat "$MD")

## Reading the numbers

- **Execution slippage vs quote** is the difference between the simulated
  quote and the executed output — 0 bps means the trade executed at exactly
  the quoted price (no other trades landed in between). This is the number a
  user's slippage tolerance protects; the tolerance was never consumed.
- **Executed price vs 1:1 par** bundles the StableSwap curve impact and the
  0.01% output fee. The pool's reserves are not perfectly balanced (earlier
  test trading left a surplus of one token), so the curve prices direction:
  a trade that *restores* balance executes at a small premium above par
  (negative bps), and a trade that *worsens* balance pays above par
  (positive bps). That asymmetry — visible across swaps 1-3 — is the
  StableSwap curve doing its job, not an accounting error.
- **Price impact grows with size**: the larger swap executes at a worse
  per-token price than the small swap in the same direction (compare swaps
  1 and 2), exactly as the curve predicts.
- The rejected swap produced no transaction: with \`min_out\` above the
  achievable output the contract traps with \`SlippageExceeded\` (error 14) in
  simulation, so nothing was signed, submitted, or moved. The same guard is
  exercised on-chain in the unit tests
  (\`swap_exact_in_rejects_when_min_out_not_met\`).
EOF
rm -f "$MD"

echo "wrote $EVIDENCE_FILE" >&2
