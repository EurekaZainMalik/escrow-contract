#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Ledger, vec};

/// After the temporary `TimeExt` entry is evicted, the persistent copy must
/// still be present so that `time_until_auto_release` and `claim_auto_release`
/// use the full extended deadline.
#[test]
fn time_ext_survives_temporary_eviction() {
    let env = Env::default();
    env.mock_all_auths();
    let amounts = vec![&env, 5_000_i128];
    let (client_addr, freelancer_addr, _, _, _, contract_id, escrow) =
        setup_funded_escrow(&env, amounts);

    escrow.mark_delivered(&freelancer_addr, &0);

    let extension: u32 = 300;
    escrow.extend_milestone_deadline(&client_addr, &0, &extension);

    // Simulate TTL eviction of the temporary `TimeExt` entry.
    env.as_contract(&contract_id, || {
        env.storage().temporary().remove(&DataKey::TimeExt(0));
    });

    // The persistent copy must keep the extension alive.
    let remaining = escrow.time_until_auto_release(&0);
    let job = escrow.get_job();
    let milestone = job.milestones.get(0).unwrap();
    let expected_deadline = milestone.delivered_at + job.auto_release_seconds + extension as u64;
    let expected_remaining = i64::try_from(expected_deadline).unwrap()
        - i64::try_from(env.ledger().timestamp()).unwrap();
    assert_eq!(remaining, expected_remaining);

    // Advance to just before the extended deadline — auto-release must fail.
    env.ledger()
        .with_mut(|l| l.timestamp = expected_deadline - 1);
    assert_eq!(
        escrow.try_claim_auto_release(&freelancer_addr, &0),
        Err(Ok(Error::DeadlineNotPassed))
    );

    // At exactly the extended deadline the claim must succeed.
    env.ledger().with_mut(|l| l.timestamp = expected_deadline);
    escrow.claim_auto_release(&freelancer_addr, &0);
}
