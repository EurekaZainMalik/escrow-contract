#!/usr/bin/env bash
#
# scripts/e2e.sh — local end-to-end smoke test.
#
# Builds the milestone-escrow contract, deploys it to a throwaway local
# Stellar network, and drives the funded -> delivered -> disputed -> refunded
# path, asserting the resulting on-chain state.
#
# Requirements:
#   * stellar-cli (https://developers.stellar.org/docs/tools/developer-tools/cli)
#   * Docker (the local network runs in a container)
#
# Usage:
#   ./scripts/e2e.sh
#
# Environment overrides:
#   NETWORK_NAME   local network name (default: milestone-e2e)
#   KEEP_NETWORK=1 leave the local network running on exit (for debugging)

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MANIFEST="${ROOT_DIR}/contracts/milestone-escrow/Cargo.toml"
NETWORK="${NETWORK:-local}"
NETWORK_NAME="${NETWORK_NAME:-milestone-e2e}"
KEEP_NETWORK="${KEEP_NETWORK:-0}"

log() { printf '\n\033[1;34m==> %s\033[0m\n' "$*"; }
fail() { printf '\n\033[1;31mFAIL: %s\033[0m\n' "$*" >&2; exit 1; }

require() {
  command -v "$1" >/dev/null 2>&1 || fail "required tool '$1' not found on PATH"
}

cleanup() {
  if [[ "${KEEP_NETWORK}" != "1" ]]; then
    log "Stopping local Stellar network"
    stellar network stop "${NETWORK}" >/dev/null 2>&1 || true
  fi
}
trap cleanup EXIT

log "Checking prerequisites"
require cargo
require docker
require stellar
stellar --version

log "Starting throwaway local network"
stellar network start "${NETWORK}" --name "${NETWORK_NAME}" || true
stellar network use "${NETWORK}" >/dev/null 2>&1 || true

log "Creating and funding test identities"
for who in admin client freelancer arbiter; do
  if ! stellar keys address "${who}" >/dev/null 2>&1; then
    stellar keys generate "${who}" --network "${NETWORK}" --fund >/dev/null
  fi
done
ADMIN="$(stellar keys address admin)"
CLIENT_ADDR="$(stellar keys address client)"
FREELANCER="$(stellar keys address freelancer)"
ARBITER="$(stellar keys address arbiter)"

log "Building contract WASM"
stellar contract build --manifest-path "${MANIFEST}"
WASM="${ROOT_DIR}/target/wasm32v1-none/release/milestone_escrow.wasm"
[[ -f "${WASM}" ]] || fail "expected WASM at ${WASM}"

log "Deploying contract"
CONTRACT_ID="$(stellar contract deploy \
  --wasm "${WASM}" \
  --source admin \
  --network "${NETWORK}")"
echo "contract id: ${CONTRACT_ID}"

log "Deploying a test SAC token"
TOKEN_ID="$(stellar contract asset deploy \
  --asset ":${ADMIN}" \
  --source admin \
  --network "${NETWORK}")"
echo "token id: ${TOKEN_ID}"

invoke() {
  stellar contract invoke \
    --id "${CONTRACT_ID}" \
    --source "${1}" \
    --network "${NETWORK}" \
    -- "${@:2}"
}

log "Initializing escrow"
invoke admin initialize \
  --admin "${ADMIN}" \
  --client "${CLIENT_ADDR}" \
  --freelancer "${FREELANCER}" \
  --arbiter "${ARBITER}" \
  --token "${TOKEN_ID}" \
  --auto_release_seconds 604800 \
  --milestone_amounts '["1000000"]' \
  --milestone_spec_hashes '["0000000000000000000000000000000000000000000000000000000000000000"]'

log "Minting test tokens to the client"
stellar contract invoke \
  --id "${TOKEN_ID}" \
  --source admin \
  --network "${NETWORK}" \
  -- mint --to "${CLIENT_ADDR}" --amount 1000000

log "Funding escrow"
invoke client fund --client "${CLIENT_ADDR}"

log "Marking milestone delivered"
invoke freelancer mark_delivered --freelancer "${FREELANCER}" --milestone_index 0

log "Raising a dispute"
invoke client raise_dispute --caller "${CLIENT_ADDR}" --milestone_index 0

log "Resolving dispute as refund to client"
invoke arbiter resolve_dispute \
  --arbiter "${ARBITER}" \
  --milestone_index 0 \
  --release_to_freelancer false

log "Asserting final milestone status is Refunded"
JOB="$(invoke admin get_job)"
echo "${JOB}"
echo "${JOB}" | grep -qi 'refunded' || fail "expected milestone status Refunded after dispute resolution"

log "E2E passed"
