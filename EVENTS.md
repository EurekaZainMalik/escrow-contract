# Event Reference

All events emitted by the `milestone-escrow` contract, keyed by their
`symbol_short!` topic. This table is kept in sync with the source by the
`event_docs_sync_tests` module — a topic cannot be added to `lib.rs` without a
row here, and a row here must correspond to a topic in the source.

| Topic      | Event struct                     | Kind              |
| ---------- | -------------------------------- | ----------------- |
| `init`     | `InitializedEvent`               | state change      |
| `fund`     | `FundedEvent`                    | state change      |
| `acceptjob`| `JobAcceptedEvent`               | state change      |
| `acceptarb`| `JobAcceptedEvent`               | state change      |
| `unaccref` | `UnacceptedWithdrawnEvent`       | state change      |
| `withdraw` | `WithdrawnEvent`                 | state change      |
| `msfund`   | `MilestoneFundedEvent`           | state change      |
| `deliver`  | `DeliveredEvent`                 | state change      |
| `msrev`    | `RevisionRequestedEvent`         | state change      |
| `approve`  | `ApprovedEvent`                  | state change      |
| `claim`    | `ClaimedEvent`                   | state change      |
| `extend`   | `DeadlineExtendedEvent`          | state change      |
| `dispute`  | `DisputeRaisedEvent`             | state change      |
| `dispescal`| `DisputeEscalatedEvent`          | state change      |
| `resolve`  | `DisputeResolvedEvent`           | state change      |
| `sweep`    | `ExcessSweptEvent`               | state change      |
| `cancel`   | `CancelEscrowInitiatedEvent`     | state change      |
| `cxlappr`  | `CancelApprovalRecordedEvent`    | state change      |
| `cancelrev`| `CancelApprovalRevokedEvent`     | state change      |
| `upgrade`  | `ContractUpgradedEvent`          | state change      |
| `wtok`     | `TokenWhitelistedEvent`          | state change      |
| `wldel`    | `TokenRemovedEvent`              | state change      |
| `pause`    | `EscrowPausedEvent`              | state change      |
| `resume`   | `EscrowResumedEvent`             | state change      |
| `empause`  | `EmergencyPausedEvent`           | state change      |
| `emunpause`| `EmergencyUnpausedEvent`         | state change      |
| `emoverrid`| `EmergencyPauseAdminOverrideEvent` | state change    |
| `epalloc`  | `EmergencyPauseAllocationEvent`  | calculation       |
| `admin`    | `TransferAdminEvent`             | state change      |
| `adminprp` | `AdminTransferProposedEvent`     | state change      |
| `admincxl` | `AdminTransferCancelledEvent`    | state change      |
| `adminexc` | `AdminTransferExecutedEvent`     | state change      |
| `adcovls`  | `AdminCancelOverrideReleaseEvent`| state change      |
| `adcovrf`  | `AdminCancelOverrideRefundEvent` | state change      |
| `admovrls` | `AdminOverrideReleaseEvent`      | state change      |
| `admovrf`  | `AdminOverrideRefundEvent`       | state change      |
| `admstrm`  | `AdminOverrideStreamingReleaseEvent` | state change  |
| `adtxrls`  | `AdminOverrideTaxReleaseEvent`   | state change      |
| `adtxrfd`  | `AdminOverrideTaxRefundEvent`    | state change      |
| `adtwsplt` | `AdminOverrideTaxSplitRefundEvent` | calculation     |
| `arbsplit` | `ArbitrationSplitAppliedEvent`   | calculation       |
| `msiginit` | `MultisigApprovalInitEvent`      | state change      |
| `msigappr` | `MultiSigApprovedEvent`          | state change      |
| `mslock`   | `MultisigLockedEvent`            | state change      |
| `msadmrel` | `MultisigAdminOverrideReleaseEvent` | state change   |
| `msadmref` | `MultisigAdminOverrideRefundEvent`  | state change   |
| `intyield` | `EscrowInterestYieldEvent`       | calculation       |
| `yldset`   | `EscrowInterestYieldSetEvent`    | state change      |
| `yldcons`  | `EscrowInterestYieldConsentSetEvent` | state change  |
| `yldrate`  | `YieldRateSetEvent`              | state change      |
| `yldacc`   | `YieldAccruedEvent`              | calculation       |
| `yldlock`  | `EscrowInterestYieldLockedEvent` | state change      |
| `yldunlock`| `EscrowInterestYieldUnlockedEvent` | state change    |
| `pf_set`   | `PlatformFeeAllocationSetEvent`  | state change      |
| `pf_lock`  | `PlatformFeeAllocationLockedEvent` | state change    |
| `pf_ovr`   | `PlatformFeeAllocationOverrideEvent` | state change  |
| `pf_split` | `PlatformFeeSplitCalculatedEvent`| calculation       |
| `p_stream` | `PaymentStreamingEvent`          | calculation       |
| `p_strcns` | `PaymentStreamingConsentEvent`   | state change      |
| `m_ext`    | `MilestoneTimeExtensionEvent`    | state change      |
| `m_extcns` | `TimeExtConsentEvent`            | state change      |
| `taxwith`  | `TaxWithholdingAppliedEvent`     | state change      |
| `taxwh`    | `TaxWithholdingDeductionsEvent`  | calculation       |
| `twspltref`| `TaxWithholdingSplitRefundEvent` | calculation       |
| `splitref` | `SplitRefundCalculatedEvent`     | calculation       |
| `iyspltref`| `SplitRefundCalculatedEvent`     | calculation       |
| `epspltref`| `SplitRefundCalculatedEvent`     | calculation       |
| `cxlspref` | `CancelSplitRefundCalculatedEvent` | calculation     |
| `sprefnet` | `SplitRefundNetDistributionEvent`| calculation       |

## Notes

- **Kind** distinguishes events that record a durable state change from those
  that report a derived calculation (fee/tax/split math). Indexers should treat
  calculation events as advisory and reconcile against persisted state.
- Several split-refund paths share the `SplitRefundCalculatedEvent` struct but
  use distinct topics (`arbsplit`, `epspltref`, `iyspltref`, `splitref`) so
  consumers can tell the originating workflow apart.
