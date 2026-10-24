#![cfg(test)]
//! `is_multisig_locked` reports the multisig deadlock flag and distinguishes
//! an uninitialized contract (`NotInitialized`) from an initialized escrow
//! that was never locked (`false`).

use crate::testutils::EscrowFixture;
use crate::Error;
use soroban_sdk::Env;

#[test]
fn uninitialized_contract_returns_not_initialized() {
    let env = Env::default();
    let fx = EscrowFixture::new(&env).uninitialized().build();

    assert_eq!(
        fx.escrow.try_is_multisig_locked(),
        Err(Ok(Error::NotInitialized))
    );
}

#[test]
fn initialized_escrow_that_was_never_locked_returns_false() {
    let env = Env::default();
    let fx = EscrowFixture::new(&env).funded().build();

    assert_eq!(fx.escrow.try_is_multisig_locked(), Ok(Ok(false)));
}

#[test]
fn locked_escrow_returns_true() {
    let env = Env::default();
    env.mock_all_auths();
    let fx = EscrowFixture::new(&env).funded().build();

    fx.escrow.multisig_lock(&fx.admin);
    assert_eq!(fx.escrow.try_is_multisig_locked(), Ok(Ok(true)));
}

#[test]
fn read_mutates_no_ledger_entry_and_publishes_no_event() {
    let env = Env::default();
    env.mock_all_auths();
    let fx = EscrowFixture::new(&env).funded().build();
    fx.escrow.multisig_lock(&fx.admin);

    let before = env.to_ledger_snapshot();
    assert!(fx.escrow.is_multisig_locked());
    assert_eq!(
        crate::all_event_tuples(&env).len(),
        0,
        "a read publishes no event"
    );
    assert!(fx.escrow.is_multisig_locked());
    let after = env.to_ledger_snapshot();

    assert_eq!(before, after, "a read mutates no ledger entry");
}
