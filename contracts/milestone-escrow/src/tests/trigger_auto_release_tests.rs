#![cfg(test)]
//! Tests for the permissionless `trigger_auto_release` (#740).
//!
//! The auto-release payout always goes to the job's freelancer, so anyone
//! may trigger it once the deadline has passed. Every test here calls the
//! trigger itself with **no** auth mocked: setup steps that need a signature
//! mock it on that one call only.

use crate::testutils::{EscrowFixture, DEFAULT_AUTO_RELEASE_SECONDS};
use crate::{ClaimedEvent, Error, MilestoneStatus};
use soroban_sdk::testutils::{Address as _, Ledger};
use soroban_sdk::{symbol_short, Address, Env, FromVal, IntoVal, Val};

fn past_deadline(env: &Env) {
    env.ledger()
        .with_mut(|l| l.timestamp += DEFAULT_AUTO_RELEASE_SECONDS + 1);
}

fn claim_events(env: &Env) -> std::vec::Vec<ClaimedEvent> {
    let claim_topic: Val = symbol_short!("claim").into_val(env);
    crate::all_event_tuples(env)
        .into_iter()
        .filter(|e| {
            e.1.get(0)
                .is_some_and(|t| t.get_payload() == claim_topic.get_payload())
        })
        .map(|e| ClaimedEvent::from_val(env, &e.2))
        .collect()
}

#[test]
fn trigger_with_no_auths_releases_to_freelancer_after_deadline() {
    let env = Env::default();
    let fx = EscrowFixture::new(&env)
        .milestones(&[1_000])
        .funded()
        .build();
    fx.escrow
        .mock_all_auths()
        .mark_delivered(&fx.freelancer, &0u32);
    past_deadline(&env);

    let keeper = Address::generate(&env);
    fx.escrow.trigger_auto_release(&keeper, &0u32);

    // No signature was requested from anyone.
    assert!(env.auths().is_empty());

    // The claim event records who triggered it and who was paid. Read it
    // before any other call replaces the last invocation's events.
    let events = claim_events(&env);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].caller, keeper);
    assert_eq!(events[0].freelancer, fx.freelancer);
    assert_eq!(events[0].amount, 1_000);

    let token = fx.token_client();
    assert_eq!(token.balance(&fx.freelancer), 1_000);
    assert_eq!(token.balance(&keeper), 0);
    assert_eq!(token.balance(&fx.contract_id), 0);
    let ms = fx.escrow.get_job().milestones.get(0).unwrap();
    assert_eq!(ms.status, MilestoneStatus::Released);
    assert_eq!(ms.released_amount, 1_000);
}

#[test]
fn trigger_before_deadline_fails_and_moves_nothing() {
    let env = Env::default();
    let fx = EscrowFixture::new(&env)
        .milestones(&[1_000])
        .funded()
        .build();
    fx.escrow
        .mock_all_auths()
        .mark_delivered(&fx.freelancer, &0u32);
    env.ledger()
        .with_mut(|l| l.timestamp += DEFAULT_AUTO_RELEASE_SECONDS - 1);

    let keeper = Address::generate(&env);
    let result = fx.escrow.try_trigger_auto_release(&keeper, &0u32);

    assert_eq!(result, Err(Ok(Error::DeadlineNotPassed)));
    assert_eq!(fx.token_client().balance(&fx.freelancer), 0);
    assert_eq!(fx.token_client().balance(&fx.contract_id), 1_000);
    let ms = fx.escrow.get_job().milestones.get(0).unwrap();
    assert_eq!(ms.status, MilestoneStatus::Delivered);
}

#[test]
fn trigger_honours_a_custom_auto_release_window() {
    let env = Env::default();
    let fx = EscrowFixture::new(&env)
        .milestones(&[1_000])
        .auto_release_seconds(60)
        .funded()
        .build();
    fx.escrow
        .mock_all_auths()
        .mark_delivered(&fx.freelancer, &0u32);
    let keeper = Address::generate(&env);

    env.ledger().with_mut(|l| l.timestamp += 59);
    assert_eq!(
        fx.escrow.try_trigger_auto_release(&keeper, &0u32),
        Err(Ok(Error::DeadlineNotPassed))
    );

    env.ledger().with_mut(|l| l.timestamp += 1);
    fx.escrow.trigger_auto_release(&keeper, &0u32);
    assert_eq!(fx.token_client().balance(&fx.freelancer), 1_000);
}

#[test]
fn trigger_rejects_undelivered_milestone() {
    let env = Env::default();
    let fx = EscrowFixture::new(&env)
        .milestones(&[1_000])
        .funded()
        .build();
    past_deadline(&env);

    let keeper = Address::generate(&env);
    assert_eq!(
        fx.escrow.try_trigger_auto_release(&keeper, &0u32),
        Err(Ok(Error::InvalidStatus))
    );
}

#[test]
fn trigger_rejects_out_of_range_index() {
    let env = Env::default();
    let fx = EscrowFixture::new(&env)
        .milestones(&[1_000])
        .funded()
        .build();

    let keeper = Address::generate(&env);
    assert_eq!(
        fx.escrow.try_trigger_auto_release(&keeper, &5u32),
        Err(Ok(Error::InvalidMilestone))
    );
}

#[test]
fn second_trigger_is_rejected() {
    let env = Env::default();
    let fx = EscrowFixture::new(&env)
        .milestones(&[1_000])
        .funded()
        .build();
    fx.escrow
        .mock_all_auths()
        .mark_delivered(&fx.freelancer, &0u32);
    past_deadline(&env);

    let keeper = Address::generate(&env);
    fx.escrow.trigger_auto_release(&keeper, &0u32);
    assert_eq!(
        fx.escrow.try_trigger_auto_release(&keeper, &0u32),
        Err(Ok(Error::InvalidStatus))
    );
    assert_eq!(fx.token_client().balance(&fx.freelancer), 1_000);
}

#[test]
fn trigger_is_blocked_while_paused() {
    let env = Env::default();
    let fx = EscrowFixture::new(&env)
        .milestones(&[1_000])
        .funded()
        .build();
    fx.escrow
        .mock_all_auths()
        .mark_delivered(&fx.freelancer, &0u32);
    fx.escrow
        .mock_all_auths()
        .emergency_pause(&fx.client, &fx.freelancer);
    past_deadline(&env);

    let keeper = Address::generate(&env);
    assert_eq!(
        fx.escrow.try_trigger_auto_release(&keeper, &0u32),
        Err(Ok(Error::Paused))
    );
}

#[test]
fn claim_auto_release_records_freelancer_as_caller() {
    let env = Env::default();
    env.mock_all_auths();
    let fx = EscrowFixture::new(&env)
        .milestones(&[1_000])
        .funded()
        .build();
    fx.escrow.mark_delivered(&fx.freelancer, &0u32);
    past_deadline(&env);

    fx.escrow.claim_auto_release(&fx.freelancer, &0u32);

    let events = claim_events(&env);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].caller, fx.freelancer);
    assert_eq!(events[0].freelancer, fx.freelancer);
}
