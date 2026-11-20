#!/usr/bin/env bash
# Upgrade the mainnet router in place to the depeg-guard build and protect the
# USD 3pool (pool 0). Every signature is requested from the Ledger behind the
# `spreadless-mainnet` identity.
#
#   DRY_RUN=1 scripts/mainnet-depeg-upgrade.sh   # preflight only, signs nothing
#   scripts/mainnet-depeg-upgrade.sh             # preflight, then 3 signatures:
#                                                # upload WASM, upgrade, set guard
#
# Preflight (read-only) aborts unless:
#   - the local build is byte-identical to the WASM demonstrated on testnet
#   - the mainnet router runs the expected previous code and is owned by the Ledger
#   - pool 0 holds exactly USDT0, PYUSD and USDC
#   - every Reflector feed answers with 3 current, aligned readings and the
#     guard, evaluated off-chain exactly like `check_depeg`, would report Healthy
#
# The script is resumable: steps already done on-chain are detected and skipped.
set -euo pipefail

STELLAR="${STELLAR:-stellar}"
NETWORK=mainnet
SOURCE="${SOURCE:-spreadless-mainnet}"
INCLUSION_FEE="${INCLUSION_FEE:-10000}"
DEPLOYMENTS_FILE=deployments/mainnet.json
ROUTER_WASM=target/wasm32v1-none/release/spreadless_router.wasm

# The router build demonstrated on testnet (docs/testnet-depeg-guard-evidence.md).
NEW_ROUTER_HASH=a1bb2f7afb71db36706293c0a3839418f334d9f321e14e2c98a8dede14a76f1d
OLD_ROUTER_HASH=aec9b485d8f2132c143ae64590367591d2bda73cf1c4dc14bb5972550f4f9ef2

# Reflector mainnet feeds (both 14 decimals, 5-minute updates).
REFLECTOR_USD=CAFJZQWSED6YAWZU3GWRTOCNPPCGBN32L7QV43XX5LZLFTK6JLN34DLN # External CEX & DEX, base USD
REFLECTOR_DEX=CALI2BYU2JE6WVRUFYTS6MSBNEHGJ35P4AVCZYF3B6QOE3QKOB2PLE6M # Stellar DEX, base USDC
USDT0=CBSJZEIO5C7KC2SF3MKSNXXJSW5G3VTNBX4ATMKUI3B2MR4JKM4R26YF
PYUSD=CCCRWH6Q3FNP3I2I57BDLM5AFAT7O6OF6GKQOC6SSJNDAVRZ57SPHGU2
USDC=CCW67TSZV3SSS2HXMBQ5JFGCKJNXKZM7UQUWUZPUTHXSTZLEO7SJMI75

POOL_ID=0
PEG_PRICE=100000000000000 # 1.00 USD at 14 decimals
MAX_DEVIATION_BPS=200
MAX_PRICE_AGE=900
CONFIRMATIONS=3

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"
export STELLAR_NO_CACHE=true

step() { printf '\n== %s\n' "$*"; }
ok() { printf '   ok: %s\n' "$*"; }
die() { printf '   ABORT: %s\n' "$*" >&2; exit 1; }

# Ask for a typed confirmation; CONFIRM=<word> answers it where there is no terminal.
confirm() { # word
  local answer="${CONFIRM:-}"
  [[ -n "$answer" ]] || read -r -p "   Type $1 to continue: " answer || true
  [[ "$answer" == "$1" ]] || die "not confirmed (type $1, or run with CONFIRM=$1)"
}

ROUTER=$(jq -r '.contracts.router.address' "$DEPLOYMENTS_FILE")
OWNER=$("$STELLAR" keys public-key "$SOURCE")

view() { # contract fn args...
  local id="$1"
  shift
  "$STELLAR" contract invoke --network "$NETWORK" --source-account "$OWNER" --send=no --id "$id" -- "$@" 2>/dev/null
}

# Sign with the Ledger and submit; prints "<result>|<tx hash>".
submit() { # contract fn args...
  local id="$1" errfile out hash
  shift
  errfile=$(mktemp)
  echo "   -> confirm on the Ledger: $*" >&2
  out=$("$STELLAR" contract invoke --network "$NETWORK" --source-account "$SOURCE" \
    --inclusion-fee "$INCLUSION_FEE" --send=yes --id "$id" -- "$@" 2>"$errfile") \
    || { cat "$errfile" >&2; rm -f "$errfile"; die "$1 failed"; }
  hash=$(grep -oE '[0-9a-f]{64}' "$errfile" | tail -1)
  rm -f "$errfile"
  echo "${out}|${hash:-unknown}"
}

onchain_router_hash() {
  local tmp
  tmp=$(mktemp)
  "$STELLAR" contract fetch --network "$NETWORK" --id "$ROUTER" -o "$tmp" 2>/dev/null
  shasum -a 256 "$tmp" | cut -d' ' -f1
  rm -f "$tmp"
}

source_for() { # token -> TokenPriceSource JSON
  local usdc_usd="{\"oracle\":\"$REFLECTOR_USD\",\"asset\":{\"Other\":\"USDC\"}}"
  case "$1" in
    "$USDC") echo "{\"feed\":$usdc_usd,\"quotes\":[]}" ;;
    "$USDT0" | "$PYUSD") echo "{\"feed\":{\"oracle\":\"$REFLECTOR_DEX\",\"asset\":{\"Stellar\":\"$1\"}},\"quotes\":[$usdc_usd]}" ;;
    *) die "no price source for pool token $1" ;;
  esac
}

# ---------------------------------------------------------------------------
step "preflight: build"
make build-router >/dev/null 2>&1 || die "make build-router failed"
built=$(shasum -a 256 "$ROUTER_WASM" | cut -d' ' -f1)
[[ "$built" == "$NEW_ROUTER_HASH" ]] || die "local build $built is not the testnet-demonstrated $NEW_ROUTER_HASH"
ok "local router build is the testnet-demonstrated $NEW_ROUTER_HASH"

step "preflight: mainnet router and pool $POOL_ID"
current=$(onchain_router_hash)
case "$current" in
  "$OLD_ROUTER_HASH") ok "router $ROUTER runs the expected previous code" ;;
  "$NEW_ROUTER_HASH") ok "router $ROUTER already runs the new code (resuming)" ;;
  *) die "router runs unexpected code $current" ;;
esac
[[ "$(view "$ROUTER" get_owner | tr -d '"')" == "$OWNER" ]] || die "router owner is not $OWNER"
ok "router owner is the Ledger key $OWNER"
pool=$(view "$ROUTER" pool_at --id "$POOL_ID" | tr -d '"')
[[ "$pool" == "$(jq -r ".contracts.pools[] | select(.id == $POOL_ID) | .address" "$DEPLOYMENTS_FILE")" ]] || die "pool $POOL_ID address mismatch"
tokens=$(view "$pool" get_tokens)
[[ "$(jq -c . <<<"$tokens")" == "[\"$USDT0\",\"$PYUSD\",\"$USDC\"]" ]] || die "pool tokens are $tokens"
ok "pool $POOL_ID $pool holds USDT0, PYUSD, USDC"
[[ "$(view "$REFLECTOR_DEX" base | jq -c .)" == "{\"Stellar\":\"$USDC\"}" ]] || die "DEX feed base is not USDC"
[[ "$(view "$REFLECTOR_USD" base | jq -c .)" == '{"Other":"USD"}' ]] || die "USD feed base is not USD"
ok "Reflector feed bases: DEX in USDC, external in USD"

sources=""
for t in $(jq -r '.[]' <<<"$tokens"); do sources+="${sources:+,}$(source_for "$t")"; done
CONFIG="{\"sources\":[${sources}],\"peg_price\":\"$PEG_PRICE\",\"max_deviation_bps\":$MAX_DEVIATION_BPS,\"max_price_age\":$MAX_PRICE_AGE,\"confirmations\":$CONFIRMATIONS}"

step "preflight: live Reflector readings, evaluated like check_depeg"
readings=$(mktemp)
for feed in $(jq -c '.sources[] | .feed, .quotes[]' <<<"$CONFIG" | sort -u); do
  oracle=$(jq -r .oracle <<<"$feed")
  asset=$(jq -c .asset <<<"$feed")
  jq -nc --argjson feed "$feed" \
    --argjson decimals "$(view "$oracle" decimals)" \
    --argjson records "$(view "$oracle" prices --asset "$asset" --records "$CONFIRMATIONS")" \
    '{feed: $feed, decimals: $decimals, records: $records}' >>"$readings"
done
python3 -I - "$readings" "$CONFIG" "$(date +%s)" "$(jq -r '.[]' <<<"$tokens" | paste -sd, -)" <<'PY' || die "preflight evaluation failed"
import json, sys
readings_path, config, now, tokens = sys.argv[1], json.loads(sys.argv[2]), int(sys.argv[3]), sys.argv[4].split(",")
feeds = {}
for line in open(readings_path):
    r = json.loads(line)
    feeds[json.dumps(r["feed"], sort_keys=True)] = r
SCALE, n, peg = 10**14, config["confirmations"], int(config["peg_price"])

def read(feed):
    r = feeds[json.dumps(feed, sort_keys=True)]
    recs = r["records"]
    if not recs or len(recs) < n:
        sys.exit(f"{feed}: fewer than {n} readings")
    out, prev = [], None
    for rec in recs[:n]:
        price, ts = int(rec["price"]), int(rec["timestamp"])
        if price <= 0 or (prev is not None and ts >= prev):
            sys.exit(f"{feed}: malformed readings {recs}")
        prev = ts
        d = r["decimals"]
        out.append((ts, price // 10**(d - 14) if d >= 14 else price * 10**(14 - d)))
    return out

breached_any = False
for token, src in zip(tokens, config["sources"]):
    base = read(src["feed"])
    age = now - base[0][0]
    if age > config["max_price_age"]:
        sys.exit(f"{token}: latest reading {age}s old (StalePrice)")
    quotes = [dict(read(q)) for q in src["quotes"]]
    devs, breached = [], True
    for ts, p in base:
        for q in quotes:
            if ts not in q:
                sys.exit(f"{token}: quote has no reading at {ts} (PriceUnavailable)")
            p = p * q[ts] // SCALE
        dist = abs(p - peg)
        breached &= dist * 10_000 > config["max_deviation_bps"] * peg
        devs.append((p, dist * 10_000 // peg))
    breached_any |= breached
    print(f"   {token}: latest {devs[0][0] / SCALE:.6f} USD ({age}s old), deviation bps per reading {[d for _, d in devs]}{'  BREACHED' if breached else ''}")
if breached_any:
    sys.exit("a token is already past the threshold: check_depeg would pause pool 0 immediately")
PY
rm -f "$readings"
ok "all feeds current and aligned; check_depeg would report Healthy"

if [[ "${DRY_RUN:-0}" == 1 ]]; then
  step "dry run complete: nothing signed"
  echo "$CONFIG" | jq .
  exit 0
fi

printf '\nAbout to sign on mainnet with %s:\n  1. upload router WASM %s\n  2. upgrade router %s in place\n  3. set_depeg_guard on pool %s (%s bps, %ss, %s readings)\n' \
  "$OWNER" "${NEW_ROUTER_HASH:0:12}…" "$ROUTER" "$POOL_ID" "$MAX_DEVIATION_BPS" "$MAX_PRICE_AGE" "$CONFIRMATIONS"
confirm UPGRADE

upload_tx="" upgrade_tx="" guard_tx=""

# ---------------------------------------------------------------------------
if [[ "$(onchain_router_hash)" != "$NEW_ROUTER_HASH" ]]; then
  step "1. upload router WASM"
  echo "   -> confirm on the Ledger: upload WASM" >&2
  errfile=$(mktemp)
  uploaded=$("$STELLAR" contract upload --network "$NETWORK" --source-account "$SOURCE" \
    --inclusion-fee "$INCLUSION_FEE" --wasm "$ROUTER_WASM" 2>"$errfile") || { cat "$errfile" >&2; die "upload failed"; }
  upload_tx=$(grep -oE '[0-9a-f]{64}' "$errfile" | grep -v "$NEW_ROUTER_HASH" | tail -1 || true)
  rm -f "$errfile"
  [[ "$uploaded" == "$NEW_ROUTER_HASH" ]] || die "uploaded hash $uploaded"
  ok "uploaded $uploaded ${upload_tx:+(tx $upload_tx)}"

  step "2. upgrade the router in place"
  r=$(submit "$ROUTER" upgrade --new_wasm_hash "$NEW_ROUTER_HASH" --operator "$OWNER")
  upgrade_tx="${r##*|}"
  [[ "$(onchain_router_hash)" == "$NEW_ROUTER_HASH" ]] || die "router code did not change"
  ok "router upgraded (tx $upgrade_tx)"
else
  step "1-2. router already upgraded; skipping"
fi
# Pool creation is permissionless, so the count may be above 1; pool 0 must be unchanged.
[[ "$(view "$ROUTER" next_pool_id)" -ge 1 ]] || die "pool registry is empty after the upgrade"
[[ "$(view "$ROUTER" pool_at --id "$POOL_ID" | tr -d '"')" == "$pool" ]] || die "pool registry changed"
ok "registry preserved"

# ---------------------------------------------------------------------------
if [[ "$(view "$ROUTER" get_depeg_guard --pool_id "$POOL_ID" | jq -S -c .)" != "$(jq -S -c . <<<"$CONFIG")" ]]; then
  step "3. protect pool $POOL_ID"
  r=$(submit "$ROUTER" set_depeg_guard --pool_id "$POOL_ID" --config "$CONFIG")
  guard_tx="${r##*|}"
  ok "guard set (tx $guard_tx)"
else
  step "3. guard already set; skipping"
fi

check=$(view "$ROUTER" check_depeg --pool_id "$POOL_ID")
[[ "$(jq -r .status <<<"$check")" == Healthy ]] || die "check_depeg simulation: $check"
ok "check_depeg simulation: $check"

# ---------------------------------------------------------------------------
step "record in $DEPLOYMENTS_FILE"
tmp=$(mktemp)
jq --arg new "$NEW_ROUTER_HASH" --arg old "$OLD_ROUTER_HASH" \
  --arg commit "$(git rev-parse --short HEAD)" \
  --arg at "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
  --arg upload_tx "$upload_tx" --arg upgrade_tx "$upgrade_tx" --arg guard_tx "$guard_tx" \
  --argjson config "$CONFIG" --argjson pool_id "$POOL_ID" '
  .contracts.router.router_wasm_hash = $new
  | .contracts.router.previous_router_wasm_hashes = ((.contracts.router.previous_router_wasm_hashes // []) + [$old] | unique)
  | .contracts.router.depeg_guard_upgrade = (
      (.contracts.router.depeg_guard_upgrade // {}) + {source_commit: $commit, upgraded_at: $at}
      + (if $upload_tx != "" then {wasm_upload_tx: $upload_tx} else {} end)
      + (if $upgrade_tx != "" then {upgrade_tx: $upgrade_tx} else {} end))
  | .contracts.pools |= map(if .id == $pool_id then
      .depeg_guard = ((.depeg_guard // {}) + {config: $config} + (if $guard_tx != "" then {set_tx: $guard_tx} else {} end))
    else . end)
  | .notes = ((.notes // []) + ["Router upgraded in place to the depeg-guard build; pool 0 protected (200 bps, 900 s, 3 Reflector readings). Run scripts/depeg-keeper.sh to watch it."] | unique)
' "$DEPLOYMENTS_FILE" >"$tmp" && mv "$tmp" "$DEPLOYMENTS_FILE"
ok "recorded"

step "mainnet depeg upgrade complete"
echo "Start the keeper: LOOP=1 scripts/depeg-keeper.sh  (identity: spreadless-depeg-keeper, funded with a few XLM)"
