#![cfg(test)]
//! Boundary and overflow tables for the ratio-split helpers (issue #750):
//! `split_round_nearest`, `allocate_platform_fee`,
//! `split_refund_net_distribution`, `emergency_pause_allocation` and
//! `emergency_split_exact`.
//!
//! Every case asserts either a correct split — the parts are non-negative and
//! sum to the total — or the specific typed error. None of them may panic.

use super::*;
use crate::{DataKey, Error, PlatformFeeAllocation};
use soroban_sdk::testutils::EnvTestConfig;
use soroban_sdk::{vec, Env, Vec};

const SCALE: u32 = 10_000;
const BPS_CASES: [u32; 4] = [0, 1, 9_999, 10_000];

fn test_env() -> Env {
    Env::new_with_config(EnvTestConfig {
        capture_snapshot_at_drop: false,
    })
}

fn total_cases() -> [i128; 6] {
    let edge = i128::MAX / SCALE as i128;
    [0, 1, edge - 1, edge, edge + 1, i128::MAX]
}

fn bare_contract(env: &Env) -> MilestoneEscrowClient<'_> {
    let contract_id = env.register(MilestoneEscrow, ());
    MilestoneEscrowClient::new(env, &contract_id)
}

// ── split_round_nearest ──────────────────────────────────────────────────────

#[test]
fn split_round_nearest_table() {
    for total in total_cases() {
        for bps in BPS_CASES {
            let result = MilestoneEscrow::split_round_nearest(total, bps as i128, SCALE as i128);
            let expected = total
                .checked_mul(bps as i128)
                .and_then(|scaled| scaled.checked_add(SCALE as i128 / 2))
                .map(|biased| biased / SCALE as i128);

            match expected {
                Some(first) => {
                    let split = result.unwrap();
                    assert_eq!(split.first, first, "total {total}, bps {bps}");
                    assert!(split.first >= 0 && split.second >= 0);
                    assert_eq!(split.first.checked_add(split.second), Some(total));
                }
                None => assert_eq!(
                    result,
                    Err(Error::InvalidAmount),
                    "total {total}, bps {bps}"
                ),
            }
        }
    }
}

#[test]
fn split_round_nearest_rejects_invalid_ratio_inputs() {
    for (total, numerator, denominator) in [
        (-1_i128, 1_i128, 10_000_i128),
        (1_000, -1, 10_000),
        (1_000, 1, 0),
        (1_000, 1, -1),
        (1_000, 10_001, 10_000),
    ] {
        assert_eq!(
            MilestoneEscrow::split_round_nearest(total, numerator, denominator),
            Err(Error::InvalidRatio)
        );
    }
}

// ── allocate_platform_fee ────────────────────────────────────────────────────

fn fee_allocation(
    client_bps: u32,
    freelancer_bps: u32,
    treasury_bps: u32,
) -> PlatformFeeAllocation {
    PlatformFeeAllocation {
        client_bps,
        freelancer_bps,
        treasury_bps,
        locked: false,
    }
}

#[test]
fn allocate_platform_fee_table() {
    let allocations = [
        fee_allocation(0, 10_000, 0),
        fee_allocation(1, 9_999, 0),
        fee_allocation(9_999, 0, 1),
        fee_allocation(10_000, 0, 0),
        fee_allocation(3_333, 3_333, 3_334),
    ];

    for total in total_cases() {
        for allocation in allocations.iter() {
            let result = MilestoneEscrow::allocate_platform_fee(total, allocation);
            let overflows = [
                allocation.client_bps,
                allocation.freelancer_bps,
                allocation.treasury_bps,
            ]
            .iter()
            .any(|bps| total.checked_mul(*bps as i128).is_none());

            if overflows {
                assert_eq!(result, Err(Error::ArithmeticOverflow), "total {total}");
            } else {
                let split = result.unwrap();
                assert!(
                    split.client_amount >= 0
                        && split.freelancer_amount >= 0
                        && split.treasury_amount >= 0
                );
                assert_eq!(
                    split
                        .client_amount
                        .checked_add(split.freelancer_amount)
                        .and_then(|v| v.checked_add(split.treasury_amount)),
                    Some(total),
                    "total {total}"
                );
            }
        }
    }
}

#[test]
fn allocate_platform_fee_rejects_negative_totals() {
    let allocation = fee_allocation(3_333, 3_333, 3_334);

    assert_eq!(
        MilestoneEscrow::allocate_platform_fee(i128::MIN, &allocation),
        Err(Error::ArithmeticOverflow)
    );
    assert_eq!(
        MilestoneEscrow::allocate_platform_fee(-1, &allocation),
        Err(Error::InvalidAmount)
    );
}

#[test]
fn allocate_platform_fee_hands_residue_by_largest_remainder() {
    let allocation = fee_allocation(3_333, 3_333, 3_334);

    // One residue unit: the treasury holds the largest remainder.
    assert_eq!(
        MilestoneEscrow::allocate_platform_fee(1, &allocation),
        Ok(PlatformFeeDistribution {
            client_amount: 0,
            freelancer_amount: 0,
            treasury_amount: 1,
        })
    );
    // Two residue units: the treasury first, then the client wins the tie
    // with the freelancer by field order.
    assert_eq!(
        MilestoneEscrow::allocate_platform_fee(2, &allocation),
        Ok(PlatformFeeDistribution {
            client_amount: 1,
            freelancer_amount: 0,
            treasury_amount: 1,
        })
    );
}

// ── split_refund_net_distribution ────────────────────────────────────────────

#[test]
fn split_refund_net_distribution_table() {
    let env = test_env();
    let client = bare_contract(&env);
    let fees = fee_allocation(100, 9_800, 100);
    let edge = i128::MAX / SCALE as i128;

    for total in total_cases() {
        for bps in BPS_CASES {
            let result =
                client.try_split_refund_net_distribution(&total, &bps, &(SCALE - bps), &fees);

            if total <= 0 {
                assert_eq!(result, Err(Ok(Error::InvalidAmount)));
                continue;
            }
            match result {
                Ok(Ok(split)) => {
                    assert!(
                        split.client_net_refund >= 0
                            && split.client_fee_share >= 0
                            && split.freelancer_net_payout >= 0
                            && split.treasury_fee_share >= 0
                    );
                    assert_eq!(
                        split
                            .client_net_refund
                            .checked_add(split.client_fee_share)
                            .and_then(|v| v.checked_add(split.freelancer_net_payout))
                            .and_then(|v| v.checked_add(split.treasury_fee_share)),
                        Some(total),
                        "total {total}, bps {bps}"
                    );
                }
                other => {
                    // Only totals beyond the `i128::MAX / 10_000` edge may
                    // overflow, and they must do so as a typed error.
                    assert!(total > edge, "total {total}, bps {bps}");
                    assert_eq!(other, Err(Ok(Error::InvalidAmount)));
                }
            }
        }
    }
}

#[test]
fn split_refund_net_distribution_rejects_bad_ratios() {
    let env = test_env();
    let client = bare_contract(&env);
    let fees = fee_allocation(100, 9_800, 100);

    for (client_bps, freelancer_bps) in [(u32::MAX, 1_u32), (5_000, 4_999), (5_000, 5_001)] {
        assert_eq!(
            client.try_split_refund_net_distribution(
                &1_000_i128,
                &client_bps,
                &freelancer_bps,
                &fees
            ),
            Err(Ok(Error::InvalidRatio))
        );
    }
}

#[test]
fn split_refund_net_distribution_rejects_fee_bps_above_scale() {
    let env = test_env();
    let client = bare_contract(&env);

    assert_eq!(
        client.try_split_refund_net_distribution(
            &1_000_i128,
            &5_000u32,
            &5_000u32,
            &fee_allocation(10_001, 0, 0)
        ),
        Err(Ok(Error::InvalidRatio))
    );
}

#[test]
fn split_refund_net_distribution_rejects_paused_contract() {
    let env = test_env();
    let contract_id = env.register(MilestoneEscrow, ());
    let client = MilestoneEscrowClient::new(&env, &contract_id);
    let fees = fee_allocation(100, 9_800, 100);

    env.as_contract(&contract_id, || {
        env.storage().instance().set(&DataKey::Paused, &true);
    });
    assert_eq!(
        client.try_split_refund_net_distribution(&1_000_i128, &5_000u32, &5_000u32, &fees),
        Err(Ok(Error::Paused))
    );
}

#[test]
fn split_refund_net_distribution_rejects_emergency_paused_contract() {
    let env = test_env();
    let contract_id = env.register(MilestoneEscrow, ());
    let client = MilestoneEscrowClient::new(&env, &contract_id);
    let fees = fee_allocation(100, 9_800, 100);

    env.as_contract(&contract_id, || {
        env.storage().instance().set(&DataKey::Ep, &true);
    });
    assert_eq!(
        client.try_split_refund_net_distribution(&1_000_i128, &5_000u32, &5_000u32, &fees),
        Err(Ok(Error::Paused))
    );
}

// ── emergency_pause_allocation ───────────────────────────────────────────────

#[test]
fn emergency_pause_allocation_table() {
    let env = test_env();
    env.mock_all_auths();
    let (_, _, _, admin_addr, _, _, client) = setup_funded_escrow(&env, vec![&env, 1_000_i128]);
    client.emergency_pause_admin_override(&admin_addr, &true);

    for total in total_cases() {
        for bps in BPS_CASES {
            let weights: Vec<i128> = vec![&env, bps as i128, (SCALE - bps) as i128];
            let result = client.try_emergency_pause_allocation(&admin_addr, &total, &weights);

            if total <= 0 {
                assert_eq!(result, Err(Ok(Error::InvalidAmount)));
                continue;
            }
            let overflows = weights.iter().any(|w| total.checked_mul(w).is_none());
            if overflows {
                assert_eq!(
                    result,
                    Err(Ok(Error::InvalidAmount)),
                    "total {total}, bps {bps}"
                );
            } else {
                let allocations = result.unwrap().unwrap();
                assert_eq!(allocations.len(), 2);
                let mut sum = 0_i128;
                for amount in allocations.iter() {
                    assert!(amount >= 0);
                    sum = sum.checked_add(amount).unwrap();
                }
                assert_eq!(sum, total, "total {total}, bps {bps}");
            }
        }
    }
}

// ── emergency_split_exact ────────────────────────────────────────────────────

#[test]
fn emergency_split_exact_table() {
    for total in total_cases() {
        for bps in BPS_CASES {
            let result = MilestoneEscrow::emergency_split_exact(total, bps, SCALE - bps);

            if total <= 0 {
                assert_eq!(result, Err(Error::InvalidAmount));
                continue;
            }
            // Never forms `total × bps`, so every positive total splits.
            let split = result.unwrap();
            assert!(split.client_refund >= 0 && split.freelancer_payout >= 0);
            assert_eq!(
                split.client_refund.checked_add(split.freelancer_payout),
                Some(total),
                "total {total}, bps {bps}"
            );
            assert_eq!(split.client_refund_bps, bps);
            assert_eq!(split.freelancer_payout_bps, SCALE - bps);

            // Wherever the direct formula fits, both must agree.
            if let Ok(direct) =
                MilestoneEscrow::split_round_nearest(total, bps as i128, SCALE as i128)
            {
                assert_eq!(
                    split.client_refund, direct.first,
                    "total {total}, bps {bps}"
                );
            }
        }
    }
}

#[test]
fn emergency_split_exact_rejects_bad_ratios() {
    for (client_bps, freelancer_bps) in [(u32::MAX, 1_u32), (5_000, 4_999), (5_000, 5_001)] {
        assert_eq!(
            MilestoneEscrow::emergency_split_exact(1_000, client_bps, freelancer_bps),
            Err(Error::InvalidRatio)
        );
    }
}

#[test]
fn emergency_split_exact_rejects_negative_total() {
    assert_eq!(
        MilestoneEscrow::emergency_split_exact(-1, 5_000, 5_000),
        Err(Error::InvalidAmount)
    );
    assert_eq!(
        MilestoneEscrow::emergency_split_exact(i128::MIN, 5_000, 5_000),
        Err(Error::InvalidAmount)
    );
}
