#!/usr/bin/env bash
set -euo pipefail

STELLAR="${STELLAR:-stellar}"
NETWORK="${NETWORK:-testnet}"
SOURCE="${SOURCE:-default}"
RUST_VERSION="${RUST_VERSION:-1.92.0}"
TARGET_TRIPLE="${TARGET_TRIPLE:-wasm32v1-none}"
DEPLOYMENTS_FILE="${DEPLOYMENTS_FILE:-deployments/testnet.json}"

TOKEN_DECIMALS="${TOKEN_DECIMALS:-7}"
TOKEN_A_NAME="${TOKEN_A_NAME:-Spreadless Test USDC}"
TOKEN_A_SYMBOL="${TOKEN_A_SYMBOL:-sUSDC}"
TOKEN_B_NAME="${TOKEN_B_NAME:-Spreadless Test USDT}"
TOKEN_B_SYMBOL="${TOKEN_B_SYMBOL:-sUSDT}"
TOKEN_C_NAME="${TOKEN_C_NAME:-Spreadless Test DAI}"
TOKEN_C_SYMBOL="${TOKEN_C_SYMBOL:-sDAI}"

AMP_FACTOR="${AMP_FACTOR:-100}"
SWAP_FEE="${SWAP_FEE:-100000}"
PROTOCOL_FEE="${PROTOCOL_FEE:-0}"
MAX_CAP="${MAX_CAP:-30000000000000000}"
LP_MAX_SUPPLY="${LP_MAX_SUPPLY:-3000000000000000000}"
INITIAL_MINT="${INITIAL_MINT:-1000000000000}"
INITIAL_DEPOSIT="${INITIAL_DEPOSIT:-1000000000000}"

export STELLAR_NO_CACHE="${STELLAR_NO_CACHE:-true}"

require_cmd() {
  command -v "$1" >/dev/null 2>&1 || {
    echo "ERROR: missing required command: $1" >&2
    exit 1
  }
}

deploy_contract() {
  local wasm="$1"
  shift
  "$STELLAR" contract deploy \
    --network "$NETWORK" \
    --source-account "$SOURCE" \
    --wasm "$wasm" \
    -- "$@"
}

invoke_contract() {
  local contract_id="$1"
  shift
  "$STELLAR" contract invoke \
    --network "$NETWORK" \
    --source-account "$SOURCE" \
    --id "$contract_id" \
    -- "$@"
}

sort_contract_addresses() {
  python3 - "$@" <<'PY'
import base64
import sys

def payload(address: str) -> bytes:
    padded = address + "=" * ((8 - len(address) % 8) % 8)
    decoded = base64.b32decode(padded)
    return decoded[1:-2]

print(*sorted(sys.argv[1:], key=payload))
PY
}

require_cmd "$STELLAR"
require_cmd jq
require_cmd python3
require_cmd rustup

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

liquidity_pool_wasm="target/${TARGET_TRIPLE}/release/liquidity_pool.wasm"
test_token_wasm="target/${TARGET_TRIPLE}/release/test_token.wasm"

echo "Building contracts..."
rustup run "$RUST_VERSION" "$STELLAR" contract build --package test-token --optimize
rustup run "$RUST_VERSION" "$STELLAR" contract build --package liquidity-pool --optimize

owner="${OWNER:-$("$STELLAR" keys public-key "$SOURCE")}"
beneficiary="${BENEFICIARY:-$owner}"

echo "Deploying open-mint test tokens to ${NETWORK}..."
if [[ -n "${TOKEN_A_ADDRESS:-}" && -n "${TOKEN_B_ADDRESS:-}" && -n "${TOKEN_C_ADDRESS:-}" ]]; then
  token_a="$TOKEN_A_ADDRESS"
  token_b="$TOKEN_B_ADDRESS"
  token_c="$TOKEN_C_ADDRESS"
  echo "Reusing TOKEN_A_ADDRESS=${token_a}"
  echo "Reusing TOKEN_B_ADDRESS=${token_b}"
  echo "Reusing TOKEN_C_ADDRESS=${token_c}"
else
  token_a="$(deploy_contract "$test_token_wasm" \
    --decimals "$TOKEN_DECIMALS" \
    --name "$TOKEN_A_NAME" \
    --symbol "$TOKEN_A_SYMBOL")"
  token_b="$(deploy_contract "$test_token_wasm" \
    --decimals "$TOKEN_DECIMALS" \
    --name "$TOKEN_B_NAME" \
    --symbol "$TOKEN_B_SYMBOL")"
  token_c="$(deploy_contract "$test_token_wasm" \
    --decimals "$TOKEN_DECIMALS" \
    --name "$TOKEN_C_NAME" \
    --symbol "$TOKEN_C_SYMBOL")"
fi

read -r token_0 token_1 token_2 < <(sort_contract_addresses "$token_a" "$token_b" "$token_c")

echo "Deploying liquidity pool to ${NETWORK}..."
pool="$(deploy_contract "$liquidity_pool_wasm" \
  --owner "$owner" \
  --tokens "[\"${token_0}\",\"${token_1}\",\"${token_2}\"]" \
  --amp_factor "$AMP_FACTOR" \
  --swap_fee "$SWAP_FEE" \
  --protocol_fee "$PROTOCOL_FEE" \
  --beneficiary "$beneficiary" \
  --max_caps "[\"${MAX_CAP}\",\"${MAX_CAP}\",\"${MAX_CAP}\"]" \
  --lp_max_supply "$LP_MAX_SUPPLY")"

echo "Minting initial token balances to ${owner}..."
invoke_contract "$token_a" mint --to "$owner" --amount "$INITIAL_MINT" >/dev/null
invoke_contract "$token_b" mint --to "$owner" --amount "$INITIAL_MINT" >/dev/null
invoke_contract "$token_c" mint --to "$owner" --amount "$INITIAL_MINT" >/dev/null

echo "Seeding initial pool liquidity..."
deposit_result="$(invoke_contract "$pool" deposit \
  --to "$owner" \
  --amounts_in "[\"${INITIAL_DEPOSIT}\",\"${INITIAL_DEPOSIT}\",\"${INITIAL_DEPOSIT}\"]" \
  --min_lp_out 0)"
deposit_result="$(printf '%s\n' "$deposit_result" | jq -r . 2>/dev/null || printf '%s\n' "$deposit_result")"

mkdir -p "$(dirname "$DEPLOYMENTS_FILE")"

jq -n \
  --arg network "$NETWORK" \
  --arg deployed_at "$(date -u +"%Y-%m-%dT%H:%M:%SZ")" \
  --arg source "$SOURCE" \
  --arg deployer "$owner" \
  --arg beneficiary "$beneficiary" \
  --arg token_a "$token_a" \
  --arg token_a_name "$TOKEN_A_NAME" \
  --arg token_a_symbol "$TOKEN_A_SYMBOL" \
  --arg token_b "$token_b" \
  --arg token_b_name "$TOKEN_B_NAME" \
  --arg token_b_symbol "$TOKEN_B_SYMBOL" \
  --arg token_c "$token_c" \
  --arg token_c_name "$TOKEN_C_NAME" \
  --arg token_c_symbol "$TOKEN_C_SYMBOL" \
  --arg token_0 "$token_0" \
  --arg token_1 "$token_1" \
  --arg token_2 "$token_2" \
  --arg pool "$pool" \
  --arg deposit_result "$deposit_result" \
  --argjson decimals "$TOKEN_DECIMALS" \
  --argjson amp_factor "$AMP_FACTOR" \
  --argjson swap_fee "$SWAP_FEE" \
  --argjson protocol_fee "$PROTOCOL_FEE" \
  --arg max_cap "$MAX_CAP" \
  --arg lp_max_supply "$LP_MAX_SUPPLY" \
  --arg initial_mint "$INITIAL_MINT" \
  --arg initial_deposit "$INITIAL_DEPOSIT" \
  '{
    network: $network,
    network_passphrase: "Test SDF Network ; September 2015",
    deployed_at: $deployed_at,
    source_identity: $source,
    deployer: $deployer,
    contracts: {
      test_tokens: [
        {
          label: "sUSDC",
          address: $token_a,
          name: $token_a_name,
          symbol: $token_a_symbol,
          decimals: $decimals,
          open_mint: true,
          supply_cap: null
        },
        {
          label: "sUSDT",
          address: $token_b,
          name: $token_b_name,
          symbol: $token_b_symbol,
          decimals: $decimals,
          open_mint: true,
          supply_cap: null
        },
        {
          label: "sDAI",
          address: $token_c,
          name: $token_c_name,
          symbol: $token_c_symbol,
          decimals: $decimals,
          open_mint: true,
          supply_cap: null
        }
      ],
      liquidity_pool: {
        label: "3-token-usd-pool",
        address: $pool,
        owner: $deployer,
        beneficiary: $beneficiary,
        tokens: [$token_0, $token_1, $token_2],
        amp_factor: $amp_factor,
        swap_fee: $swap_fee,
        protocol_fee: $protocol_fee,
        max_caps: [$max_cap, $max_cap, $max_cap],
        lp_max_supply: $lp_max_supply
      }
    },
    initial_liquidity: {
      provider: $deployer,
      minted_each_token: $initial_mint,
      deposited_each_token: $initial_deposit,
      deposit_result: $deposit_result
    },
    notes: [
      "The test token mint(to, amount) entrypoint is intentionally open and uncapped.",
      "Pool token order is canonical and may differ from the display labels above."
    ]
  }' > "$DEPLOYMENTS_FILE"

echo "Saved deployment addresses to ${DEPLOYMENTS_FILE}"
echo "Token A (${TOKEN_A_SYMBOL}): ${token_a}"
echo "Token B (${TOKEN_B_SYMBOL}): ${token_b}"
echo "Token C (${TOKEN_C_SYMBOL}): ${token_c}"
echo "Pool: ${pool}"
