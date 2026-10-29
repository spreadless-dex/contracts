#!/usr/bin/env bash
# Rehearse the mainnet governance flow against the testnet router recorded in
# deployments/testnet.json:
#
#   1. create a pool through the router, deposit, swap
#   2. protocol pause: router pauses a pool; the pool owner cannot lift it
#   3. amp ramp guards: too-short ramps are rejected; a valid ramp can be stopped
#   4. upgrades: router upgrades a pool, router upgrades itself, outsiders cannot
#   5. ownership: router -> classic multisig account (2-of-3), the multisig
#      accepts and administers, the old owner is locked out, a single
#      multisig signature is not enough, then ownership returns to the deployer
#
# Every step either succeeds or fails exactly as expected; anything else aborts.
set -euo pipefail

STELLAR="${STELLAR:-stellar}"
NETWORK="${NETWORK:-testnet}"
SOURCE="${SOURCE:-perps-testnet}"
DEPLOYMENTS_FILE="${DEPLOYMENTS_FILE:-deployments/testnet.json}"
TARGET_TRIPLE="${TARGET_TRIPLE:-wasm32v1-none}"
MULTISIG="${MULTISIG:-spreadless-rehearsal-multisig}"
SIGNERS=("${MULTISIG}-a" "${MULTISIG}-b" "${MULTISIG}-c")
OUTSIDER="${OUTSIDER:-spreadless-rehearsal-outsider}"

export STELLAR_NO_CACHE="${STELLAR_NO_CACHE:-true}"

if [[ "$NETWORK" != "testnet" ]]; then
  echo "ERROR: the rehearsal mints test tokens and creates throwaway keys; testnet only." >&2
  exit 1
fi

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

ROUTER=$(jq -r '.contracts.router.address' "$DEPLOYMENTS_FILE")
token_addr() { jq -r --arg l "$1" '.contracts.test_tokens[] | select(.label == $l) | .address' "$DEPLOYMENTS_FILE"; }
SDAI=$(token_addr sDAI)
SUSDT=$(token_addr sUSDT)
DEPLOYER=$("$STELLAR" keys public-key "$SOURCE")

step() { printf '\n== %s\n' "$*"; }
ok() { printf '   ok: %s\n' "$*"; }
die() { printf '   FAIL: %s\n' "$*" >&2; exit 1; }

invoke_as() { # identity contract fn args...
  local who="$1" id="$2"
  shift 2
  "$STELLAR" contract invoke --network "$NETWORK" --source-account "$who" --id "$id" -- "$@" 2>/dev/null | tr -d '"'
}
invoke() { invoke_as "$SOURCE" "$@"; }
view() {
  local id="$1"
  shift
  "$STELLAR" contract invoke --network "$NETWORK" --source-account "$SOURCE" --send=no --id "$id" -- "$@" 2>/dev/null | tr -d '"'
}

# Run a call that must fail with the given contract error code.
expect_error() { # code description identity contract fn args...
  local code="$1" what="$2" who="$3" id="$4" err
  shift 4
  err=$(mktemp)
  if "$STELLAR" contract invoke --network "$NETWORK" --source-account "$who" --id "$id" -- "$@" >/dev/null 2>"$err"; then
    rm -f "$err"
    die "$what succeeded but should have failed"
  fi
  if [[ "$code" == auth ]]; then
    grep -qE 'Error\(Auth|InvalidAction|not authorized|signature|Missing signing key' "$err" || { cat "$err" >&2; die "$what failed for the wrong reason"; }
  else
    grep -qE "Error\(Contract, #${code}\)" "$err" || { cat "$err" >&2; die "$what failed for the wrong reason (expected #${code})"; }
  fi
  rm -f "$err"
  ok "$what rejected"
}

# Submit a call signed only by the transaction source; any other address the
# contract requires auth from is left unsigned, so the network must reject it.
expect_unsigned_auth_rejected() { # description identity contract fn args...
  local what="$1" who="$2" id="$3" tx err
  shift 3
  tx=$("$STELLAR" contract invoke --network "$NETWORK" --source-account "$who" --build-only --id "$id" -- "$@")
  tx=$("$STELLAR" tx simulate --network "$NETWORK" --source-account "$who" <<<"$tx")
  tx=$("$STELLAR" tx sign --network "$NETWORK" --sign-with-key "$who" <<<"$tx")
  err=$(mktemp)
  if "$STELLAR" tx send --network "$NETWORK" <<<"$tx" >/dev/null 2>"$err"; then
    rm -f "$err"
    die "$what succeeded but should have failed"
  fi
  rm -f "$err"
  ok "$what rejected on-chain"
}

ensure_identity() {
  "$STELLAR" keys public-key "$1" >/dev/null 2>&1 || "$STELLAR" keys generate "$1" --network "$NETWORK" --fund >/dev/null
}

# Build an invocation with the multisig account as transaction source, sign it
# with the given signer identities, and submit it.
multisig_invoke() { # "signer ..." contract fn args...
  local signers="$1" id="$2" tx s
  shift 2
  tx=$("$STELLAR" contract invoke --network "$NETWORK" --source-account "$MULTISIG_ADDR" --build-only --id "$id" -- "$@")
  tx=$("$STELLAR" tx simulate --network "$NETWORK" --source-account "$MULTISIG_ADDR" <<<"$tx")
  for s in $signers; do
    tx=$("$STELLAR" tx sign --network "$NETWORK" --sign-with-key "$s" <<<"$tx")
  done
  "$STELLAR" tx send --network "$NETWORK" <<<"$tx" >/dev/null
}

wasm_hash_of() { "$STELLAR" contract upload --network "$NETWORK" --source-account "$SOURCE" --wasm "$1" 2>/dev/null; }
latest_ledger() { "$STELLAR" ledger latest --network "$NETWORK" --output json 2>/dev/null | jq -r '.sequence'; }

echo "Router:   $ROUTER"
echo "Deployer: $DEPLOYER"

# ---------------------------------------------------------------------------
step "1. create pool, deposit, swap"
pool_id=$(view "$ROUTER" next_pool_id)
pool=$(invoke "$ROUTER" create_pool \
  --creator "$DEPLOYER" \
  --tokens "[\"$SDAI\",\"$SUSDT\"]" \
  --amp_factor 100 \
  --amp_control ProtocolManaged \
  --swap_fee 100000 \
  --max_caps '["100000000000","100000000000"]' \
  --lp_max_supply 100000000000000 \
  --lp_name '"Spreadless Rehearsal LP"' \
  --lp_symbol '"SPR-REH"')
[[ "$(view "$ROUTER" pool_at --id "$pool_id")" == "$pool" ]] || die "pool not registered at id $pool_id"
ok "pool $pool registered as id $pool_id"

for t in "$SDAI" "$SUSDT"; do invoke "$t" mint --to "$DEPLOYER" --amount 200000000000 >/dev/null; done
lp=$(invoke "$pool" deposit --to "$DEPLOYER" --amounts_in '["10000000000","10000000000"]' --min_lp_out 0)
ok "deposited 1000/1000, minted $lp LP"

expect_error 15 "deposit above token cap" "$SOURCE" "$pool" deposit \
  --to "$DEPLOYER" --amounts_in '["100000000000","0"]' --min_lp_out 0

quote=$(view "$pool" swap_exact_in --to "$DEPLOYER" --token_in "$SDAI" --token_out "$SUSDT" --amount_in 100000000 --min_out 0)
out=$(invoke "$pool" swap_exact_in --to "$DEPLOYER" --token_in "$SDAI" --token_out "$SUSDT" --amount_in 100000000 --min_out "$quote")
[[ "$out" == "$quote" ]] || die "swap out $out != quote $quote"
ok "swap 10 sDAI -> $out sUSDT (exactly the quote)"

# ---------------------------------------------------------------------------
step "2. protocol pause"
invoke "$ROUTER" pause_pool --pool_id "$pool_id" >/dev/null
[[ "$(view "$pool" is_protocol_paused)" == true ]] || die "pool not protocol-paused"
expect_error 1000 "swap while protocol-paused" "$SOURCE" "$pool" swap_exact_in \
  --to "$DEPLOYER" --token_in "$SDAI" --token_out "$SUSDT" --amount_in 1000000 --min_out 0
invoke "$pool" unpause >/dev/null 2>&1 || true
[[ "$(view "$pool" is_protocol_paused)" == true ]] || die "pool owner lifted the protocol pause"
ok "pool owner unpause does not lift the protocol pause"
invoke "$ROUTER" unpause_pool --pool_id "$pool_id" >/dev/null
[[ "$(view "$pool" paused)" == false ]] || die "pool still paused"
ok "router unpause restores trading"

# ---------------------------------------------------------------------------
step "3. amp ramp guards"
expect_error 23 "1-hour amp ramp" "$SOURCE" "$ROUTER" set_pool_amp_ramp \
  --pool_id "$pool_id" --target_factor 200 --duration 3600
expect_error 25 "100x amp change" "$SOURCE" "$ROUTER" set_pool_amp_ramp \
  --pool_id "$pool_id" --target_factor 10000 --duration 86400
invoke "$ROUTER" set_pool_amp_ramp --pool_id "$pool_id" --target_factor 200 --duration 86400 >/dev/null
invoke "$ROUTER" stop_pool_amp_ramp --pool_id "$pool_id" >/dev/null
ok "valid 1-day ramp started and stopped (amp now $(view "$pool" get_amp))"

# ---------------------------------------------------------------------------
step "4. upgrades"
ensure_identity "$OUTSIDER"
outsider_addr=$("$STELLAR" keys public-key "$OUTSIDER")
pool_hash=$(view "$ROUTER" get_pool_wasm_hash)
router_hash=$(wasm_hash_of "target/${TARGET_TRIPLE}/release/spreadless_router.wasm")
reserves_before=$(view "$pool" get_reserves)

expect_error 26 "pool upgrade by a non-controller operator" "$OUTSIDER" "$pool" upgrade \
  --new_wasm_hash "$pool_hash" --operator "$outsider_addr"
expect_error auth "upgrade_pool by an outsider" "$OUTSIDER" "$ROUTER" upgrade_pool \
  --pool_id "$pool_id" --new_wasm_hash "$pool_hash"
expect_error 6 "router upgrade naming an outsider as operator" "$OUTSIDER" "$ROUTER" upgrade \
  --new_wasm_hash "$router_hash" --operator "$outsider_addr"
# `contract invoke` would sign the owner's auth entry itself whenever the owner
# key is a local identity, so build the transaction and sign it as the outsider only.
expect_unsigned_auth_rejected "router upgrade by an outsider naming the owner" "$OUTSIDER" "$ROUTER" upgrade \
  --new_wasm_hash "$router_hash" --operator "$DEPLOYER"

invoke "$ROUTER" upgrade_pool --pool_id "$pool_id" --new_wasm_hash "$pool_hash" >/dev/null
[[ "$(view "$pool" get_reserves)" == "$reserves_before" ]] || die "pool state changed across upgrade"
ok "router upgraded pool $pool_id in place, state preserved"
invoke "$ROUTER" upgrade --new_wasm_hash "$router_hash" --operator "$DEPLOYER" >/dev/null
[[ "$(view "$ROUTER" pool_at --id "$pool_id")" == "$pool" ]] || die "router registry lost across upgrade"
ok "router upgraded itself in place, registry preserved"

# ---------------------------------------------------------------------------
step "5. ownership to a 2-of-3 classic multisig and back"
ensure_identity "$MULTISIG"
for s in "${SIGNERS[@]}"; do ensure_identity "$s"; done
MULTISIG_ADDR=$("$STELLAR" keys public-key "$MULTISIG")

if [[ "$(curl -s "https://horizon-testnet.stellar.org/accounts/$MULTISIG_ADDR" | jq '.signers | length')" -lt 4 ]]; then
  for s in "${SIGNERS[@]}"; do
    "$STELLAR" tx new set-options --network "$NETWORK" --source-account "$MULTISIG" \
      --signer "$("$STELLAR" keys public-key "$s")" --signer-weight 1 >/dev/null
  done
  "$STELLAR" tx new set-options --network "$NETWORK" --source-account "$MULTISIG" \
    --master-weight 0 --low-threshold 2 --med-threshold 2 --high-threshold 2 >/dev/null
fi
ok "multisig $MULTISIG_ADDR: 3 signers, threshold 2, master key disabled"

live_until=$(($(latest_ledger) + 17280)) # ~1 day
invoke "$ROUTER" transfer_ownership --new_owner "$MULTISIG_ADDR" --live_until_ledger "$live_until" >/dev/null
[[ "$(view "$ROUTER" get_owner)" == "$DEPLOYER" ]] || die "owner changed before acceptance"
ok "transfer proposed; deployer still owner until accepted"

if multisig_invoke "${SIGNERS[0]}" "$ROUTER" accept_ownership 2>/dev/null; then
  die "accept_ownership with 1 of 3 signatures succeeded"
fi
ok "accept with a single signature rejected"
multisig_invoke "${SIGNERS[0]} ${SIGNERS[1]}" "$ROUTER" accept_ownership
[[ "$(view "$ROUTER" get_owner)" == "$MULTISIG_ADDR" ]] || die "multisig is not owner"
ok "multisig accepted with 2 of 3 signatures"

expect_error auth "pause_pool by the old owner" "$SOURCE" "$ROUTER" pause_pool --pool_id "$pool_id"
multisig_invoke "${SIGNERS[1]} ${SIGNERS[2]}" "$ROUTER" pause_pool --pool_id "$pool_id"
[[ "$(view "$pool" is_protocol_paused)" == true ]] || die "multisig pause had no effect"
multisig_invoke "${SIGNERS[0]} ${SIGNERS[2]}" "$ROUTER" unpause_pool --pool_id "$pool_id"
[[ "$(view "$pool" is_protocol_paused)" == false ]] || die "multisig unpause had no effect"
ok "multisig paused and unpaused pool $pool_id"
multisig_invoke "${SIGNERS[0]} ${SIGNERS[1]}" "$ROUTER" upgrade_pool --pool_id "$pool_id" --new_wasm_hash "$pool_hash"
ok "multisig upgraded pool $pool_id"

live_until=$(($(latest_ledger) + 17280))
multisig_invoke "${SIGNERS[0]} ${SIGNERS[1]}" "$ROUTER" transfer_ownership \
  --new_owner "$DEPLOYER" --live_until_ledger "$live_until"
invoke "$ROUTER" accept_ownership >/dev/null
[[ "$(view "$ROUTER" get_owner)" == "$DEPLOYER" ]] || die "ownership did not return to the deployer"
ok "ownership returned to the deployer"

step "rehearsal passed"
