#![cfg(test)]
//! Error-path coverage for `add_whitelisted_token` and
//! `remove_whitelisted_token` (issue #749).
//!
//! Every rejected call must return its typed error and leave
//! `get_whitelisted_tokens()` exactly as it was before the call.
//!
//! Both endpoints re-read the persistent `Admin` key right after
//! `require_admin`, which has already read and compared that same key. The
//! "persistent admin missing" and "persistent admin mismatch" branches of that
//! second read therefore cannot be reached through the contract interface and
//! have no test here; the mismatch case is exercised through `require_admin`.

use super::*;
use crate::{DataKey, Error};
use soroban_sdk::testutils::EnvTestConfig;
use soroban_sdk::{vec, Address, Env, Vec};

const ZERO_ACCOUNT: &str = "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF";
const ZERO_CONTRACT: &str = "CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABSC4";

fn test_env() -> Env {
    Env::new_with_config(EnvTestConfig {
        capture_snapshot_at_drop: false,
    })
}

// The whitelist is cold configuration kept in persistent storage (#789);
// the contract still falls back to a legacy instance copy, so clear both.
fn set_whitelist(env: &Env, contract_id: &Address, whitelist: &Vec<Address>) {
    env.as_contract(contract_id, || {
        env.storage()
            .persistent()
            .set(&DataKey::WhitelistedTokens, whitelist);
        env.storage().instance().remove(&DataKey::WhitelistedTokens);
    });
}

fn remove_whitelist_entry(env: &Env, contract_id: &Address) {
    env.as_contract(contract_id, || {
        env.storage()
            .persistent()
            .remove(&DataKey::WhitelistedTokens);
        env.storage().instance().remove(&DataKey::WhitelistedTokens);
    });
}

// ── add_whitelisted_token ────────────────────────────────────────────────────

#[test]
fn add_rejects_non_admin() {
    let env = test_env();
    env.mock_all_auths();

    let (_, client_addr, _, _, _, _, client) = setup_multisig_env(&env);
    let before = client.get_whitelisted_tokens();

    assert_eq!(
        client.try_add_whitelisted_token(&client_addr, &Address::generate(&env)),
        Err(Ok(Error::Unauthorized))
    );
    assert_eq!(client.get_whitelisted_tokens(), before);
}

#[test]
fn add_rejects_uninitialised_contract() {
    let env = test_env();
    env.mock_all_auths();

    let contract_id = env.register(MilestoneEscrow, ());
    let client = MilestoneEscrowClient::new(&env, &contract_id);

    assert_eq!(
        client.try_add_whitelisted_token(&Address::generate(&env), &Address::generate(&env)),
        Err(Ok(Error::NotInitialized))
    );
    assert_eq!(
        client.try_get_whitelisted_tokens(),
        Err(Ok(Error::NotInitialized))
    );
}

#[test]
fn add_rejects_sentinel_and_self_addresses() {
    let env = test_env();
    env.mock_all_auths();

    let (admin_addr, _, _, _, _, contract_id, client) = setup_multisig_env(&env);
    let before = client.get_whitelisted_tokens();

    for token in [
        Address::from_str(&env, ZERO_ACCOUNT),
        Address::from_str(&env, ZERO_CONTRACT),
        contract_id.clone(),
    ] {
        assert_eq!(
            client.try_add_whitelisted_token(&admin_addr, &token),
            Err(Ok(Error::InvalidAddress))
        );
        assert_eq!(client.get_whitelisted_tokens(), before);
    }
}

#[test]
fn add_rejects_after_funding() {
    let env = test_env();
    env.mock_all_auths();

    let (_, _, _, admin_addr, _, _, client) = setup_funded_escrow(&env, vec![&env, 1_000_i128]);
    let before = client.get_whitelisted_tokens();

    assert_eq!(
        client.try_add_whitelisted_token(&admin_addr, &Address::generate(&env)),
        Err(Ok(Error::AlreadyFunded))
    );
    assert_eq!(client.get_whitelisted_tokens(), before);
}

#[test]
fn add_rejects_missing_whitelist() {
    let env = test_env();
    env.mock_all_auths();

    let (admin_addr, _, _, _, _, contract_id, client) = setup_multisig_env(&env);
    remove_whitelist_entry(&env, &contract_id);

    assert_eq!(
        client.try_add_whitelisted_token(&admin_addr, &Address::generate(&env)),
        Err(Ok(Error::NotInitialized))
    );
    assert_eq!(
        client.try_get_whitelisted_tokens(),
        Err(Ok(Error::NotInitialized))
    );
}

#[test]
fn add_rejects_duplicate_token() {
    let env = test_env();
    env.mock_all_auths();

    let (admin_addr, _, _, _, token_contract_id, _, client) = setup_multisig_env(&env);
    let before = client.get_whitelisted_tokens();

    assert_eq!(
        client.try_add_whitelisted_token(&admin_addr, &token_contract_id),
        Err(Ok(Error::TokenAlreadyWhitelisted))
    );
    assert_eq!(client.get_whitelisted_tokens(), before);
}

#[test]
fn add_rejects_when_whitelist_is_full() {
    let env = test_env();
    env.mock_all_auths();

    let (admin_addr, _, _, _, _, contract_id, client) = setup_multisig_env(&env);
    let mut full: Vec<Address> = Vec::new(&env);
    for _ in 0..50 {
        full.push_back(Address::generate(&env));
    }
    set_whitelist(&env, &contract_id, &full);

    assert_eq!(
        client.try_add_whitelisted_token(&admin_addr, &Address::generate(&env)),
        Err(Ok(Error::InvalidAmount))
    );
    assert_eq!(client.get_whitelisted_tokens(), full);
}

// ── remove_whitelisted_token ─────────────────────────────────────────────────

#[test]
fn remove_rejects_non_admin() {
    let env = test_env();
    env.mock_all_auths();

    let (_, client_addr, _, _, token_contract_id, _, client) = setup_multisig_env(&env);
    let before = client.get_whitelisted_tokens();

    assert_eq!(
        client.try_remove_whitelisted_token(&client_addr, &token_contract_id),
        Err(Ok(Error::Unauthorized))
    );
    assert_eq!(client.get_whitelisted_tokens(), before);
}

#[test]
fn remove_rejects_uninitialised_contract() {
    let env = test_env();
    env.mock_all_auths();

    let contract_id = env.register(MilestoneEscrow, ());
    let client = MilestoneEscrowClient::new(&env, &contract_id);

    assert_eq!(
        client.try_remove_whitelisted_token(&Address::generate(&env), &Address::generate(&env)),
        Err(Ok(Error::NotInitialized))
    );
    assert_eq!(
        client.try_get_whitelisted_tokens(),
        Err(Ok(Error::NotInitialized))
    );
}

#[test]
fn remove_rejects_after_funding() {
    let env = test_env();
    env.mock_all_auths();

    let (_, _, _, admin_addr, token_contract_id, _, client) =
        setup_funded_escrow(&env, vec![&env, 1_000_i128]);
    let before = client.get_whitelisted_tokens();

    assert_eq!(
        client.try_remove_whitelisted_token(&admin_addr, &token_contract_id),
        Err(Ok(Error::AlreadyFunded))
    );
    assert_eq!(client.get_whitelisted_tokens(), before);
}

#[test]
fn remove_rejects_sentinel_addresses() {
    let env = test_env();
    env.mock_all_auths();

    let (admin_addr, _, _, _, _, _, client) = setup_multisig_env(&env);
    let before = client.get_whitelisted_tokens();

    for token in [
        Address::from_str(&env, ZERO_ACCOUNT),
        Address::from_str(&env, ZERO_CONTRACT),
    ] {
        assert_eq!(
            client.try_remove_whitelisted_token(&admin_addr, &token),
            Err(Ok(Error::InvalidAddress))
        );
        assert_eq!(client.get_whitelisted_tokens(), before);
    }
}

#[test]
fn remove_rejects_missing_whitelist() {
    let env = test_env();
    env.mock_all_auths();

    let (admin_addr, _, _, _, token_contract_id, contract_id, client) = setup_multisig_env(&env);
    remove_whitelist_entry(&env, &contract_id);

    assert_eq!(
        client.try_remove_whitelisted_token(&admin_addr, &token_contract_id),
        Err(Ok(Error::NotInitialized))
    );
    assert_eq!(
        client.try_get_whitelisted_tokens(),
        Err(Ok(Error::NotInitialized))
    );
}

#[test]
fn remove_rejects_empty_whitelist() {
    let env = test_env();
    env.mock_all_auths();

    let (admin_addr, _, _, _, token_contract_id, contract_id, client) = setup_multisig_env(&env);
    let empty: Vec<Address> = Vec::new(&env);
    set_whitelist(&env, &contract_id, &empty);

    assert_eq!(
        client.try_remove_whitelisted_token(&admin_addr, &token_contract_id),
        Err(Ok(Error::TokenNotWhitelisted))
    );
    assert_eq!(client.get_whitelisted_tokens(), empty);
}

#[test]
fn remove_rejects_last_remaining_token() {
    let env = test_env();
    env.mock_all_auths();

    let (admin_addr, _, _, _, token_contract_id, _, client) = setup_multisig_env(&env);
    let before = client.get_whitelisted_tokens();
    assert_eq!(before, vec![&env, token_contract_id.clone()]);

    assert_eq!(
        client.try_remove_whitelisted_token(&admin_addr, &token_contract_id),
        Err(Ok(Error::InvalidAmount))
    );
    assert_eq!(client.get_whitelisted_tokens(), before);
}

#[test]
fn remove_rejects_token_not_in_whitelist() {
    let env = test_env();
    env.mock_all_auths();

    let (admin_addr, _, _, _, _, _, client) = setup_multisig_env(&env);
    client.add_whitelisted_token(&admin_addr, &Address::generate(&env));
    let before = client.get_whitelisted_tokens();

    assert_eq!(
        client.try_remove_whitelisted_token(&admin_addr, &Address::generate(&env)),
        Err(Ok(Error::TokenNotWhitelisted))
    );
    assert_eq!(client.get_whitelisted_tokens(), before);
}
