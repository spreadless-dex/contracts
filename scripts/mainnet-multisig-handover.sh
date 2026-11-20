#!/usr/bin/env bash
# Move mainnet router ownership from the single Ledger key to a 2-of-3 classic
# Stellar multisig account.
#
# The multisig is an ordinary account whose own (master) key is disabled and
# whose three signers each have weight 1, with every threshold at 2. Run the
# stages in order; each one checks the previous one on-chain first.
#
#   scripts/mainnet-multisig-handover.sh setup      # create + configure the account
#   scripts/mainnet-multisig-handover.sh test       # 1 XLM payment needing 2 signatures
#   scripts/mainnet-multisig-handover.sh transfer   # Ledger proposes, multisig accepts
#   scripts/mainnet-multisig-handover.sh send FILE  # submit a fully signed transaction
#   scripts/mainnet-multisig-handover.sh verify     # confirm + record in deployments/mainnet.json
#
# `test` and `transfer` each print a transaction already signed by the Ledger
# (signer 1). A second signer signs it at https://lab.stellar.org (Transactions
# > Sign transaction, network Mainnet) and submits it there, or sends the signed
# XDR back to be submitted with `send`. Transactions carry no expiry, so the
# second signature can be added later.
set -euo pipefail

STELLAR="${STELLAR:-stellar}"
NETWORK=mainnet
HORIZON=https://horizon.stellar.org
LEDGER_ID="${LEDGER_ID:-spreadless-mainnet}"            # current router owner, signer 1
MULTISIG_ID="${MULTISIG_ID:-spreadless-mainnet-multisig}" # local key, disabled by `setup`
INCLUSION_FEE="${INCLUSION_FEE:-10000}"
DEPLOYMENTS_FILE=deployments/mainnet.json
EXPECTED_ROUTER_HASH=a1bb2f7afb71db36706293c0a3839418f334d9f321e14e2c98a8dede14a76f1d
THRESHOLD=2
MIN_FUNDING_XLM=5

SIGNER_2=GAXTKDSCB23GQZEEVF6RQDFPASJX3IUBX4LFGVXY5SFYPNUYGKJ6TUW3
SIGNER_3=GBQVQBPNRFVQYDYHBKNL2BJBPTFWGGZFZ2KFM2XN7ZVKAEMA6GUZ6N2N

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"
export STELLAR_NO_CACHE=true

step() { printf '\n== %s\n' "$*"; }
ok() { printf '   ok: %s\n' "$*"; }
die() { printf '   ABORT: %s\n' "$*" >&2; exit 1; }

ROUTER=$(jq -r '.contracts.router.address' "$DEPLOYMENTS_FILE")
LEDGER_ADDR=$("$STELLAR" keys public-key "$LEDGER_ID")
SIGNERS="$LEDGER_ADDR $SIGNER_2 $SIGNER_3"

account() { curl -fsS "$HORIZON/accounts/$1" 2>/dev/null; }
view() { # contract fn args...
  local id="$1"
  shift
  "$STELLAR" contract invoke --network "$NETWORK" --source-account "$LEDGER_ADDR" --send=no --id "$id" -- "$@" 2>/dev/null
}
multisig_addr() { "$STELLAR" keys public-key "$MULTISIG_ID" 2>/dev/null; }

check_signers() {
  local s
  for s in $SIGNERS; do
    [[ "$s" =~ ^G[A-Z2-7]{55}$ ]] || die "signer $s is not a 56-character G-address"
    # Round-trips through the CLI, which verifies the address checksum.
    "$STELLAR" tx new payment --network "$NETWORK" --source-account "$LEDGER_ADDR" \
      --destination "$s" --amount 1 --build-only >/dev/null 2>&1 || die "signer $s is not a valid address"
  done
  [[ "$(tr ' ' '\n' <<<"$SIGNERS" | sort -u | wc -l | tr -d ' ')" == 3 ]] || die "signers must be three different addresses"
}

# Fails unless the account is exactly the intended 2-of-3.
require_configured() {
  local addr="$1" acct
  acct=$(account "$addr") || die "multisig account $addr does not exist; run setup"
  jq -e --arg a "$addr" --argjson t "$THRESHOLD" --arg signers "$SIGNERS" '
    (.thresholds.low_threshold == $t and .thresholds.med_threshold == $t and .thresholds.high_threshold == $t)
    and ([.signers[] | select(.key == $a) | .weight] == [0])
    and (([.signers[] | select(.weight > 0)] | map(.key) | sort) == ($signers | split(" ") | sort))
    and ([.signers[] | select(.weight > 0) | .weight] | all(. == 1))
  ' <<<"$acct" >/dev/null || die "multisig $addr is not a 2-of-3 of the configured signers; run setup"
}

print_for_cosigner() { # file description
  printf '\n   Signed by the Ledger (1 of %s). A second signer must now sign and submit:\n' "$THRESHOLD"
  printf '     1. open https://lab.stellar.org  >  Transactions  >  Sign transaction, network Mainnet\n'
  printf '     2. paste the transaction below, sign with %s or %s, then submit\n' "${SIGNER_2:0:6}…" "${SIGNER_3:0:6}…"
  printf '   (or return the signed XDR and run: %s send <file>)\n\n' "$0"
  cat "$1"
  printf '\n\n   saved to %s (%s)\n' "$1" "$2"
}

# ---------------------------------------------------------------------------
cmd_setup() {
  local addr acct balance s
  step "signers"
  check_signers
  for s in $SIGNERS; do ok "$s"; done

  step "multisig account"
  if ! multisig_addr >/dev/null; then
    "$STELLAR" keys generate "$MULTISIG_ID" >/dev/null
    ok "generated local key $MULTISIG_ID"
  fi
  addr=$(multisig_addr)
  echo "   address: $addr"
  if ! acct=$(account "$addr"); then
    die "not funded yet: send at least $MIN_FUNDING_XLM XLM to $addr, then run setup again"
  fi
  balance=$(jq -r '.balances[] | select(.asset_type == "native") | .balance' <<<"$acct")
  ok "balance $balance XLM"

  if jq -e --arg a "$addr" '[.signers[] | select(.key == $a) | .weight] == [0]' <<<"$acct" >/dev/null; then
    require_configured "$addr"
    ok "already configured: 2-of-3, master key disabled"
    return
  fi
  [[ "${balance%.*}" -ge "$MIN_FUNDING_XLM" ]] || die "needs at least $MIN_FUNDING_XLM XLM (reserves for 3 signers, the test payment and fees)"

  step "add signers (weight 1 each)"
  for s in $SIGNERS; do
    if jq -e --arg s "$s" '.signers | any(.key == $s and .weight == 1)' <<<"$acct" >/dev/null; then
      ok "$s already a signer"
    else
      "$STELLAR" tx new set-options --network "$NETWORK" --source-account "$MULTISIG_ID" \
        --inclusion-fee "$INCLUSION_FEE" --signer "$s" --signer-weight 1 >/dev/null
      ok "added $s"
    fi
  done
  acct=$(account "$addr")
  for s in $SIGNERS; do
    jq -e --arg s "$s" '.signers | any(.key == $s and .weight == 1)' <<<"$acct" >/dev/null || die "signer $s missing on-chain"
  done
  [[ "$(jq '[.signers[] | select(.weight > 0)] | length' <<<"$acct")" == 4 ]] || die "unexpected signers on the account"

  step "set thresholds to $THRESHOLD and disable the master key"
  printf '   After this, %s can only act with %s of these signatures:\n' "$addr" "$THRESHOLD"
  for s in $SIGNERS; do printf '     %s\n' "$s"; done
  read -r -p "   Type LOCK to continue: " answer
  [[ "$answer" == LOCK ]] || die "not confirmed"
  "$STELLAR" tx new set-options --network "$NETWORK" --source-account "$MULTISIG_ID" \
    --inclusion-fee "$INCLUSION_FEE" --master-weight 0 \
    --low-threshold "$THRESHOLD" --med-threshold "$THRESHOLD" --high-threshold "$THRESHOLD" >/dev/null
  require_configured "$addr"
  ok "multisig $addr: 3 signers, threshold $THRESHOLD, master key disabled"
  echo "   next: $0 test"
}

# ---------------------------------------------------------------------------
cmd_test() {
  local addr out=target/multisig-test-payment.xdr tx
  addr=$(multisig_addr) || die "run setup first"
  require_configured "$addr"

  step "test payment: 1 XLM from the multisig back to the Ledger"
  tx=$("$STELLAR" tx new payment --network "$NETWORK" --source-account "$addr" \
    --inclusion-fee "$INCLUSION_FEE" --destination "$LEDGER_ADDR" --amount 10000000 --build-only)
  echo "   -> confirm on the Ledger" >&2
  tx=$("$STELLAR" tx sign --network "$NETWORK" --sign-with-key "$LEDGER_ID" <<<"$tx")

  if "$STELLAR" tx send --network "$NETWORK" <<<"$tx" >/dev/null 2>&1; then
    die "the payment went through with ONE signature: the multisig is not enforcing its threshold"
  fi
  ok "rejected with a single signature, as it must be"

  echo "$tx" >"$out"
  print_for_cosigner "$out" "test payment"
  echo "   when it is submitted, check with: $0 verify-test"
}

test_payment_done() { # multisig address
  curl -fsS "$HORIZON/accounts/$1/payments?limit=50&order=desc" 2>/dev/null |
    jq -e --arg from "$1" --arg to "$LEDGER_ADDR" \
      '._embedded.records | any(.type == "payment" and .from == $from and .to == $to and .asset_type == "native")' >/dev/null
}

cmd_verify_test() {
  local addr
  addr=$(multisig_addr) || die "run setup first"
  test_payment_done "$addr" || die "no payment from $addr to $LEDGER_ADDR yet"
  ok "2-of-3 test payment is on-chain: the multisig can sign"
  echo "   next: $0 transfer"
}

# ---------------------------------------------------------------------------
cmd_transfer() {
  local addr tmp owner latest live_until out=target/multisig-accept-ownership.xdr tx
  addr=$(multisig_addr) || die "run setup first"
  require_configured "$addr"

  step "preflight"
  test_payment_done "$addr" || die "run the test payment first ($0 test)"
  ok "multisig has already signed a 2-of-3 payment"
  tmp=$(mktemp)
  "$STELLAR" contract fetch --network "$NETWORK" --id "$ROUTER" -o "$tmp" 2>/dev/null
  [[ "$(shasum -a 256 "$tmp" | cut -d' ' -f1)" == "$EXPECTED_ROUTER_HASH" ]] ||
    die "router is not on the depeg-guard build yet; run make mainnet-depeg-upgrade first"
  rm -f "$tmp"
  ok "router runs $EXPECTED_ROUTER_HASH"
  owner=$(view "$ROUTER" get_owner | tr -d '"')

  if [[ "$owner" == "$addr" ]]; then
    ok "multisig already owns the router"
    return
  fi
  [[ "$owner" == "$LEDGER_ADDR" ]] || die "router owner is $owner, not the Ledger"

  step "1. Ledger proposes the transfer (it stays owner until the multisig accepts)"
  printf '   new owner: %s\n' "$addr"
  read -r -p "   Type TRANSFER to continue: " answer
  [[ "$answer" == TRANSFER ]] || die "not confirmed"
  latest=$("$STELLAR" ledger latest --network "$NETWORK" --output json 2>/dev/null | jq -r '.sequence')
  live_until=$((latest + 34560)) # ~2 days to collect the second signature
  echo "   -> confirm on the Ledger" >&2
  "$STELLAR" contract invoke --network "$NETWORK" --source-account "$LEDGER_ID" \
    --inclusion-fee "$INCLUSION_FEE" --send=yes --id "$ROUTER" -- \
    transfer_ownership --new_owner "$addr" --live_until_ledger "$live_until" >/dev/null
  [[ "$(view "$ROUTER" get_owner | tr -d '"')" == "$LEDGER_ADDR" ]] || die "owner changed before acceptance"
  ok "transfer proposed, valid for about 2 days"

  step "2. multisig accepts"
  tx=$("$STELLAR" contract invoke --network "$NETWORK" --source-account "$addr" \
    --inclusion-fee "$INCLUSION_FEE" --build-only --id "$ROUTER" -- accept_ownership)
  tx=$("$STELLAR" tx simulate --network "$NETWORK" --source-account "$addr" <<<"$tx")
  echo "   -> confirm on the Ledger" >&2
  tx=$("$STELLAR" tx sign --network "$NETWORK" --sign-with-key "$LEDGER_ID" <<<"$tx")
  echo "$tx" >"$out"
  print_for_cosigner "$out" "accept_ownership"
  echo "   when it is submitted, finish with: $0 verify"
}

cmd_send() {
  [[ -f "${1:-}" ]] || die "usage: $0 send <file with signed transaction XDR>"
  "$STELLAR" tx send --network "$NETWORK" <"$1"
}

# ---------------------------------------------------------------------------
cmd_verify() {
  local addr owner tmp
  addr=$(multisig_addr) || die "run setup first"
  require_configured "$addr"
  owner=$(view "$ROUTER" get_owner | tr -d '"')
  [[ "$owner" == "$addr" ]] || die "router owner is still $owner"
  ok "router $ROUTER is owned by the 2-of-3 multisig $addr"

  tmp=$(mktemp)
  jq --arg owner "$addr" --arg signers "$SIGNERS" --argjson t "$THRESHOLD" \
    --arg at "$(date -u +%Y-%m-%dT%H:%M:%SZ)" --arg previous "$LEDGER_ADDR" '
    .contracts.router.owner = $owner
    | .contracts.router.owner_multisig = {threshold: $t, signers: ($signers | split(" ")), master_key_disabled: true, previous_owner: $previous, transferred_at: $at}
    | .notes = ([.notes[]? | select(test("single Ledger key") | not)]
        + ["Router owner is a 2-of-3 classic multisig account; the protocol fee beneficiary is unchanged."] | unique)
  ' "$DEPLOYMENTS_FILE" >"$tmp" && mv "$tmp" "$DEPLOYMENTS_FILE"
  ok "recorded in $DEPLOYMENTS_FILE"
}

case "${1:-}" in
  setup) cmd_setup ;;
  test) cmd_test ;;
  verify-test) cmd_verify_test ;;
  transfer) cmd_transfer ;;
  send) cmd_send "${2:-}" ;;
  verify) cmd_verify ;;
  *) sed -n '2,20p' "$0" | sed 's/^# \{0,1\}//'; exit 1 ;;
esac
