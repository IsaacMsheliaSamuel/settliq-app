#!/usr/bin/env bash
set -euo pipefail

NETWORK="${NETWORK:-testnet}"
CONTRACT_ID="${CONTRACT_ID:-${ESCROW_CONTRACT_ID:-}}"

case "$NETWORK" in
  testnet)
    RPC_URL="${SOROBAN_RPC_URL:-https://soroban-testnet.stellar.org}"
    NETWORK_PASSPHRASE="${STELLAR_NETWORK_PASSPHRASE:-Test SDF Network ; September 2015}"
    ;;
  mainnet)
    RPC_URL="${SOROBAN_RPC_URL:-https://mainnet.sorobanrpc.com}"
    NETWORK_PASSPHRASE="${STELLAR_NETWORK_PASSPHRASE:-Public Global Stellar Network ; September 2015}"
    ;;
  *)
    echo "Unsupported NETWORK: ${NETWORK}. Use testnet or mainnet." >&2
    exit 1
    ;;
esac

if [[ -z "${CONTRACT_ID}" ]]; then
  echo "Missing CONTRACT_ID (or ESCROW_CONTRACT_ID)." >&2
  exit 1
fi

if [[ -z "${STELLAR_SOURCE_ACCOUNT_SECRET:-}" ]]; then
  echo "Missing STELLAR_SOURCE_ACCOUNT_SECRET." >&2
  exit 1
fi

# PaymentEscrowContract.__constructor requires 8 arguments (#89)
if [[ -z "${ESCROW_ADMIN:-}" || -z "${ESCROW_XLM_TOKEN:-}" || -z "${ESCROW_USDC_TOKEN:-}" || \
      -z "${ESCROW_DEFAULT_TTL_LEDGERS:-}" || -z "${ESCROW_REGISTRY:-}" || \
      -z "${ESCROW_EMERGENCY_SIGNERS:-}" || -z "${ESCROW_EMERGENCY_TREASURY:-}" || \
      -z "${ESCROW_EMERGENCY_COOLDOWN_LEDGERS:-}" ]]; then
  echo "Missing one or more required constructor args:" >&2
  echo "  ESCROW_ADMIN, ESCROW_XLM_TOKEN, ESCROW_USDC_TOKEN," >&2
  echo "  ESCROW_DEFAULT_TTL_LEDGERS, ESCROW_REGISTRY," >&2
  echo "  ESCROW_EMERGENCY_SIGNERS, ESCROW_EMERGENCY_TREASURY," >&2
  echo "  ESCROW_EMERGENCY_COOLDOWN_LEDGERS" >&2
  exit 1
fi

INIT_CMD=(
  stellar contract invoke
  --id "${CONTRACT_ID}"
  --source-account "${STELLAR_SOURCE_ACCOUNT_SECRET}"
  --rpc-url "${RPC_URL}"
  --network-passphrase "${NETWORK_PASSPHRASE}"
  --fn "__constructor"
  --admin "${ESCROW_ADMIN}"
  --xlm_token "${ESCROW_XLM_TOKEN}"
  --usdc_token "${ESCROW_USDC_TOKEN}"
  --default_ttl_ledgers "${ESCROW_DEFAULT_TTL_LEDGERS}"
  --registry "${ESCROW_REGISTRY}"
  --emergency_signers "${ESCROW_EMERGENCY_SIGNERS}"
  --emergency_treasury "${ESCROW_EMERGENCY_TREASURY}"
  --emergency_cooldown_ledgers "${ESCROW_EMERGENCY_COOLDOWN_LEDGERS}"
)

echo "Initializing escrow contract ${CONTRACT_ID} on ${NETWORK} (__constructor)..."
"${INIT_CMD[@]}"
echo "Initialization complete."
