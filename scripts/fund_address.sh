#!/usr/bin/env bash
set -euo pipefail

# Optional: delay execution to allow local node / faucet to finish booting.
FUND_DELAY_SECONDS=${FUND_DELAY_SECONDS:-3}
if [[ ${FUND_DELAY_SECONDS} -gt 0 ]]; then
  echo "[fund] Delaying start for ${FUND_DELAY_SECONDS}s..."
  sleep "${FUND_DELAY_SECONDS}"
fi

# Ensure localnet environment exists and switch to it
if ! sui client envs 2>/dev/null | grep -q "localnet"; then
  echo "[fund] Creating localnet environment..."
  sui client new-env --alias localnet --rpc http://127.0.0.1:9000
fi

sui client switch --env localnet
echo "[fund] Switched to localnet environment."

ADDRESS=$(sui client active-address)
echo "[fund] Funding address: $ADDRESS"

# Faucet retry loop with exponential backoff
MAX_FAUCET_ATTEMPTS=${MAX_FAUCET_ATTEMPTS:-15}
for attempt in $(seq 1 ${MAX_FAUCET_ATTEMPTS}); do
  echo "[fund] Faucet attempt ${attempt}/${MAX_FAUCET_ATTEMPTS}..."
  if sui client faucet --address "$ADDRESS" 2>/dev/null; then
    echo "[fund] Faucet request succeeded."
    break
  fi
  if [[ $attempt -eq ${MAX_FAUCET_ATTEMPTS} ]]; then
    echo "[fund] Faucet failed after ${MAX_FAUCET_ATTEMPTS} attempts." >&2
    exit 1
  fi
  SLEEP=$(( attempt * 2 ))
  if [[ $SLEEP -gt 10 ]]; then SLEEP=10; fi
  echo "[fund] Sleeping ${SLEEP}s before retry..."
  sleep $SLEEP
done

# Poll balance with timeout
MAX_BALANCE_WAIT=${MAX_BALANCE_WAIT:-60}
BALANCE=0
for i in $(seq 1 ${MAX_BALANCE_WAIT}); do
  RAW=$(sui client gas 2>/dev/null || true)
  # Check if we have any gas coins
  if echo "$RAW" | grep -q "gasCoinId"; then
    BALANCE=1
    echo "[fund] Balance confirmed at poll ${i}/${MAX_BALANCE_WAIT}"
    break
  fi
  echo "[fund] Poll ${i}/${MAX_BALANCE_WAIT} - waiting for balance..."
  sleep 1
done

if [[ ${BALANCE} -eq 0 ]]; then
  echo "[fund] Failed to observe funded balance after ${MAX_BALANCE_WAIT}s" >&2
  exit 1
fi

echo "[fund] Address funded successfully!"
sui client gas
