//! Upgrade-test fixture compiled to `testdata/escrow_v2.wasm` (issue #752).
//!
//! Links in the full `milestone-escrow` contract, so every v1 entrypoint and
//! storage layout carries over, and adds two entrypoints v1 does not have:
//!
//! * `v2_only` – returns `2`; proves the upgraded code is what runs.
//! * `milestone_v2` – reads `DataKey::Milestone` with a changed layout (an
//!   extra field), showing how a storage-incompatible upgrade fails.
//!
//! Rebuild after changing the escrow's storage layout:
//!
//! ```sh
//! stellar contract build --manifest-path contracts/milestone-escrow/testdata/escrow_v2/Cargo.toml
//! cp contracts/milestone-escrow/testdata/escrow_v2/target/wasm32v1-none/release/escrow_v2.wasm \
//!    contracts/milestone-escrow/testdata/escrow_v2.wasm
//! ```
#![no_std]

use milestone_escrow::{DataKey, MilestoneStatus};
use soroban_sdk::{contract, contractimpl, contracttype, Env};

/// `Milestone` with an added field: v1 entries do not decode into it.
#[contracttype]
pub struct MilestoneV2 {
    pub amount: i128,
    pub released_amount: i128,
    pub status: MilestoneStatus,
    pub delivered_at: u64,
    pub deadline: u64,
}

#[contract]
pub struct EscrowV2;

#[contractimpl]
impl EscrowV2 {
    pub fn v2_only(_env: Env) -> u32 {
        2
    }

    pub fn milestone_v2(env: Env, milestone_index: u32) -> Option<MilestoneV2> {
        env.storage()
            .persistent()
            .get(&DataKey::Milestone(milestone_index))
    }
}
