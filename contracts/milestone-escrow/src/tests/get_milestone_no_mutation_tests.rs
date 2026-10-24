#![cfg(test)]
//! Read-only and storage-footprint guarantees for `get_milestone`.

use crate::test::setup_funded_escrow;
use crate::{Error, MilestoneEscrow, MilestoneEscrowClient};
use soroban_sdk::testutils::EnvTestConfig;
use soroban_sdk::{vec, Env};

fn env_without_snapshot() -> Env {
    Env::new_with_config(EnvTestConfig {
        capture_snapshot_at_drop: false,
    })
}

#[test]
fn get_milestone_returns_one_milestone_without_mutating_ledger() {
    let env = Env::default();
    env.mock_all_auths();
    let (_, _, _, _, _, _, escrow) =
        setup_funded_escrow(&env, vec![&env, 1_000_i128, 2_000, 3_000]);

    let before = env.to_ledger_snapshot();
    let milestone = escrow.get_milestone(&1);
    assert_eq!(milestone.amount, 2_000);
    assert!(milestone.funded);
    assert_eq!(crate::all_event_tuples(&env).len(), 0);
    let after = env.to_ledger_snapshot();
    assert_eq!(before, after, "get_milestone mutated the ledger");
}

#[test]
fn get_milestone_rejects_out_of_range_index_without_mutation() {
    let env = Env::default();
    env.mock_all_auths();
    let (_, _, _, _, _, _, escrow) = setup_funded_escrow(&env, vec![&env, 1_000_i128]);

    let before = env.to_ledger_snapshot();
    assert_eq!(
        escrow.try_get_milestone(&1),
        Err(Ok(Error::InvalidMilestone))
    );
    let after = env.to_ledger_snapshot();
    assert_eq!(before, after, "a rejected get_milestone mutated the ledger");
}

#[test]
fn get_milestone_reads_only_job_meta_and_one_milestone_entry() {
    let env = env_without_snapshot();
    env.mock_all_auths();
    let (_, _, _, _, _, _, escrow) =
        setup_funded_escrow(&env, vec![&env, 1_000_i128, 2_000, 3_000, 4_000]);

    assert_eq!(escrow.get_milestone(&3).amount, 4_000);
    let resources = env.cost_estimate().resources();
    assert_eq!(
        resources.memory_read_entries, 2,
        "one contract-instance entry containing Job metadata plus one persistent Milestone entry; measured {resources:?}"
    );
    assert_eq!(resources.write_entries, 0, "a read path writes nothing");
    assert_eq!(resources.disk_read_entries, 0);
}

#[test]
fn milestone_count_returns_metadata_count_and_requires_initialization() {
    let env = Env::default();
    env.mock_all_auths();
    let (_, _, _, _, _, _, escrow) =
        setup_funded_escrow(&env, vec![&env, 1_000_i128, 2_000, 3_000]);
    assert_eq!(escrow.milestone_count(), 3);

    let contract_id = env.register(MilestoneEscrow, ());
    let uninitialized = MilestoneEscrowClient::new(&env, &contract_id);
    assert_eq!(
        uninitialized.try_milestone_count(),
        Err(Ok(Error::NotInitialized))
    );
}
