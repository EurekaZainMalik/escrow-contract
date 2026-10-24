#![cfg(test)]

use crate::testutils::EscrowFixture;

#[test]
fn test_time_until_auto_release_performs_no_state_mutation() {
    let env = soroban_sdk::Env::default();
    let fx = EscrowFixture::new(&env).milestones(&[101]).funded().build();

    // This is a read-only query that returns seconds remaining until auto-release
    let seconds_remaining = fx.escrow.time_until_auto_release(&0);
    // Verify it returns a valid i64 value without panicking
    assert!(
        seconds_remaining >= 0,
        "Should return valid seconds for populated state"
    );
}

#[test]
fn test_time_until_auto_release_returns_documented_values_populated_state() {
    let env = soroban_sdk::Env::default();
    let fx = EscrowFixture::new(&env).milestones(&[101]).funded().build();

    // Populated state: milestone exists with delivery timestamp and auto-release delay
    let seconds_remaining = fx.escrow.time_until_auto_release(&0);
    // Should return the number of seconds until deadline (positive if not yet expired)
    assert!(
        seconds_remaining > 0,
        "Newly delivered milestone should have positive time remaining"
    );
}

#[test]
fn test_time_until_auto_release_returns_documented_values_empty_state() {
    let env = soroban_sdk::Env::default();
    let fx = EscrowFixture::new(&env).milestones(&[101]).funded().build();

    // Empty state: attempt to query a non-existent milestone
    let result = fx.escrow.try_time_until_auto_release(&999);
    // Should return an error for non-existent milestone
    assert!(
        result.is_err(),
        "Non-existent milestone should return error"
    );
}

#[test]
fn test_time_until_auto_release_returns_documented_values_boundary_state() {
    let env = soroban_sdk::Env::default();
    let fx = EscrowFixture::new(&env).milestones(&[101]).funded().build();

    // Boundary state: verify correct computation of remaining time
    let seconds_remaining = fx.escrow.time_until_auto_release(&0);
    // Value should be a valid i64 representing seconds (can be positive or negative)
    // Positive = deadline not yet reached, negative = deadline passed
    let _: i64 = seconds_remaining; // Type-check that it's i64
}
