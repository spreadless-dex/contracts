#!/usr/bin/env bash
set -euo pipefail

STELLAR="${STELLAR:-stellar}"
NETWORK="${NETWORK:-testnet}"
SOURCE="${SOURCE:-default}"
SAC_ISSUER="${SAC_ISSUER:-spreadless-${NETWORK}-asset-issuer}"
RUST_VERSION="${RUST_VERSION:-1.92.0}"
TARGET_TRIPLE="${TARGET_TRIPLE:-wasm32v1-none}"
TOKEN_CATALOG="${TOKEN_CATALOG:-config/tokens.json}"
DEPLOYMENTS_FILE="${DEPLOYMENTS_FILE:-deployments/${NETWORK}.json}"
INITIAL_BALANCE="${INITIAL_BALANCE:-1000000000000}"

require_cmd() {
  command -v "$1" >/dev/null 2>&1 || {
    echo "ERROR: missing required command: $1" >&2
    exit 1
  }
}

if [[ "$NETWORK" != "testnet" ]]; then
  echo "ERROR: deploy-tokens.sh creates open-mint and self-issued branded assets." >&2
  echo "It is intentionally restricted to NETWORK=testnet." >&2
  exit 1
fi

require_cmd "$STELLAR"
require_cmd jq
require_cmd rustup

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

jq -e '
  (.decimals | type == "number") and
  (.tokens | length > 0) and
  ([.tokens[].id] | length == (unique | length)) and
  ([.tokens[].symbol] | length == (unique | length)) and
  all(.tokens[]; (.symbol | test("^[A-Za-z0-9]{1,12}$")))
' "$TOKEN_CATALOG" >/dev/null || {
  echo "ERROR: invalid token catalog: ${TOKEN_CATALOG}" >&2
  exit 1
}

test_token_wasm="target/${TARGET_TRIPLE}/release/test_token.wasm"
echo "Building the optimized Soroban token contract..."
rustup run "$RUST_VERSION" "$STELLAR" contract build --package test-token --optimize

holder="$("$STELLAR" keys public-key "$SOURCE")"
if ! issuer="$("$STELLAR" keys public-key "$SAC_ISSUER" 2>/dev/null)"; then
  echo "Creating and funding SAC issuer identity ${SAC_ISSUER}..."
  "$STELLAR" keys generate "$SAC_ISSUER" --network "$NETWORK" --fund
  issuer="$("$STELLAR" keys public-key "$SAC_ISSUER")"
fi

mkdir -p "$(dirname "$DEPLOYMENTS_FILE")"
if [[ ! -f "$DEPLOYMENTS_FILE" ]]; then
  jq -n \
    --arg network "$NETWORK" \
    --arg source "$SOURCE" \
    --arg deployer "$holder" \
    '{network: $network, source_identity: $source, deployer: $deployer, contracts: {tokens: []}}' \
    > "$DEPLOYMENTS_FILE"
fi

manifest_network="$(jq -r '.network // empty' "$DEPLOYMENTS_FILE")"
if [[ -n "$manifest_network" && "$manifest_network" != "$NETWORK" ]]; then
  echo "ERROR: ${DEPLOYMENTS_FILE} belongs to ${manifest_network}, not ${NETWORK}" >&2
  exit 1
fi

write_token() {
  local token_json="$1"
  local id
  local temp_file
  id="$(jq -r '.id' <<<"$token_json")"
  temp_file="$(mktemp "$(dirname "$DEPLOYMENTS_FILE")/.tokens.XXXXXX")"
  jq --arg id "$id" --argjson token "$token_json" '
    .contracts = (.contracts // {})
    | .contracts.tokens = ((.contracts.tokens // []) | map(select(.id != $id)) + [$token])
    | .contracts.tokens |= sort_by(.id)
  ' "$DEPLOYMENTS_FILE" > "$temp_file"
  mv "$temp_file" "$DEPLOYMENTS_FILE"
}

token_definitions=()
while IFS= read -r definition; do
  token_definitions+=("$definition")
done < <(jq -c '.tokens[]' "$TOKEN_CATALOG")

for definition in "${token_definitions[@]}"; do
  id="$(jq -r '.id' <<<"$definition")"
  category="$(jq -r '.category' <<<"$definition")"
  name="$(jq -r '.name' <<<"$definition")"
  symbol="$(jq -r '.symbol' <<<"$definition")"
  configured_sac_asset="$(jq -r '.sac_asset // empty' <<<"$definition")"
  decimals="$(jq -r '.decimals' "$TOKEN_CATALOG")"

  current="$(jq -c --arg id "$id" '.contracts.tokens[]? | select(.id == $id)' "$DEPLOYMENTS_FILE")"
  if [[ -z "$current" ]]; then
    current='{}'
  fi
  soroban_address="$(jq -r '.soroban.address // empty' <<<"$current")"
  soroban_balance="$(jq -r '.soroban.initial_balance // empty' <<<"$current")"

  if [[ -z "$soroban_address" ]]; then
    echo "Deploying Soroban token ${symbol}..."
    soroban_address="$("$STELLAR" contract deploy \
      --network "$NETWORK" \
      --source-account "$SOURCE" \
      --wasm "$test_token_wasm" \
      -- \
      --decimals "$decimals" \
      --name "$name" \
      --symbol "$symbol")"
    current="$(jq -n \
      --arg id "$id" --arg category "$category" --arg name "$name" --arg symbol "$symbol" \
      --arg address "$soroban_address" --argjson decimals "$decimals" \
      '{id: $id, category: $category, name: $name, symbol: $symbol, decimals: $decimals,
        soroban: {address: $address, kind: "custom_soroban_token", open_mint: true}, sac: null}')"
    write_token "$current"
  fi

  if [[ -z "$soroban_balance" ]]; then
    echo "Minting ${symbol} Soroban balance to ${holder}..."
    "$STELLAR" contract invoke \
      --network "$NETWORK" \
      --source-account "$SOURCE" \
      --id "$soroban_address" \
      -- mint --to "$holder" --amount "$INITIAL_BALANCE" >/dev/null
    current="$(jq --arg amount "$INITIAL_BALANCE" '.soroban.initial_balance = $amount' <<<"$current")"
    write_token "$current"
  fi

  current="$(jq -c --arg id "$id" '.contracts.tokens[] | select(.id == $id)' "$DEPLOYMENTS_FILE")"
  sac_address="$(jq -r '.sac.address // empty' <<<"$current")"
  sac_balance="$(jq -r '.sac.initial_balance // empty' <<<"$current")"
  asset="${configured_sac_asset:-${symbol}:${issuer}}"

  if [[ -z "$sac_address" ]]; then
    if [[ "$asset" == "native" ]]; then
      echo "Resolving the network-native XLM SAC..."
      sac_address="$("$STELLAR" contract id asset --network "$NETWORK" --asset native)"
      current="$(jq \
        --arg address "$sac_address" \
        '.sac = {address: $address, asset: "native", issuer: null,
          issuer_identity: null, kind: "native_stellar_asset_with_sac", initial_balance: "network-funded"}' <<<"$current")"
    else
      echo "Creating ${symbol} trustline for ${holder}..."
      "$STELLAR" tx new change-trust \
        --network "$NETWORK" \
        --source-account "$SOURCE" \
        --line "$asset" >/dev/null

      echo "Deploying SAC wrapper for ${asset}..."
      sac_address="$("$STELLAR" contract asset deploy \
        --network "$NETWORK" \
        --source-account "$SOURCE" \
        --asset "$asset")"
      current="$(jq \
        --arg address "$sac_address" --arg asset "$asset" --arg issuer "$issuer" --arg issuer_identity "$SAC_ISSUER" \
        '.sac = {address: $address, asset: $asset, issuer: $issuer,
          issuer_identity: $issuer_identity, kind: "stellar_classic_asset_with_sac"}' <<<"$current")"
    fi
    write_token "$current"
  fi

  if [[ "$asset" != "native" && -z "$sac_balance" ]]; then
    echo "Issuing ${symbol} SAC balance to ${holder}..."
    "$STELLAR" tx new payment \
      --network "$NETWORK" \
      --source-account "$SAC_ISSUER" \
      --destination "$holder" \
      --asset "$asset" \
      --amount "$INITIAL_BALANCE" >/dev/null
    current="$(jq --arg amount "$INITIAL_BALANCE" '.sac.initial_balance = $amount' <<<"$current")"
    write_token "$current"
  fi

  echo "Available: ${symbol} (Soroban ${soroban_address}, SAC ${sac_address})"
done

temp_file="$(mktemp "$(dirname "$DEPLOYMENTS_FILE")/.deployment.XXXXXX")"
jq \
  --arg deployed_at "$(date -u +"%Y-%m-%dT%H:%M:%SZ")" \
  --arg catalog "$TOKEN_CATALOG" \
  --arg holder "$holder" \
  --arg issuer "$issuer" \
  '.deployed_at = $deployed_at
   | .contracts.token_catalog = {source: $catalog, holder: $holder, sac_issuer: $issuer}' \
  "$DEPLOYMENTS_FILE" > "$temp_file"
mv "$temp_file" "$DEPLOYMENTS_FILE"

echo "Saved token addresses to ${DEPLOYMENTS_FILE}"
