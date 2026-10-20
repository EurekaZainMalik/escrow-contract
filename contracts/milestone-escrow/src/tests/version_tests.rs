#![cfg(test)]
//! Regression suite for `version`, covering issues #558, #559 and the
//! contract-metadata fix.
//!
//! # #558 — zero state mutation
//! `version` is a public read path and must never write to instance,
//! persistent, or temporary storage, extend a TTL, or emit an event.  Each
//! test takes a full ledger snapshot (`Env::to_ledger_snapshot`, which
//! captures every entry across all storage durabilities plus live-until
//! ledgers) immediately before and after the call and asserts they are
//! identical.  Any future `.set(`, `.remove(`, or `.extend_ttl(` added to
//! `version` or `read_version` fails these tests.
//!
//! # #559 — documented return contract
//! `version` returns the compile-time `CONTRACT_VERSION` constant, so it is
//! infallible (no error variants) and independent of storage.  Each
//! documented case has a test below:
//!
//! | Documented case                  | Test                                              |
//! |----------------------------------|---------------------------------------------------|
//! | `CONTRACT_VERSION` after init    | `version_is_one_after_initialize`                 |
//! | `CONTRACT_VERSION` regardless of storage | `version_ignores_stored_marker`           |
//! | `CONTRACT_VERSION` when uninitialized | `version_on_uninitialized_contract_returns_one` |
//! | infallible — no error variants   | `try_version_never_errors`                        |
//! | matches `Cargo.toml`             | `version_matches_cargo_pkg_version`               |

use super::*;
use soroban_sdk::{vec, Address, Env};

fn initialized_escrow(env: &Env) -> (Address, MilestoneEscrowClient<'_>) {
    env.mock_all_auths();

    let admin_addr = Address::generate(env);
    let client_addr = Address::generate(env);
    let freelancer_addr = Address::generate(env);
    let arbiter_addr = Address::generate(env);

    let token_contract_id = env
        .register_stellar_asset_contract_v2(admin_addr.clone())
        .address();

    let contract_id = env.register(MilestoneEscrow, ());
    let escrow = MilestoneEscrowClient::new(env, &contract_id);

    escrow.initialize(
        &admin_addr,
        &client_addr,
        &freelancer_addr,
        &arbiter_addr,
        &token_contract_id,
        &604_800u64,
        &vec![env, 1_000_i128],
        &crate::test_spec_hashes(env, &vec![env, 1_000_i128]),
    );

    (contract_id, escrow)
}

// ── #558: zero state mutation ────────────────────────────────────────────────

#[test]
fn version_does_not_mutate_ledger_when_initialized() {
    let env = Env::default();
    let (_, escrow) = initialized_escrow(&env);

    let before = env.to_ledger_snapshot();
    let v = escrow.version();
    let after = env.to_ledger_snapshot();

    assert_eq!(v, CONTRACT_VERSION);
    assert_eq!(before, after, "version must not mutate the ledger");
}

#[test]
fn version_does_not_mutate_ledger_when_uninitialized() {
    let env = Env::default();
    let contract_id = env.register(MilestoneEscrow, ());
    let escrow = MilestoneEscrowClient::new(&env, &contract_id);

    let before = env.to_ledger_snapshot();
    let v = escrow.version();
    let after = env.to_ledger_snapshot();

    assert_eq!(v, CONTRACT_VERSION);
    assert_eq!(before, after, "version must not mutate the ledger");
    // Nothing must be materialized into storage.
    assert_eq!(env.storage().instance().get(&DataKey::Version), None);
}

#[test]
fn repeated_version_calls_do_not_mutate_ledger() {
    let env = Env::default();
    let (_, escrow) = initialized_escrow(&env);

    let before = env.to_ledger_snapshot();
    for _ in 0..5 {
        assert_eq!(escrow.version(), CONTRACT_VERSION);
    }
    let after = env.to_ledger_snapshot();

    assert_eq!(before, after);
}

/// A read must not extend TTLs: advance the ledger so any bump would move the
/// live-until ledger, then confirm the snapshot is still identical.
#[test]
fn version_does_not_extend_ttl() {
    use soroban_sdk::testutils::Ledger as _;

    let env = Env::default();
    let (_, escrow) = initialized_escrow(&env);

    env.ledger().with_mut(|li| li.sequence_number += 100);

    let before = env.to_ledger_snapshot();
    escrow.version();
    let after = env.to_ledger_snapshot();

    assert_eq!(before, after, "version must not extend any TTL");
}

#[test]
fn version_emits_no_events() {
    let env = Env::default();
    let (_, escrow) = initialized_escrow(&env);

    escrow.version();

    // The test env only records events from the latest invocation, so this
    // is exactly what `version` emitted.
    assert_eq!(crate::all_event_tuples(&env).len(), 0);
}

// ── #559: documented return contract ─────────────────────────────────────────

#[test]
fn version_is_one_after_initialize() {
    let env = Env::default();
    let (_, escrow) = initialized_escrow(&env);

    assert_eq!(escrow.version(), CONTRACT_VERSION);
}

/// `version` reports the compile-time constant, not the storage schema
/// marker, so seeding `DataKey::Version` must not change the result.
#[test]
fn version_ignores_stored_marker() {
    let env = Env::default();
    let (contract_id, escrow) = initialized_escrow(&env);

    env.as_contract(&contract_id, || {
        env.storage().instance().set(&DataKey::Version, &7u32);
    });

    let before = env.to_ledger_snapshot();
    assert_eq!(escrow.version(), CONTRACT_VERSION);
    assert_eq!(before, env.to_ledger_snapshot());
}

#[test]
fn version_on_uninitialized_contract_returns_one() {
    let env = Env::default();
    let contract_id = env.register(MilestoneEscrow, ());
    let escrow = MilestoneEscrowClient::new(&env, &contract_id);

    assert_eq!(escrow.version(), CONTRACT_VERSION);
}

/// The rustdoc lists no error variants; confirm none is reachable in any
/// documented state.
#[test]
fn try_version_never_errors() {
    let env = Env::default();

    let uninit_id = env.register(MilestoneEscrow, ());
    let uninit = MilestoneEscrowClient::new(&env, &uninit_id);
    assert_eq!(uninit.try_version(), Ok(Ok(CONTRACT_VERSION)));

    let (contract_id, escrow) = initialized_escrow(&env);
    assert_eq!(escrow.try_version(), Ok(Ok(CONTRACT_VERSION)));

    env.as_contract(&contract_id, || {
        env.storage().instance().set(&DataKey::Version, &u32::MAX);
    });
    assert_eq!(escrow.try_version(), Ok(Ok(CONTRACT_VERSION)));
}

/// `version()` must report the crate version from `Cargo.toml`.
#[test]
fn version_matches_cargo_pkg_version() {
    let env = Env::default();
    let (_, escrow) = initialized_escrow(&env);

    let expected: u32 = env!("CARGO_PKG_VERSION_MAJOR").parse().unwrap();
    assert_eq!(escrow.version(), expected);
    assert_eq!(escrow.version(), CONTRACT_VERSION);
}
