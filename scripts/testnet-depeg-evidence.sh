#!/usr/bin/env bash
# Demonstrate the router's depeg guard on testnet against Reflector's live
# testnet price feed, and record every step in docs/testnet-depeg-guard-evidence.md.
#
#   1. upgrade the testnet router in place to the locally built WASM
#   2. create a sUSDC/sUSDT pool, priced by Reflector's USDC and USDT feeds
#   3. governance protects it (2%, 15 min, 3 readings); an outsider's check is Healthy
#   4. a stale-data check is rejected without pausing
#   5. governance moves the peg to 1.05 to simulate a ~5% depeg on live data;
#      an outsider's check pauses the pool; swaps revert; withdrawals stay open;
#      neither the pool owner nor an outsider can lift the pause
#   6. a repeat check returns AlreadyPaused; governance restores the peg and
#      unpauses; trading resumes
#
# Any step that does not behave exactly as expected aborts the run.
set -euo pipefail

STELLAR="${STELLAR:-stellar}"
NETWORK="${NETWORK:-testnet}"
SOURCE="${SOURCE:-perps-testnet}"
KEEPER="${KEEPER:-spreadless-depeg-keeper}"
DEPLOYMENTS_FILE="${DEPLOYMENTS_FILE:-deployments/testnet.json}"
TARGET_TRIPLE="${TARGET_TRIPLE:-wasm32v1-none}"
OUT="${OUT:-docs/testnet-depeg-guard-evidence.md}"
# Reflector testnet "External CEX & DEX" feed: USD base, 14 decimals, 5-minute updates.
REFLECTOR="${REFLECTOR:-CCYOZJCOPG34LLQQ7N24YXBM7LL62R7ONMZ3G6WZAAYPB5OYKOMJRN63}"

export STELLAR_NO_CACHE="${STELLAR_NO_CACHE:-true}"

if [[ "$NETWORK" != "testnet" ]]; then
  echo "ERROR: testnet only (creates throwaway keys and a demo pool)." >&2
  exit 1
fi

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"
ROUTER_WASM="target/${TARGET_TRIPLE}/release/spreadless_router.wasm"
[[ -f "$ROUTER_WASM" ]] || { echo "ERROR: build first: make build-router" >&2; exit 1; }

ROUTER=$(jq -r '.contracts.router.address' "$DEPLOYMENTS_FILE")
token_addr() { jq -r --arg l "$1" '.contracts.test_tokens[] | select(.label == $l) | .address' "$DEPLOYMENTS_FILE"; }
DEPLOYER=$("$STELLAR" keys public-key "$SOURCE")

ONE=100000000000000          # 1.0 at the guard's 14-decimal scale
DEPEG_PEG=105000000000000    # 1.05: live ~1.00 prices read ~4.8% below it

step() { printf '\n== %s\n' "$*"; }
ok() { printf '   ok: %s\n' "$*"; }
die() { printf '   FAIL: %s\n' "$*" >&2; exit 1; }

view() { # contract fn args...
  local id="$1"
  shift
  "$STELLAR" contract invoke --network "$NETWORK" --source-account "$SOURCE" --send=no --id "$id" -- "$@" 2>/dev/null | tr -d '"'
}

view_json() { # contract fn args... (quotes kept)
  local id="$1"
  shift
  "$STELLAR" contract invoke --network "$NETWORK" --source-account "$SOURCE" --send=no --id "$id" -- "$@" 2>/dev/null
}

# Submit as `who`; prints "<result>|<tx hash>".
submit() { # identity contract fn args...
  local who="$1" id="$2" errfile out hash
  shift 2
  errfile=$(mktemp)
  out=$("$STELLAR" contract invoke --network "$NETWORK" --source-account "$who" --send=yes --id "$id" -- "$@" 2>"$errfile") \
    || { cat "$errfile" >&2; rm -f "$errfile"; die "$*"; }
  hash=$(grep -oE '[0-9a-f]{64}' "$errfile" | tail -1)
  rm -f "$errfile"
  echo "${out}|${hash:-unknown}"
}
result_of() { echo "${1%|*}"; }
hash_of() { echo "${1##*|}"; }

# Simulate as `who` and require contract error `code`; nothing is submitted.
expect_error() { # code description identity contract fn args...
  local code="$1" what="$2" who="$3" id="$4" err
  shift 4
  err=$(mktemp)
  if "$STELLAR" contract invoke --network "$NETWORK" --source-account "$who" --send=no --id "$id" -- "$@" >/dev/null 2>"$err"; then
    rm -f "$err"
    die "$what succeeded but should have failed"
  fi
  grep -qE "Error\(Contract, #${code}\)" "$err" || { cat "$err" >&2; rm -f "$err"; die "$what failed for the wrong reason (expected #${code})"; }
  rm -f "$err"
  ok "$what rejected with #${code}"
}

# Submit signed only by `who`; the owner's auth entry stays unsigned, so the
# network must reject it. (`contract invoke` would sign for any local identity.)
expect_unsigned_auth_rejected() { # description identity contract fn args...
  local what="$1" who="$2" id="$3" tx err
  shift 3
  tx=$("$STELLAR" contract invoke --network "$NETWORK" --source-account "$who" --build-only --id "$id" -- "$@")
  tx=$("$STELLAR" tx simulate --network "$NETWORK" --source-account "$who" <<<"$tx")
  tx=$("$STELLAR" tx sign --network "$NETWORK" --sign-with-key "$who" <<<"$tx")
  REJECTED_HASH=$("$STELLAR" tx hash --network "$NETWORK" <<<"$tx")
  err=$(mktemp)
  if "$STELLAR" tx send --network "$NETWORK" <<<"$tx" >/dev/null 2>"$err"; then
    rm -f "$err"
    die "$what succeeded but should have failed"
  fi
  rm -f "$err"
  ok "$what rejected on-chain"
}

# Soroban orders addresses by their bytes; in strkey's base32 the digits 2-7
# come after Z, unlike ASCII.
sort_addresses() {
  python3 -I -c 'import sys; t=str.maketrans("234567","[\\]^_`"); print("\n".join(sorted(sys.argv[1:], key=lambda a: a.translate(t))))' "$@"
}

# Reflector asset each test token stands in for.
symbol_of() {
  case "$1" in
    "$SUSDC") echo USDC ;;
    "$SUSDT") echo USDT ;;
    *) die "no Reflector symbol for $1" ;;
  esac
}

guard_config() { # peg max_price_age
  local sources="" first=1 token
  for token in "${TOKENS[@]}"; do
    [[ $first == 1 ]] || sources+=","
    first=0
    sources+="{\"feed\":{\"oracle\":\"$REFLECTOR\",\"asset\":{\"Other\":\"$(symbol_of "$token")\"}},\"quotes\":[]}"
  done
  echo "{\"sources\":[${sources}],\"peg_price\":\"$1\",\"max_deviation_bps\":200,\"max_price_age\":$2,\"confirmations\":3}"
}

price_text() { awk -v v="$1" 'BEGIN { printf "%.6f", v / 1e14 }'; }

rows=()
record() { rows+=("| $1 | $2 | $3 | $4 | $5 |"); } # step caller action result hash

"$STELLAR" keys public-key "$KEEPER" >/dev/null 2>&1 || "$STELLAR" keys generate "$KEEPER" --network "$NETWORK" --fund >/dev/null
KEEPER_ADDR=$("$STELLAR" keys public-key "$KEEPER")
echo "Router:    $ROUTER"
echo "Governance: $DEPLOYER"
echo "Keeper:    $KEEPER_ADDR (outsider: no role anywhere)"
echo "Reflector: $REFLECTOR"

# ---------------------------------------------------------------------------
step "1. upgrade the router in place"
router_hash=$("$STELLAR" contract upload --network "$NETWORK" --source-account "$SOURCE" --wasm "$ROUTER_WASM" 2>/dev/null)
next_before=$(view "$ROUTER" next_pool_id)
r=$(submit "$SOURCE" "$ROUTER" upgrade --new_wasm_hash "$router_hash" --operator "$DEPLOYER")
[[ "$(view "$ROUTER" next_pool_id)" == "$next_before" ]] || die "registry changed across upgrade"
view "$ROUTER" get_depeg_guard --pool_id 0 >/dev/null || die "upgraded router lacks get_depeg_guard"
ok "router now runs $router_hash; $next_before pools still registered"
record 1 governance "\`upgrade\` router to \`${router_hash:0:12}…\`" "registry preserved ($next_before pools)" "\`$(hash_of "$r")\`"

# ---------------------------------------------------------------------------
step "2. demo pool priced by Reflector"
SUSDC=$(token_addr sUSDC)
SUSDT=$(token_addr sUSDT)
TOKENS=()
while IFS= read -r t; do TOKENS+=("$t"); done < <(sort_addresses "$SUSDC" "$SUSDT")
tokens_json=$(printf '"%s",' "${TOKENS[@]}")
tokens_json="[${tokens_json%,}]"

pool_id=$(view "$ROUTER" next_pool_id)
r=$(submit "$SOURCE" "$ROUTER" create_pool \
  --creator "$DEPLOYER" \
  --tokens "$tokens_json" \
  --amp_factor 100 \
  --amp_control ProtocolManaged \
  --swap_fee 100000 \
  --max_caps '["100000000000","100000000000"]' \
  --lp_max_supply 100000000000000 \
  --lp_name '"Spreadless Depeg Demo LP"' \
  --lp_symbol '"SPR-DPG"')
POOL=$(result_of "$r" | tr -d '"')
record 2 governance "\`create_pool\` sUSDC/sUSDT → id $pool_id" "\`$POOL\`" "\`$(hash_of "$r")\`"
for t in "${TOKENS[@]}"; do submit "$SOURCE" "$t" mint --to "$DEPLOYER" --amount 100000000000 >/dev/null; done
r=$(submit "$SOURCE" "$POOL" deposit --to "$DEPLOYER" --amounts_in '["10000000000","10000000000"]' --min_lp_out 0)
record 2 "pool owner" "\`deposit\` 1,000 of each token" "$(result_of "$r" | tr -d '"') LP minted" "\`$(hash_of "$r")\`"
ok "pool $POOL (id $pool_id) seeded"

oracle_rows=()
for t in "${TOKENS[@]}"; do
  lp=$(view_json "$REFLECTOR" lastprice --asset "{\"Other\":\"$(symbol_of "$t")\"}")
  price=$(jq -r '.price' <<<"$lp")
  ts=$(jq -r '.timestamp' <<<"$lp")
  oracle_rows+=("| \`$t\` | \`{\"Other\":\"$(symbol_of "$t")\"}\` | $(price_text "$price") | $ts |")
done

# ---------------------------------------------------------------------------
step "3. protect the pool; a healthy check by an outsider"
expect_error 7 "check_depeg on an unprotected pool" "$KEEPER" "$ROUTER" check_depeg --pool_id "$pool_id"
record 3 keeper "\`check_depeg\` before opting in" "rejected \`DepegGuardNotSet\` (#7), simulation only" "none"
r=$(submit "$SOURCE" "$ROUTER" set_depeg_guard --pool_id "$pool_id" --config "$(guard_config "$ONE" 900)")
record 3 governance "\`set_depeg_guard\`: peg 1.00, 200 bps, 900 s, 3 readings" "pool protected" "\`$(hash_of "$r")\`"
r=$(submit "$KEEPER" "$ROUTER" check_depeg --pool_id "$pool_id")
check=$(result_of "$r")
[[ "$(jq -r '.status' <<<"$check")" == Healthy ]] || die "expected Healthy, got $check"
[[ "$(view "$POOL" paused)" == false ]] || die "pool paused on healthy prices"
ok "Healthy: $check"
record 3 keeper "\`check_depeg\`" "\`Healthy\`, worst deviation $(jq -r '.deviation_bps' <<<"$check") bps" "\`$(hash_of "$r")\`"

sim=$("$STELLAR" contract invoke --network "$NETWORK" --source-account "$KEEPER" --build-only --id "$ROUTER" -- check_depeg --pool_id "$pool_id")
sim=$("$STELLAR" tx simulate --network "$NETWORK" --source-account "$KEEPER" <<<"$sim")
cost=$("$STELLAR" tx decode --output json <<<"$sim" | jq -r '.tx.tx.ext.v1 | "CPU instructions: \(.resources.instructions)\nResource fee:     \(.resource_fee) stroops"')

# ---------------------------------------------------------------------------
step "4. stale data is rejected, never acted on"
r=$(submit "$SOURCE" "$ROUTER" set_depeg_guard --pool_id "$pool_id" --config "$(guard_config "$DEPEG_PEG" 1)")
record 4 governance "\`set_depeg_guard\`: peg 1.05, max age 1 s" "config updated" "\`$(hash_of "$r")\`"
expect_error 9 "check_depeg with 1-second max age" "$KEEPER" "$ROUTER" check_depeg --pool_id "$pool_id"
[[ "$(view "$POOL" paused)" == false ]] || die "pool paused on stale data"
record 4 keeper "\`check_depeg\` (prices off-peg but older than 1 s)" "rejected \`StalePrice\` (#9); pool not paused" "none"

# ---------------------------------------------------------------------------
step "5. simulated ~5% depeg on live prices: an outsider pauses the pool"
r=$(submit "$SOURCE" "$ROUTER" set_depeg_guard --pool_id "$pool_id" --config "$(guard_config "$DEPEG_PEG" 900)")
record 5 governance "\`set_depeg_guard\`: peg 1.05, max age 900 s" "live ~1.00 prices now read ~4.8% off-peg" "\`$(hash_of "$r")\`"
r=$(submit "$KEEPER" "$ROUTER" check_depeg --pool_id "$pool_id")
check=$(result_of "$r")
[[ "$(jq -r '.status' <<<"$check")" == Paused ]] || die "expected Paused, got $check"
[[ "$(view "$POOL" is_protocol_paused)" == true ]] || die "pool not protocol-paused"
ok "Paused: $check"
record 5 keeper "\`check_depeg\`" "**\`Paused\`**: token \`$(jq -r '.token' <<<"$check")\`, price $(price_text "$(jq -r '.price' <<<"$check")"), deviation $(jq -r '.deviation_bps' <<<"$check") bps" "\`$(hash_of "$r")\`"

expect_error 1000 "swap while depeg-paused" "$SOURCE" "$POOL" swap_exact_in \
  --to "$DEPLOYER" --token_in "${TOKENS[0]}" --token_out "${TOKENS[1]}" --amount_in 1000000 --min_out 0
record 5 "pool owner" "\`swap_exact_in\`" "rejected \`EnforcedPause\` (#1000), simulation only" "none"
expect_error 1001 "pool owner unpause" "$SOURCE" "$POOL" unpause
record 5 "pool owner" "\`unpause\` on the pool" "rejected \`ExpectedPause\` (#1001): the owner cannot lift a protocol pause" "none"
expect_unsigned_auth_rejected "unpause_pool by the keeper" "$KEEPER" "$ROUTER" unpause_pool --pool_id "$pool_id"
record 5 keeper "\`unpause_pool\` on the router" "rejected on-chain (tx status FAILED): owner authorization missing" "\`$REJECTED_HASH\`"
r=$(submit "$SOURCE" "$POOL" withdraw --to "$DEPLOYER" --lp_amount 1000000000 --min_amounts_out '["0","0"]')
record 5 "pool owner" "\`withdraw\` while paused" "paid $(result_of "$r" | tr -d '\n ')" "\`$(hash_of "$r")\`"
ok "withdrawals stay open"

# ---------------------------------------------------------------------------
step "6. governance restores the peg and unpauses"
r=$(submit "$KEEPER" "$ROUTER" check_depeg --pool_id "$pool_id")
[[ "$(jq -r '.status' <<<"$(result_of "$r")")" == AlreadyPaused ]] || die "expected AlreadyPaused"
record 6 keeper "\`check_depeg\` again" "\`AlreadyPaused\` (no oracle read)" "\`$(hash_of "$r")\`"
r=$(submit "$SOURCE" "$ROUTER" set_depeg_guard --pool_id "$pool_id" --config "$(guard_config "$ONE" 900)")
record 6 governance "\`set_depeg_guard\`: peg back to 1.00" "config restored" "\`$(hash_of "$r")\`"
r=$(submit "$SOURCE" "$ROUTER" unpause_pool --pool_id "$pool_id")
[[ "$(view "$POOL" paused)" == false ]] || die "pool still paused"
record 6 governance "\`unpause_pool\`" "pool trading again" "\`$(hash_of "$r")\`"
r=$(submit "$SOURCE" "$POOL" swap_exact_in --to "$DEPLOYER" --token_in "${TOKENS[0]}" --token_out "${TOKENS[1]}" --amount_in 10000000 --min_out 0)
record 6 "pool owner" "\`swap_exact_in\` 1 token" "received $(result_of "$r" | tr -d '"') raw units" "\`$(hash_of "$r")\`"
r=$(submit "$KEEPER" "$ROUTER" check_depeg --pool_id "$pool_id")
[[ "$(jq -r '.status' <<<"$(result_of "$r")")" == Healthy ]] || die "expected Healthy after recovery"
record 6 keeper "\`check_depeg\`" "\`Healthy\`" "\`$(hash_of "$r")\`"
ok "recovered"

# ---------------------------------------------------------------------------
step "write $OUT"
{
  cat <<EOF
# Testnet depeg guard evidence

Generated by \`scripts/testnet-depeg-evidence.sh\` on $(date -u +%Y-%m-%dT%H:%M:%SZ)
from commit \`$(git rev-parse --short HEAD)\`. Every hash can be looked up on
[Stellar Expert (testnet)](https://stellar.expert/explorer/testnet).

## Setup

| | |
|---|---|
| Router (upgraded in place) | \`$ROUTER\` |
| Router WASM hash | \`$router_hash\` |
| Demo pool (id $pool_id) | \`$POOL\` |
| Governance (router owner, pool owner) | \`$DEPLOYER\` |
| Keeper (no role anywhere) | \`$KEEPER_ADDR\` |
| Oracle | Reflector testnet CEX & DEX feed \`$REFLECTOR\` (USD, 14 decimals, 5-minute updates) |
| Guard | 200 bps, 900 s max age, 3 confirmation readings |

Live Reflector prices when the run started:

| Pool token | Reflector asset | Price (USD) | Timestamp |
|---|---|---|---|
$(printf '%s\n' "${oracle_rows[@]}")

Testnet test tokens have no market of their own, so each is priced by the
Reflector feed of the asset it stands in for. sDAI is left out: Reflector's
testnet DAI feed had stopped updating at the time of the run, and
\`set_depeg_guard\` rejected a configuration using it (\`InvalidDepegGuard\`,
#8) because the feed had no current price. Real prices sit near 1.00, so to
show the pause firing on live data the run moves the configured peg to 1.05
(step 5): the unmodified oracle readings are then ~4.8% from the peg, past the
2% threshold on all three readings. Swaps never read the oracle.

## Steps

| Step | Caller | Action | Result | Transaction |
|---|---|---|---|---|
$(printf '%s\n' "${rows[@]}")

"Simulation only" rows were rejected during simulation, so nothing was
submitted. The keeper's \`unpause_pool\` was submitted signed only by the keeper
and failed on-chain.

## Cost of a check

Simulated \`check_depeg\` on the 2-token pool (2 feeds × 3 readings), reading
the real Reflector contract:

\`\`\`
$cost
\`\`\`
EOF
} >"$OUT"
ok "wrote $OUT"
step "depeg evidence run passed"
