#![cfg(test)]
//! After funding, no admin-only path may change the yield shares (issue #776).
//!
//! * `set_escrow_interest_yield` (admin only) is open before funding and
//!   refused with `AlreadyFunded` afterwards.
//! * `set_interest_yield_consent` (admin + client + freelancer) remains the
//!   way to change the split after funding.
//! * `unlock_escrow_interest_yield` needs the same three signatures, so the
//!   admin cannot undo its own lock and then rewrite the split.

use crate::{DataKey, Error, EscrowInterestYieldState, MilestoneEscrow, MilestoneEscrowClient};
use soroban_sdk::testutils::{Address as _, EnvTestConfig, MockAuth, MockAuthInvoke};
use soroban_sdk::{token, vec, Address, Env, IntoVal};

struct Setup<'a> {
    escrow: MilestoneEscrowClient<'a>,
    contract_id: Address,
    admin: Address,
    client: Address,
}

fn test_env() -> Env {
    Env::new_with_config(EnvTestConfig {
        capture_snapshot_at_drop: false,
    })
}

/// Initialised but not yet funded escrow.
fn setup(env: &Env) -> Setup<'_> {
    env.mock_all_auths();
    let admin = Address::generate(env);
    let client = Address::generate(env);
    let freelancer = Address::generate(env);
    let arbiter = Address::generate(env);
    let token = env
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    token::StellarAssetClient::new(env, &token).mint(&client, &1_000);
    let contract_id = env.register(MilestoneEscrow, ());
    let escrow = MilestoneEscrowClient::new(env, &contract_id);
    escrow.initialize(
        &admin,
        &client,
        &freelancer,
        &arbiter,
        &token,
        &604800u64,
        &vec![env, 1_000_i128],
        &crate::spec_hashes_for(&vec![env, 1_000_i128]),
    );
    Setup {
        escrow,
        contract_id,
        admin,
        client,
    }
}

fn stored_state(env: &Env, contract_id: &Address) -> Option<EscrowInterestYieldState> {
    env.as_contract(contract_id, || {
        env.storage().persistent().get(&DataKey::InterestYieldState)
    })
}

#[test]
fn admin_can_set_yield_shares_before_funding() {
    let env = test_env();
    let s = setup(&env);

    assert_eq!(
        s.escrow
            .try_set_escrow_interest_yield(&s.admin, &6_000u32, &4_000u32),
        Ok(Ok(()))
    );
    let state = stored_state(&env, &s.contract_id).unwrap();
    assert_eq!(state.client_share_bps, 6_000);
    assert_eq!(state.freelancer_share_bps, 4_000);
}

#[test]
fn admin_cannot_set_yield_shares_after_funding() {
    let env = test_env();
    let s = setup(&env);
    s.escrow
        .set_escrow_interest_yield(&s.admin, &5_000u32, &5_000u32);
    s.escrow.fund(&s.client);

    let before = stored_state(&env, &s.contract_id);
    assert_eq!(
        s.escrow
            .try_set_escrow_interest_yield(&s.admin, &10_000u32, &0u32),
        Err(Ok(Error::AlreadyFunded))
    );
    assert_eq!(stored_state(&env, &s.contract_id), before);
}

#[test]
fn admin_cannot_create_yield_shares_after_funding() {
    let env = test_env();
    let s = setup(&env);
    s.escrow.fund(&s.client);

    assert_eq!(
        s.escrow
            .try_set_escrow_interest_yield(&s.admin, &10_000u32, &0u32),
        Err(Ok(Error::AlreadyFunded))
    );
    assert_eq!(stored_state(&env, &s.contract_id), None);
}

#[test]
fn parties_can_still_change_yield_shares_after_funding() {
    let env = test_env();
    let s = setup(&env);
    s.escrow
        .set_escrow_interest_yield(&s.admin, &5_000u32, &5_000u32);
    s.escrow.fund(&s.client);

    assert_eq!(
        s.escrow
            .try_set_interest_yield_consent(&s.admin, &7_000u32, &3_000u32),
        Ok(Ok(()))
    );
    let state = stored_state(&env, &s.contract_id).unwrap();
    assert_eq!(state.client_share_bps, 7_000);
    assert_eq!(state.freelancer_share_bps, 3_000);
}

#[test]
fn admin_alone_cannot_unlock_yield_shares() {
    let env = test_env();
    let s = setup(&env);
    s.escrow
        .set_escrow_interest_yield(&s.admin, &5_000u32, &5_000u32);
    s.escrow.fund(&s.client);
    s.escrow.lock_escrow_interest_yield(&s.admin);

    // Only the admin signs: the client/freelancer `require_auth` must fail.
    env.mock_auths(&[MockAuth {
        address: &s.admin,
        invoke: &MockAuthInvoke {
            contract: &s.contract_id,
            fn_name: "unlock_escrow_interest_yield",
            args: (s.admin.clone(),).into_val(&env),
            sub_invokes: &[],
        },
    }]);
    assert!(s.escrow.try_unlock_escrow_interest_yield(&s.admin).is_err());
    assert!(stored_state(&env, &s.contract_id).unwrap().locked);
}

#[test]
fn admin_and_parties_together_can_unlock_yield_shares() {
    let env = test_env();
    let s = setup(&env);
    s.escrow
        .set_escrow_interest_yield(&s.admin, &5_000u32, &5_000u32);
    s.escrow.fund(&s.client);
    s.escrow.lock_escrow_interest_yield(&s.admin);

    assert_eq!(
        s.escrow.try_unlock_escrow_interest_yield(&s.admin),
        Ok(Ok(()))
    );
    assert!(!stored_state(&env, &s.contract_id).unwrap().locked);

    // Unlocking does not reopen the admin-only path after funding.
    assert_eq!(
        s.escrow
            .try_set_escrow_interest_yield(&s.admin, &10_000u32, &0u32),
        Err(Ok(Error::AlreadyFunded))
    );
}
