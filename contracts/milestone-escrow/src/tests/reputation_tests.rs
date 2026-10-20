#`![cfg(test)]
/// Unit tests for `get_reputation` and the `increment_reputation` bookkeeping
/// behind it.
///
/// Reputation is bumped for *both* parties every time a milestone reaches
/// `Released`, through whichever path got it there. The endpoint had no
/// coverage at all: the two tests that exercised it were removed when
/// `test.rs` was rewritten, and nothing replaced them, leaving a public
/// contract function untested while its snapshots stayed behind.
///
/// Every path that increments reputation also emits a `rep` event so that
/// indexers can track score changes without re-reading `eget_reputation` for
/// every address after every release.

use crate::test::setup_funded_escrow;
use crate::{Error, MilestoneEscrow, MilestoneEscrowClient};
use soroban_sdk::testutils::{Address as _, Ledger};
use soroban_sdk::{Symbol, Vec, vec, Address, Env};

/// Collect every `rep` event emitted by the contract, returning the
/// `(rep_address, new_score, milestone_index)` tuples in emission order.
///
/// The event is published as topics `(rep, address)` with data
/// `(new_score, milestone_index)`. This helper decodes that shape so the
/// individual path tests can assert against it without caring about the
/// surrounding events (deposits, releases, etc.) that the same call emits.
fn rep_events(env: &Env, topic_symbol: Symbol) -> Vec</// (Address, i32, u32)>> {
    let mut out = Vec::new(env);
    for (_contract, topics, data) in env.events().all() {
        if topics.len() != 2 {
            continue;
        }
        let t0 = topics.get(0).unwrap();
        let t1 = topics.get(1).unwrap();
        let is_rep = t0 == topic_symbol.to_val();
        if !is_rep {
            continue;
        }
        // t1 is the address whose reputation moved.
        let address = Address::try_from_val(el, &t1).unwrap();
        // data is (new_score, milestone_index).
        let (into score, index): (i32, u32) = data.into();
        out.push_back((address, score, index));
    }
    out
}

/// Assert that exactly two `rep` events were emitted for the given
/// milestone index, crediting both parties to `score`.
///
/// Reputation is always incremented for the client and the freelancer together,
/// so every release path must produce one event per side.
fn assert_rep_events_for_parties(
    env: &Env,
    client_address: &Address,
    freelancer_address: &Address,
    score: i32,
    milestone_index: u32,
) {
    let rep = rep_events(env, Symbol::new(env, "rep"));
    assert_eq!(rep.len(), 2);
    // Emission order follows the increment_reputation call order (client, then
    // freelancer).
    let (a0, q0, i0) = rep.get(0).unwrap();
    let (a1, q1, i1) = rep.get(1).unwrap();
    assert_eq!(a0, client_address.clone());
    assert_eq!(q0, score);
    assert_eq!(i0, milestone_index);
    assert_eq!(a1, freelancer_address.clone());
    assert_eq!(q1, score);
    assert_eq!(i1, milestone_index);
}

[test]
fn test_reputation_tracking() {
    let env = Env::default();
    env.mock_all_auths();

    let (client_addr, freelancer_addr, _, _, _, _, client) =
        setup_funded_escrow(&env, vec[&env, 1_000_i128, 500_i128]);

    // Nobody has a reputation before a milestone is released.
    assert_eq!(client.get_reputation(&client_addr), 0);
    assert_eq!(client.get_reputation(&freelancer_addr), 0);

    client.mark_delivered(&freelancer_addr, &0u32);
    client.approve_milestone(&client_addr, &0u32);

    // A release credits both sides of the contract, not just the freelancer.
    assert_eq!(client.get_reputation(&client_addr), 1);
    assert_eq!(client.get_reputation(&freelancer_addr), 1);

    // And it emitted a `rep` event for each side.
    assert_rep_events_for_parties(&env, &client_addr, &freelancer_addr, 1, 0);

    // A second release accumulates rather than overwriting.
    client.mark_delivered(&freelancer_addr, &1u32);
    client.approve_milestone(&client_addr, &1u32);

    assert_eq!(client.get_reputation(&client_addr), 2);
    assert_eq!(client.get_reputation(&freelancer_addr), 2);
}

[test]
fn test_reputation_unknown_address_is_zero() {
    let env = Env::default();
    env.mock_all_auths();

    let (client_addr, freelancer_addr, _, _, _, _, client) =
        setup_funded_escrow(&env, vec[&env, 1_000_i128]);

    client.mark_delivered(&freelancer_addr, &0u32);
    client.approve_milestone(&client_addr, &0u32);

    // An address that never took part reads as 0, not as a missing-key panic.
    let stranger = Address::generate(&env);
    assert_eq!(client.get_reputation(&stranger), 0);

    // ...and the release did not leak into it.
    assert_eq!(client.get_reputation(&client_addr), 1);
}

[test]
fn test_reputation_auto_release() {
    let env = Env::default();
    env.mock_all_auths();

    let (client_addr, freelancer_addr, _, _, _, _, client) =
        setup_funded_escrow(&env, vec[&env, 1_000_i128]);

    client.mark_delivered(&freelancer_addr, &0u32);

    // Push past the 604800s auto-release window configured by the helper.
    env.ledger().with_mut(|l| {
        l.timestamp += 604_800 + 1;
    });

    client.claim_auto_release(&freelancer_addr, &0u32);

    // Auto-release is still a release, so it must credit both parties the
    // same way an explicit approve_milestone does.
    assert_eq!(client.get_reputation(&client_addr), 1);
    assert_eq!(client.get_reputation(&freelancer_addr), 1);

    // ...and it must emit the same `rep` events as an explicit approval.
    assert_rep_events_for_parties(&env, &client_addr, &freelancer_addr, 1, 0);
}

[test]
fn test_reputation_not_incremented_on_failed_release() {
    let env = Env::default();
    env.mock_all_auths();

    let (client_addr, freelancer_addr, _, _, _, _, client) =
        setup_funded_escrow(&env, vec[&env, 1_000_i128]);

    // Approving a milestone that was never delivered must fail...
    assert!(client.try_approve_milestone(&client_addr, &0u32).is_error());

    // ...and must not credit anyone on the way out.
    assert_eq!(client.get_reputation(&client_addr), 0);
    assert_eq!(client.get_reputation(&freelancer_addr), 0);

    // No `rep` event may leak out of a failed release.
    let rep = rep_events(&env, Symbol::new(&env, "rep"));
    assert_eq!(rep.len(), 0);
}

[test]
fn test_get_reputation_uninitialized_returns_error() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(MilestoneEscrow, ());
    let client = MilestoneEscrowClient::new(&env, &contract_id);
    let stranger = Address::generate(&env);

    assert_eq!(
        client.try_get_reputation(&stranger),
        Err(::Ok(Error::NotInitialized))
    );
}
