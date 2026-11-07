// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use winreg::enums;

use crate::installer::binding::Binding;
use crate::installer::claim::Claim;
use crate::installer::registry_adapter;
use crate::installer::registry_store::{RawValue, RegistrySnapshot};
use crate::installer::state::{Phase, Snapshot};

use super::FixtureImage;

/// Phase independently decoded from the retained WAL.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub enum CorePhase {
    /// Before PATH application.
    Prepared,
    /// PATH and ownership are applied.
    Applied,
    /// The operation committed.
    Committed,
    /// The operation was compensated.
    RolledBack,
}

/// How the durable owner obtained its PATH entry.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub enum OwnerClaim {
    /// An exact pre-existing entry was borrowed.
    Borrowed,
    /// The helper retains an insertion proof.
    Inserted,
}

/// MSI identities and claim decoded from the WAL rather than command arguments.
#[derive(Debug)]
#[non_exhaustive]
pub struct FixtureOwner {
    /// `ProductCode` decoded from durable state.
    pub product: u128,
    /// Actual `PackageCode` decoded from durable state.
    pub package: u128,
    /// Ownership proof class.
    pub claim: OwnerClaim,
}

/// A derived view of owner/journal metadata, independent from the raw PATH bytes.
#[derive(Debug)]
#[non_exhaustive]
pub enum CoreInspection {
    /// No WAL, sequence or binding exists.
    Empty,
    /// The existing helper rejects the envelope or WAL.
    Rejected {
        /// Typed helper failure rendered only as local diagnostic text.
        reason: String,
    },
    /// The existing helper decoded and validated its complete metadata.
    Valid {
        /// Durable sequence, represented losslessly by the JSON consumer.
        sequence: u64,
        /// Durable journal phase when one exists.
        phase: Option<CorePhase>,
        /// Complete raw claim remains in the retained WAL bytes.
        owner: Option<FixtureOwner>,
        /// Active reserved authority decoded from the WAL.
        reserved_binding: Option<String>,
        /// Cleanup-only fence decoded from the WAL.
        cancelled_binding: Option<String>,
        /// Journal caller binding with its proven predecessor sequence.
        journal_binding: Option<String>,
    },
}

impl FixtureImage {
    /// Validates owner/journal metadata with the same decoder used by the real helper.
    #[must_use]
    pub fn inspect_core(&self) -> CoreInspection {
        let [_, state, sequence, binding] = self.values();
        if state.is_some_and(|value| value.kind != enums::REG_BINARY) {
            return CoreInspection::Rejected {
                reason: "StateV1 is not REG_BINARY".into(),
            };
        }
        let raw = |value: Option<&super::FixtureValue>| {
            value.map(|value| RawValue {
                kind: value.kind.clone(),
                bytes: value.bytes.clone(),
            })
        };
        let image = RegistrySnapshot {
            path: None,
            state: state.map(|value| value.bytes.clone()),
            sequence: raw(sequence),
            binding: raw(binding),
        };
        if let Err(fault) = super::super::validate(&image) {
            return CoreInspection::Rejected {
                reason: fault.to_string(),
            };
        }
        let snapshot = match registry_adapter::stored(&image) {
            Ok(snapshot) => snapshot,
            Err(fault) => {
                return CoreInspection::Rejected {
                    reason: fault.to_string(),
                };
            }
        };
        if image.state.is_none() {
            return CoreInspection::Empty;
        }
        decoded(snapshot)
    }
}

fn decoded(snapshot: Snapshot) -> CoreInspection {
    let phase = snapshot.journal.as_ref().map(|record| match record.phase {
        Phase::Prepared => CorePhase::Prepared,
        Phase::Applied => CorePhase::Applied,
        Phase::Committed => CorePhase::Committed,
        Phase::RolledBack => CorePhase::RolledBack,
    });
    let owner = snapshot.owner.map(|owner| FixtureOwner {
        product: owner.product,
        package: owner.package,
        claim: match owner.claim {
            Claim::Borrowed => OwnerClaim::Borrowed,
            Claim::Inserted(_) => OwnerClaim::Inserted,
        },
    });
    let journal_binding = snapshot.journal.as_ref().and_then(|record| {
        record
            .id
            .checked_sub(1)
            .map(|sequence| Binding::for_caller(sequence, &record.caller).encode())
    });
    CoreInspection::Valid {
        sequence: snapshot.sequence,
        phase,
        owner,
        reserved_binding: snapshot
            .reservation
            .map(|reservation| reservation.binding().encode()),
        cancelled_binding: snapshot.cancelled_binding.map(Binding::encode),
        journal_binding,
    }
}
