# Milestone Escrow Contract

A Soroban smart contract on Stellar that enables trustless milestone-based escrow between a client, freelancer, and arbiter.

## Overview

This contract allows a client to fund a job broken into milestones. Funds are locked in the contract and released per milestone only when the client approves delivery. Disputes can be raised by either party and resolved by a designated arbiter.

## Trust Model

The escrow's protection depends on **four distinct roles** assigned at deploy time via `__constructor`:

- **Admin** — the platform operator. Holds admin-only endpoints (whitelist management, pause/resume, admin overrides, upgrades). The admin is expected to be a neutral platform address, not the client or freelancer.
- **Client** — funds the job and approves milestone releases.
- **Freelancer** — delivers milestones and receives payouts.
- **Arbiter** — a neutral third party that resolves disputes via `resolve_dispute` and `apply_dispute_arbitration_split`.

`__constructor` enforces party distinctness: it rejects any collision among `admin`, `client`, `freelancer`, and `arbiter` with `Error::InvalidParties`. This prevents the client from resolving their own disputes (arbiter == client), the freelancer from ruling in their own favour (arbiter == freelancer), or the client from holding every admin override (admin == client). The frontend must pass four genuinely distinct addresses — it must not reuse the connected wallet as both admin and client.

## Arbiter Fee

The constructor's `settings.arbiter_fee_bps` sets an arbiter fee (basis points, max 1,000 = 10 %). When an escrow has a fee, `resolve_dispute` and `apply_dispute_arbitration_split` first pay that share of the disputed milestone's remaining balance to the arbiter, then pay or split the rest between client and freelancer. The fee is fixed at construction; `0` means no fee, and milestones that never go to dispute never pay one (#738).

## Contract Functions

| Function | Caller | Description |
|---|---|---|
| `__constructor` | Deployer | Atomically initializes the job with milestone amounts, spec hashes and `settings` (`EscrowSettings`: funding mode, dispute timeout, arbiter fee) |
| `get_arbiter_fee_bps` | Anyone | Read the arbiter fee (0 when none) |
| `fund` | Client | Deposit total amount into contract (Upfront mode) |
| `fund_milestone` | Client | Deposit one milestone (PerMilestone mode) |
| `add_milestone` | Client + Freelancer | Append a new Pending milestone to a funded job; the client deposits its amount in the same call (capped at `MAX_MILESTONES` = 50) |
| `reduce_milestone` | Client + Freelancer | Lower a Pending milestone's amount and refund the difference to the client |
| `mark_delivered` | Freelancer | Mark a milestone as delivered |
| `request_revision` | Client | Return a delivered milestone to Pending (max 3) |
| `approve_milestone` | Client | Release funds for a delivered milestone |
| `raise_dispute` | Client or Freelancer | Freeze a milestone for arbitration, committing a 32-byte `reason_hash` |
| `submit_evidence` | Client or Freelancer | Anchor an evidence hash on an open dispute (max 10 per dispute) |
| `get_dispute_info` | Anyone | Read the dispute reason hash, timestamps and evidence trail |
| `escalate_dispute` | Client or Freelancer | After the timeout, restore a disputed milestone to its previous status |
| `resolve_dispute` | Arbiter | Release to freelancer or refund to client |
| `get_job` | Anyone | View current job state |
| `get_milestone` | Anyone | View one milestone without loading the whole job |
| `milestone_count` | Anyone | Number of milestones |
| `rotate_freelancer` | Client + Freelancer | Move the freelancer role and future payouts to a new address |
| `sweep_excess` | Client or admin | Recover surplus tokens not backing obligations |
| `emergency_unpause_parties` | Client and freelancer | Jointly lift a party-initiated pause |

Events carry party addresses as extra topics for `getEvents` filtering; see
[EVENTS.md](EVENTS.md#indexed-topics).

## Milestone States

Pending → Delivered → Released

↓

Disputed → Released (arbiter favors freelancer)

→ Refunded (arbiter favors client)

The full state machine, including every entry point, its required signers and the escrow-level locks, is specified in [`contracts/milestone-escrow/SPEC.md`](contracts/milestone-escrow/SPEC.md).

## Security

Roles, trust assumptions, admin powers and known risks are described in [`THREAT_MODEL.md`](THREAT_MODEL.md).

## Prerequisites

- https://rustup.rs/ (the toolchain is pinned to 1.96.0 by `rust-toolchain.toml`)
- https://developers.stellar.org/docs/smart-contracts/getting-started/setup (stellar-cli 28.0.0)
- wasm32 target: `rustup target add wasm32v1-none`
- Docker, for reproducible builds

## Build

For development, from the repository root:

```bash
stellar contract build --manifest-path contracts/milestone-escrow/Cargo.toml
```

Anything that will be deployed must come from the reproducible build instead:

```bash
scripts/build-reproducible.sh
```

It compiles a clean export of `HEAD` (or of the commit given as the first
argument) inside a pinned container (`build/reproducible.Dockerfile`: Rust
1.96.0 by image digest, stellar-cli 28.0.0 by release checksum, linux/amd64)
and writes `dist/milestone_escrow.wasm`, its SHA-256 and `dist/build-info.json`.
The same commit gives the same hash on any machine. CI runs the same script on
every commit and publishes the hash in the job summary and the wasm as an
artifact.

### TypeScript bindings

CI also generates typed TypeScript bindings from the built wasm on every main
build (artifact `milestone-escrow-bindings-ts-<sha>`) and publishes them as the
versioned GitHub Package `@goldii-locks/milestone-escrow-bindings` on `v*`
tags, so clients stop hand-encoding ScVals. See
[docs/typescript-bindings.md](docs/typescript-bindings.md).

## Test

From the repository root:

```bash
cargo test -p milestone-escrow
```

## Project Status

- Mainnet release gates are tracked in the [mainnet readiness checklist](MAINNET.md)
- The admin authorization direction and migration decision are recorded in [ADMIN_MULTISIG_DECISION.md](ADMIN_MULTISIG_DECISION.md)
- Contract implementation completed in `contracts/milestone-escrow`
- Includes milestone delivery, approval, dispute, and arbitration flows
- Initialize now rejects empty milestone lists, zero/negative amounts, zero addresses, zero auto-release windows, and **party collisions** (`admin == client`, `admin == freelancer`, `arbiter == client`, `arbiter == freelancer`, `client == freelancer`) with explicit contract errors
- `raise_dispute` now performs zero-address check, `require_auth`, and identity validation **before** writing the re-entrancy lock (issue #566), matching the authorization pattern used by `fund`, `approve_milestone`, and `resolve_dispute`; includes the five precondition guard assertions present in all other mutating functions
- Disputes may only be raised on delivered or partially released milestones. The dispute timeout is set by the constructor's `settings.dispute_timeout_seconds` (non-zero; 30 days is the usual value). If neither party gets an arbitration decision by that deadline, either party may call `escalate_dispute` to return the milestone to its pre-dispute status; a restored delivered milestone gets a fresh auto-release window
- `cancel_escrow` is a two-party handshake and a single-party approval now expires after `CANCEL_APPROVAL_TTL_SECONDS` (7 days, issue #713): an approval older than that is treated as absent, so the counter-party's call records a fresh first approval instead of locking the escrow. `CancelApprovalRecordedEvent` (`cxlappr`) carries `approved_at` so indexers can track the same window
- `lock_escrow_interest_yield` now rejects unauthorized callers with `Unauthorized` and an illegal source state with `InvalidStatus` **before** it reads or writes the ledger entry (issue #464); authorization runs first via `require_admin_from_instance`, so a non-admin cannot probe whether an escrow is locked, and an already-locked configuration is refused rather than silently re-locked
- Contract tests and snapshots are provided under `contracts/milestone-escrow/test_snapshots`

## Deploy (Testnet)

`scripts/deploy-testnet.sh` builds the reproducible wasm, uploads it, deploys
with constructor arguments from environment variables, and appends
`{date, network, wasm_hash, contract_id, admin}` to `deployments/testnet.json`
(issue #745):

```bash
export SOURCE_ACCOUNT=testnet-deployer
export ADMIN_ADDRESS=G...
export CLIENT_ADDRESS=G...
export FREELANCER_ADDRESS=G...
export ARBITER_ADDRESS=G...
export TOKEN_ADDRESS=C...
export AUTO_RELEASE_SECONDS=2592000
export MILESTONE_AMOUNTS='["10000000"]'
export MILESTONE_SPEC_HASHES='["0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"]'
# Optional: FUNDING_MODE (Upfront), DISPUTE_TIMEOUT_SECONDS (2592000), ARBITER_FEE_BPS (0)
scripts/deploy-testnet.sh
```

The equivalent manual deploy:

```bash
scripts/build-reproducible.sh
stellar contract deploy \
  --wasm dist/milestone_escrow.wasm \
  --network testnet \
  --source <your-account> \
  -- \
  --admin <admin-address> \
  --client <client-address> \
  --freelancer <freelancer-address> \
  --arbiter <arbiter-address> \
  --token <token-address> \
  --auto_release_seconds 604800 \
  --milestone_amounts '["1000000"]' \
  --milestone_spec_hashes '["0000000000000000000000000000000000000000000000000000000000000000"]' \
  --settings '{"funding_mode":"Upfront","dispute_timeout_seconds":2592000,"arbiter_fee_bps":0}'
```

Record the `source_commit` and `wasm_sha256` from `dist/build-info.json` with
the deployment, and the hash in `CHANGELOG.md`.

After upgrading an existing deployment to this version, the admin should call
`migrate_cold_config` once to move the whitelist, multisig, yield and fee
configuration out of instance storage (issue #772). The contract keeps working
before that call; the migration only makes every later call cheaper.

## Verifying the deployed contract

Anyone can check which source a deployed contract was built from:

1. Read the installed wasm hash:

   ```bash
   stellar contract fetch --id <contract-id> --network testnet --out-file deployed.wasm
   sha256sum deployed.wasm
   ```

   (`stellar contract info` and the explorer show the same hash.)
2. Look up `source_commit` for that contract in `deployments/<network>.json`.
3. Rebuild that commit and compare:

   ```bash
   scripts/build-reproducible.sh <source_commit>
   cat dist/milestone_escrow.wasm.sha256
   ```

The two hashes must be identical. If they differ, the deployed code is not the
reviewed source at that commit.

## License

MIT

## Deployed Contract

| Network | Contract ID |
|---|---|
| Testnet | `CDD5WKK3WT3QVKXMXTJNDIXE4T73FK6GGXDSD6UTJAH6YYZU52SQ4MUH` |

Deployment records, including the installed wasm hash, are in
[`deployments/`](deployments/). The current testnet deployment predates
reproducible builds, so its source commit is unknown.

Explorer: `https://stellar.expert/explorer/testnet/contract/CDD5WKK3WT3QVKXMXTJNDIXE4T73FK6GGXDSD6UTJAH6YYZU52SQ4MUH`
