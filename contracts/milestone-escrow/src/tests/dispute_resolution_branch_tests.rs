//! Branch coverage for the dispute resolution paths (issue #751):
//! `resolve_dispute`, `apply_dispute_arbitration_split`, and the
//! `DisputeAlreadyRaised` guard in `raise_dispute`.
//!
//! The shortfall tests move tokens out of the escrow before resolving so the
//! contract balance is below the milestone's remaining amount, and pin the
//! current capping behaviour.

use super::*;
use crate::{DataKey, Error, Milestone, MilestoneStatus};
use soroban_sdk::{Address, Env};

const ZERO_ACCOUNT: &str = "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF";
const ZERO_CONTRACT: &str = "CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABSC4";

/// Fund a single-milestone escrow and raise a dispute on milestone 0.
/// Returns `(client_addr, freelancer_addr, arbiter_addr, token, contract_id, client)`.
fn disputed_escrow(
    env: &Env,
    amount: i128,
) -> (
    Address,
    Address,
    Address,
    Address,
    Address,
    MilestoneEscrowClient<'_>,
) {
    let (client_addr, freelancer_addr, arbiter_addr, _admin, token, contract_id, client) =
        setup_funded_escrow(env, vec![env, amount]);
    client.mark_delivered(&freelancer_addr, &0u32);
    client.raise_dispute(&client_addr, &0u32, &dispute_reason_hash(env));
    (
        client_addr,
        freelancer_addr,
        arbiter_addr,
        token,
        contract_id,
        client,
    )
}

/// Move `amount` out of the escrow so its balance falls below what is owed.
fn drain(env: &Env, token: &Address, contract_id: &Address, amount: i128) {
    token::Client::new(env, token).transfer(contract_id, Address::generate(env), &amount);
}

/// Mark milestone 0 as fully released while leaving it `Disputed`, so the
/// remaining balance is zero.
fn zero_remaining(env: &Env, contract_id: &Address) {
    env.as_contract(contract_id, || {
        let mut milestone: Milestone = env
            .storage()
            .persistent()
            .get(&DataKey::Milestone(0u32))
            .unwrap();
        milestone.released_amount = milestone.amount;
        env.storage()
            .persistent()
            .set(&DataKey::Milestone(0u32), &milestone);
    });
}

fn milestone_0(client: &MilestoneEscrowClient<'_>) -> Milestone {
    client.get_job().milestones.get(0).unwrap()
}

// ── raise_dispute ───────────────────────────────────────────────────────────

#[test]
fn raise_dispute_rejects_when_dispute_lock_is_held() {
    let env = Env::default();
    env.mock_all_auths();

    let (client_addr, _, _, _, _, contract_id, client) =
        setup_funded_escrow(&env, vec![&env, 1_000_i128]);

    env.as_contract(&contract_id, || {
        env.storage()
            .temporary()
            .set(&DataKey::DisputeLock(0u32), &true);
    });

    let result = client.try_raise_dispute(&client_addr, &0u32, &dispute_reason_hash(&env));
    assert_eq!(result, Err(Ok(Error::DisputeAlreadyRaised)));
    assert_eq!(milestone_0(&client).status, MilestoneStatus::Pending);
}

// ── resolve_dispute ─────────────────────────────────────────────────────────

#[test]
fn resolve_dispute_rejects_zero_remaining() {
    let env = Env::default();
    env.mock_all_auths();

    let (_, _, arbiter_addr, _, contract_id, client) = disputed_escrow(&env, 1_000);
    zero_remaining(&env, &contract_id);

    let result = client.try_resolve_dispute(&arbiter_addr, &0u32, &true);
    assert_eq!(result, Err(Ok(Error::InvalidAmount)));
    assert_eq!(milestone_0(&client).status, MilestoneStatus::Disputed);
}

#[test]
fn resolve_dispute_rejects_zero_contract_balance() {
    let env = Env::default();
    env.mock_all_auths();

    let (_, _, arbiter_addr, token, contract_id, client) = disputed_escrow(&env, 1_000);
    drain(&env, &token, &contract_id, 1_000);

    let result = client.try_resolve_dispute(&arbiter_addr, &0u32, &false);
    assert_eq!(result, Err(Ok(Error::InvalidAmount)));
    assert_eq!(milestone_0(&client).status, MilestoneStatus::Disputed);
}

#[test]
fn resolve_dispute_shortfall_caps_release_to_freelancer() {
    let env = Env::default();
    env.mock_all_auths();

    let (client_addr, freelancer_addr, arbiter_addr, token, contract_id, client) =
        disputed_escrow(&env, 1_000);
    drain(&env, &token, &contract_id, 700);

    client.resolve_dispute(&arbiter_addr, &0u32, &true);

    let token_client = token::Client::new(&env, &token);
    assert_eq!(token_client.balance(&freelancer_addr), 300);
    assert_eq!(token_client.balance(&contract_id), 0);

    let milestone = milestone_0(&client);
    assert_eq!(milestone.status, MilestoneStatus::Released);
    assert_eq!(milestone.released_amount, 300);
    assert_eq!(client.get_reputation(&client_addr), 1);
    assert_eq!(client.get_reputation(&freelancer_addr), 1);
}

#[test]
fn resolve_dispute_shortfall_caps_refund_to_client() {
    let env = Env::default();
    env.mock_all_auths();

    let (client_addr, freelancer_addr, arbiter_addr, token, contract_id, client) =
        disputed_escrow(&env, 1_000);
    drain(&env, &token, &contract_id, 600);

    client.resolve_dispute(&arbiter_addr, &0u32, &false);

    let token_client = token::Client::new(&env, &token);
    assert_eq!(token_client.balance(&client_addr), 400);
    assert_eq!(token_client.balance(&freelancer_addr), 0);
    assert_eq!(token_client.balance(&contract_id), 0);

    let milestone = milestone_0(&client);
    assert_eq!(milestone.status, MilestoneStatus::Refunded);
    assert_eq!(milestone.released_amount, 0);
}

// ── apply_dispute_arbitration_split ─────────────────────────────────────────

#[test]
fn split_rejects_zero_and_self_arbiter_addresses() {
    let env = Env::default();
    env.mock_all_auths();

    let (_, _, _, _, contract_id, client) = disputed_escrow(&env, 1_000);

    for bad in [
        Address::from_str(&env, ZERO_ACCOUNT),
        Address::from_str(&env, ZERO_CONTRACT),
        contract_id.clone(),
    ] {
        let result = client.try_apply_dispute_arbitration_split(&bad, &0u32, &5_000u32);
        assert_eq!(result, Err(Ok(Error::InvalidAddress)));
    }
    assert_eq!(milestone_0(&client).status, MilestoneStatus::Disputed);
}

#[test]
fn split_rejects_unfunded_escrow() {
    let env = Env::default();
    env.mock_all_auths();

    let client_addr = Address::generate(&env);
    let freelancer_addr = Address::generate(&env);
    let arbiter_addr = Address::generate(&env);
    let admin_addr = Address::generate(&env);
    let token = env
        .register_stellar_asset_contract_v2(admin_addr.clone())
        .address();

    let contract_id = env.register(MilestoneEscrow, ());
    let client = MilestoneEscrowClient::new(&env, &contract_id);
    client.initialize(
        &admin_addr,
        &client_addr,
        &freelancer_addr,
        &arbiter_addr,
        &token,
        &604800,
        &vec![&env, 1_000_i128],
        &crate::test_spec_hashes(&env, &vec![&env, 1_000_i128]),
    );

    let result = client.try_apply_dispute_arbitration_split(&arbiter_addr, &0u32, &5_000u32);
    assert_eq!(result, Err(Ok(Error::NotFunded)));
}

#[test]
fn split_rejects_non_disputed_milestone() {
    let env = Env::default();
    env.mock_all_auths();

    let (_, _, arbiter_addr, _, _, _, client) = setup_funded_escrow(&env, vec![&env, 1_000_i128]);

    let result = client.try_apply_dispute_arbitration_split(&arbiter_addr, &0u32, &5_000u32);
    assert_eq!(result, Err(Ok(Error::InvalidStatus)));
    assert_eq!(milestone_0(&client).status, MilestoneStatus::Pending);
}

#[test]
fn split_rejects_zero_remaining() {
    let env = Env::default();
    env.mock_all_auths();

    let (_, _, arbiter_addr, _, contract_id, client) = disputed_escrow(&env, 1_000);
    zero_remaining(&env, &contract_id);

    let result = client.try_apply_dispute_arbitration_split(&arbiter_addr, &0u32, &5_000u32);
    assert_eq!(result, Err(Ok(Error::InvalidAmount)));
    assert_eq!(milestone_0(&client).status, MilestoneStatus::Disputed);
}

#[test]
fn split_rejects_zero_contract_balance() {
    let env = Env::default();
    env.mock_all_auths();

    let (_, _, arbiter_addr, token, contract_id, client) = disputed_escrow(&env, 1_000);
    drain(&env, &token, &contract_id, 1_000);

    let result = client.try_apply_dispute_arbitration_split(&arbiter_addr, &0u32, &5_000u32);
    assert_eq!(result, Err(Ok(Error::InvalidAmount)));
    assert_eq!(milestone_0(&client).status, MilestoneStatus::Disputed);
}

#[test]
fn split_shortfall_pays_client_first_then_caps_freelancer() {
    let env = Env::default();
    env.mock_all_auths();

    let (client_addr, freelancer_addr, arbiter_addr, token, contract_id, client) =
        disputed_escrow(&env, 10_000);
    // Owed: 7_000 to the client, 3_000 to the freelancer. Only 8_000 remains.
    drain(&env, &token, &contract_id, 2_000);

    let alloc = client.apply_dispute_arbitration_split(&arbiter_addr, &0u32, &7_000u32);
    assert_eq!(alloc.client_refund, 7_000);
    assert_eq!(alloc.freelancer_payout, 1_000);

    let token_client = token::Client::new(&env, &token);
    assert_eq!(token_client.balance(&client_addr), 7_000);
    assert_eq!(token_client.balance(&freelancer_addr), 1_000);
    assert_eq!(token_client.balance(&contract_id), 0);

    let milestone = milestone_0(&client);
    assert_eq!(milestone.status, MilestoneStatus::Split);
    assert_eq!(milestone.released_amount, 1_000);
}

#[test]
fn split_shortfall_below_client_share_leaves_freelancer_unpaid() {
    let env = Env::default();
    env.mock_all_auths();

    let (client_addr, freelancer_addr, arbiter_addr, token, contract_id, client) =
        disputed_escrow(&env, 10_000);
    // Owed: 7_000 to the client, 3_000 to the freelancer. Only 5_000 remains.
    drain(&env, &token, &contract_id, 5_000);

    let alloc = client.apply_dispute_arbitration_split(&arbiter_addr, &0u32, &7_000u32);
    assert_eq!(alloc.client_refund, 5_000);
    assert_eq!(alloc.freelancer_payout, 0);

    let token_client = token::Client::new(&env, &token);
    assert_eq!(token_client.balance(&client_addr), 5_000);
    assert_eq!(token_client.balance(&freelancer_addr), 0);
    assert_eq!(token_client.balance(&contract_id), 0);

    let milestone = milestone_0(&client);
    assert_eq!(milestone.status, MilestoneStatus::Refunded);
    assert_eq!(milestone.released_amount, 0);
}
