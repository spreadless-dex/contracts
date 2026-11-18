#!/usr/bin/env bash
# Depeg keeper: call the router's permissionless `check_depeg` for every
# protected pool, submitting a transaction only when it would pause one.
#
# Each pool is first simulated (free). `Healthy` and `AlreadyPaused` results
# are logged and nothing is sent; `Paused` is submitted so the pause lands
# on-chain. Simulation errors (`StalePrice`, `PriceUnavailable`, RPC trouble)
# are logged as warnings: the guard never acts on stale data, so a stale feed
# means the pool is unprotected until the feed recovers.
#
# The keeper key needs no role anywhere; it only pays fees (a few XLM lasts a
# long time: a check costs ~0.001 XLM, and only pauses are submitted).
#
#   scripts/depeg-keeper.sh            # one pass (for cron / systemd timers)
#   LOOP=1 scripts/depeg-keeper.sh     # run forever, ~1 minute after each
#                                      # 5-minute Reflector update
#
# Environment:
#   NETWORK        mainnet (default) or testnet
#   KEEPER         stellar identity paying fees (default spreadless-depeg-keeper)
#   ROUTER         router address (default: deployments/$NETWORK.json)
#   POOL_IDS       space-separated pool ids (default: every pool with a guard)
#   ALERT_CMD      run as `$ALERT_CMD "<message>"` on a pause or an error
#   INCLUSION_FEE  max inclusion fee in stroops when submitting (default 10000)
#   PERIOD, OFFSET loop schedule in seconds (default 300 and 60)
#
# Exit status of a single pass: 0 if every pool was checked, 1 otherwise.
set -uo pipefail

STELLAR="${STELLAR:-stellar}"
NETWORK="${NETWORK:-mainnet}"
KEEPER="${KEEPER:-spreadless-depeg-keeper}"
INCLUSION_FEE="${INCLUSION_FEE:-10000}"
PERIOD="${PERIOD:-300}"
OFFSET="${OFFSET:-60}"
ALERT_CMD="${ALERT_CMD:-}"

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ROUTER="${ROUTER:-$(jq -r '.contracts.router.address' "$repo_root/deployments/${NETWORK}.json")}"

log() { printf '%s %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$*"; }
alert() {
  log "ALERT: $*"
  if [[ -n "$ALERT_CMD" ]]; then $ALERT_CMD "$*" || log "alert command failed"; fi
}

invoke() { # send-mode fn args...
  local send="$1"
  shift
  "$STELLAR" contract invoke --network "$NETWORK" --source-account "$KEEPER" \
    --inclusion-fee "$INCLUSION_FEE" --send="$send" --id "$ROUTER" -- "$@"
}

protected_pools() {
  if [[ -n "${POOL_IDS:-}" ]]; then
    echo "$POOL_IDS"
    return
  fi
  local next id ids=""
  next=$(invoke no next_pool_id 2>/dev/null) || return 1
  for ((id = 0; id < next; id++)); do
    [[ "$(invoke no get_depeg_guard --pool_id "$id" 2>/dev/null)" == null ]] || ids+="$id "
  done
  echo "$ids"
}

check_pool() { # pool_id
  local id="$1" out err status
  err=$(mktemp)
  if ! out=$(invoke no check_depeg --pool_id "$id" 2>"$err"); then
    alert "pool $id: check failed in simulation: $(grep -oE 'Error\([^)]*\)' "$err" | head -1 || true) $(tail -1 "$err")"
    rm -f "$err"
    return 1
  fi
  rm -f "$err"
  status=$(jq -r '.status' <<<"$out")
  case "$status" in
    Healthy | AlreadyPaused)
      log "pool $id: $status $(jq -c 'del(.status)' <<<"$out")"
      ;;
    Paused)
      err=$(mktemp)
      if out=$(invoke yes check_depeg --pool_id "$id" 2>"$err"); then
        alert "pool $id: DEPEG PAUSE submitted: $out tx $(grep -oE '[0-9a-f]{64}' "$err" | tail -1)"
      else
        alert "pool $id: depeg detected but submitting the pause failed: $(tail -1 "$err")"
        rm -f "$err"
        return 1
      fi
      rm -f "$err"
      ;;
    *)
      alert "pool $id: unexpected result: $out"
      return 1
      ;;
  esac
}

run_once() {
  local ids id failed=0
  if ! ids=$(protected_pools); then
    alert "could not list pools on router $ROUTER"
    return 1
  fi
  [[ -n "${ids// /}" ]] || log "no protected pools on router $ROUTER"
  for id in $ids; do check_pool "$id" || failed=1; done
  return "$failed"
}

if [[ "${LOOP:-0}" == 1 ]]; then
  log "keeper $("$STELLAR" keys public-key "$KEEPER") watching router $ROUTER on $NETWORK"
  while true; do
    run_once || true
    now=$(date +%s)
    sleep $(((PERIOD - (now % PERIOD) + OFFSET) % PERIOD + 1))
  done
else
  run_once
fi
