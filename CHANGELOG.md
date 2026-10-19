# Changelog

Every released or deployed build lists the commit it was built from and the
SHA-256 of `milestone_escrow.wasm` produced by `scripts/build-reproducible.sh`
for that commit. See "Verifying the deployed contract" in the README.

## Unreleased

WASM: build with `scripts/build-reproducible.sh <commit>`; CI publishes the
hash for every commit in the `reproducible-wasm` job summary.

### Security

- `propose_admin_transfer` and `transfer_admin` reject the contract's own
  address and the current admin as `new_admin` with `InvalidAddress` (#771).
- `set_escrow_interest_yield` (admin only) is closed once the escrow is funded
  and returns `AlreadyFunded`; after funding the yield split can only change
  through `set_interest_yield_consent`, which the client and freelancer must
  sign. `unlock_escrow_interest_yield` now requires the admin, the client and
  the freelancer, so the admin cannot undo its own lock (#776).

### Changed

- `WhitelistedTokens`, `MultiSigConfig`, `InterestYieldState` and
  `PlatformFeeAllocation` moved from instance storage to persistent keys, so
  calls that do not use them stop loading them. With a 50-token whitelist and a
  32-signer multisig, the instance entry `approve_milestone` loads shrinks from
  4,212 to 644 bytes and its metered instructions drop by about 15 % (#772).

### Added

- `migrate_cold_config(admin)`: run once after upgrading an existing deployment
  to move the configuration above out of instance storage. Reads fall back to
  the old location until then (#772).
- Reproducible WASM builds: `scripts/build-reproducible.sh`, the pinned
  `build/reproducible.Dockerfile`, a CI job that publishes the hash per commit,
  and `deployments/testnet.json` (#773).

## Testnet deployment `CDD5WKK3…4MUH`

WASM sha256 `67d0eb543aaa9ca99c1b23ba18a3f4713c899eba574bbef96540f2540b8b2f0e`,
source commit unknown (deployed before reproducible builds).
