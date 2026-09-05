#!/usr/bin/env bash
set -euo pipefail

STELLAR="${STELLAR:-stellar}"
NETWORK="${NETWORK:-testnet}"
SOURCE="${SOURCE:-default}"
RUST_VERSION="${RUST_VERSION:-1.92.0}"
TARGET_TRIPLE="${TARGET_TRIPLE:-wasm32v1-none}"
DEPLOYMENTS_FILE="${DEPLOYMENTS_FILE:-deployments/${NETWORK}.json}"
PROTOCOL_FEE="${PROTOCOL_FEE:-330000000}"
SWAP_FEE="${SWAP_FEE:-10000000}"

require_cmd() {
  command -v "$1" >/dev/null 2>&1 || {
    echo "ERROR: missing required command: $1" >&2
    exit 1
  }
}

require_cmd "$STELLAR"
require_cmd jq
require_cmd rustup

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

pool_wasm="target/${TARGET_TRIPLE}/release/spreadless_pool.wasm"
router_wasm="target/${TARGET_TRIPLE}/release/spreadless_router.wasm"

echo "Building optimized pool and router contracts..."
rustup run "$RUST_VERSION" "$STELLAR" contract build --package spreadless-pool --optimize
rustup run "$RUST_VERSION" "$STELLAR" contract build --package spreadless-router --optimize

owner="${OWNER:-$("$STELLAR" keys public-key "$SOURCE")}"
beneficiary="${BENEFICIARY:-$owner}"

echo "Uploading the pool WASM to ${NETWORK}..."
pool_wasm_hash="$("$STELLAR" contract upload \
  --network "$NETWORK" \
  --source-account "$SOURCE" \
  --wasm "$pool_wasm")"

echo "Deploying the router with protocol defaults..."
router="$("$STELLAR" contract deploy \
  --network "$NETWORK" \
  --source-account "$SOURCE" \
  --wasm "$router_wasm" \
  -- \
  --owner "$owner" \
  --pool_wasm_hash "$pool_wasm_hash" \
  --default_protocol_fee "$PROTOCOL_FEE" \
  --default_protocol_beneficiary "$beneficiary")"

mkdir -p "$(dirname "$DEPLOYMENTS_FILE")"
existing='{}'
if [[ -f "$DEPLOYMENTS_FILE" ]]; then
  existing="$(<"$DEPLOYMENTS_FILE")"
fi

updated="$(jq \
  --arg network "$NETWORK" \
  --arg deployed_at "$(date -u +"%Y-%m-%dT%H:%M:%SZ")" \
  --arg source "$SOURCE" \
  --arg deployer "$owner" \
  --arg router "$router" \
  --arg pool_wasm_hash "$pool_wasm_hash" \
  --arg beneficiary "$beneficiary" \
  --argjson protocol_fee "$PROTOCOL_FEE" \
  --argjson swap_fee "$SWAP_FEE" \
  '.network = $network
   | .deployed_at = $deployed_at
   | .source_identity = $source
   | .deployer = $deployer
   | .contracts = (.contracts // {})
   | .contracts.router = {
       address: $router,
       owner: $deployer,
       pool_wasm_hash: $pool_wasm_hash,
       default_protocol_fee: $protocol_fee,
       default_protocol_beneficiary: $beneficiary,
       permissionless_creation: true
     }
   | .pool_defaults = ((.pool_defaults // {}) + {swap_fee: $swap_fee})' <<<"$existing")"

printf '%s\n' "$updated" > "$DEPLOYMENTS_FILE"

echo "Saved router deployment to ${DEPLOYMENTS_FILE}"
echo "Pool WASM hash: ${pool_wasm_hash}"
echo "Router: ${router}"
