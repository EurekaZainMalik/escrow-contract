#![cfg(test)]
use crate::{Error, InitializedEvent, MilestoneEscrow, MilestoneEscrowClient};
use soroban_sdk::{
    symbol_short, testutils::Address as _, testutils::EnvTestConfig, vec, Address, BytesN, Env,
    FromVal, IntoVal, Val,
};

fn test_env() -> Env {
    Env::new_with_config(EnvTestConfig {
        capture_snapshot_at_drop: false,
    })
}

fn initialize_args(env: &Env) -> (Address, Address, Address, Address, Address) {
    let admin = Address::generate(env);
    let client = Address::generate(env);
    let freelancer = Address::generate(env);
    let arbiter = Address::generate(env);
    let token = env
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    (admin, client, freelancer, arbiter, token)
}

#[test]
fn initialize_persists_and_emits_milestone_spec_hashes() {
    let env = test_env();
    env.mock_all_auths();
    let (admin, client, freelancer, arbiter, token) = initialize_args(&env);
    let contract_id = env.register(MilestoneEscrow, ());
    let escrow = MilestoneEscrowClient::new(&env, &contract_id);
    let amounts = vec![&env, 250_i128, 750_i128];
    let first_hash = BytesN::from_array(&env, &[0x11; 32]);
    let second_hash = BytesN::from_array(&env, &[0x22; 32]);
    let hashes = vec![&env, first_hash.clone(), second_hash.clone()];

    escrow.initialize(
        &admin,
        &client,
        &freelancer,
        &arbiter,
        &token,
        &604_800,
        &amounts,
        &hashes,
    );

    // Events cover only the latest invocation, so read them before the
    // getter calls below.
    let init_topic: Val = symbol_short!("init").into_val(&env);
    let events = crate::all_event_tuples(&env);
    let init = events
        .iter()
        .find(|e| {
            e.1.get(0)
                .is_some_and(|t| t.get_payload() == init_topic.get_payload())
        })
        .expect("initialize emits an init event");
    let event = InitializedEvent::from_val(&env, &init.2);
    assert_eq!(event.milestone_spec_hashes, hashes);

    assert_eq!(escrow.get_milestone_spec_hash(&0), first_hash);
    assert_eq!(escrow.get_milestone_spec_hash(&1), second_hash);
}

#[test]
fn initialize_rejects_spec_hash_count_mismatch_without_mutation() {
    let env = test_env();
    env.mock_all_auths();
    let (admin, client, freelancer, arbiter, token) = initialize_args(&env);
    let contract_id = env.register(MilestoneEscrow, ());
    let escrow = MilestoneEscrowClient::new(&env, &contract_id);
    let amounts = vec![&env, 250_i128, 750_i128];
    let hashes = vec![&env, BytesN::from_array(&env, &[0x11; 32])];
    let before = env.to_ledger_snapshot();

    assert_eq!(
        escrow.try_initialize(
            &admin,
            &client,
            &freelancer,
            &arbiter,
            &token,
            &604_800,
            &amounts,
            &hashes,
        ),
        Err(Ok(Error::InvalidAmount))
    );
    assert_eq!(env.to_ledger_snapshot(), before);
}

#[test]
fn initialize_spec_hashes_are_retrievable_only_for_existing_milestones() {
    let env = test_env();
    env.mock_all_auths();
    let (admin, client, freelancer, arbiter, token) = initialize_args(&env);
    let contract_id = env.register(MilestoneEscrow, ());
    let escrow = MilestoneEscrowClient::new(&env, &contract_id);
    let amounts = vec![&env, 1_i128];
    let hashes = vec![&env, BytesN::from_array(&env, &[0xAB; 32])];

    escrow.initialize(
        &admin,
        &client,
        &freelancer,
        &arbiter,
        &token,
        &604_800,
        &amounts,
        &hashes,
    );

    assert_eq!(
        escrow.try_get_milestone_spec_hash(&1),
        Err(Ok(Error::InvalidMilestone))
    );
}
