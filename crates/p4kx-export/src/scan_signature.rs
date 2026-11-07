// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, Record, RecordTree, TreeBuilder, Value};
use rayon::prelude::*;

use crate::crafting_warning::CraftingWarning;
use crate::export_ledger::ExportLedger;
use crate::record_fields::{elements, locale_key, structure};
use crate::{CatalogError, CatalogFailure, Kind};

const RADAR_GLOBAL_PARAMS: &str = "RadarSystemGlobalParams";
const SCAN_SIGNATURE_KEY: &str = "@hud_scanning_info_rs_signature";

/// Why the slot of the scan signature cannot be determined.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SlotProblem {
    /// The Archive has no `RadarSystemGlobalParams` Record.
    NoRecord,
    /// No `signatureTypeParams` entry has the scan signature key.
    NoEntry,
    /// Several `signatureTypeParams` entries or Records have the scan signature key.
    Ambiguous,
}

impl SlotProblem {
    /// Returns the warning for the problem.
    pub(crate) const fn warning(self) -> CraftingWarning {
        CraftingWarning::UnknownScanSignatureSlot(match self {
            Self::NoRecord => "the Archive has no RadarSystemGlobalParams Record",
            Self::NoEntry => "no signature type of the radar parameters is the scan signature",
            Self::Ambiguous => {
                "several signature types of the radar parameters are the scan signature"
            }
        })
    }
}

/// The slot of an entity's `signatures` that holds the scan signature.
///
/// The `signatureTypeParams` array of the `RadarSystemGlobalParams` Record has one entry per slot
/// of `signatures`; the slot whose `displayName` is `@hud_scanning_info_rs_signature` is the scan
/// signature. The index only applies to an entity whose `signatures` has as many slots as the
/// array has entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SignatureSlot(Option<(usize, usize)>);

type CatalogSlotRead = (
    SignatureSlot,
    Vec<CatalogFailure>,
    Option<SlotProblem>,
    ExportLedger,
);

impl SignatureSlot {
    /// Returns the slot of an Archive that names none.
    pub(crate) const fn unknown() -> Self {
        Self(None)
    }

    /// Returns the slot `index` of `types` signature types.
    #[cfg(test)]
    pub(crate) const fn at(index: usize, types: usize) -> Self {
        Self(Some((index, types)))
    }

    /// Reads scan-signature dependencies and retains their canonical field inspections.
    pub(crate) fn read_catalog(
        records: &[Record<'_>],
        cancel: Option<&CancelToken>,
    ) -> Result<CatalogSlotRead, CatalogError> {
        let found = records
            .par_iter()
            .map_init(TreeBuilder::new, |trees, record| {
                if cancel.is_some_and(CancelToken::is_cancelled) {
                    return Err(CatalogError::Cancelled);
                }
                if record.type_name().ok() != Some(RADAR_GLOBAL_PARAMS) {
                    return Ok(None);
                }
                let read = crate::export_dependency_read::read(
                    trees,
                    record,
                    Kind::MineableEntities,
                    cancel,
                )?;
                let outcome = read
                    .tree
                    .map(|tree| Self::of_tree(&tree))
                    .map_err(|error| CatalogFailure::of(record, &error));
                Ok(Some((outcome, read.ledger)))
            })
            .filter_map(Result::transpose)
            .collect::<Result<Vec<_>, CatalogError>>()?;
        let mut failures = Vec::new();
        let mut slots = Vec::new();
        let mut ledger = ExportLedger::default();
        for (result, observed) in found {
            ledger.merge(observed);
            match result {
                Ok(slot) => slots.push(slot),
                Err(failure) => failures.push(failure),
            }
        }
        let outcome = match slots.as_slice() {
            [] if failures.is_empty() => Err(SlotProblem::NoRecord),
            [] => return Ok((Self::unknown(), failures, None, ledger)),
            [only] => *only,
            _ => Err(SlotProblem::Ambiguous),
        };
        match outcome {
            Ok(slot) => Ok((slot, failures, None, ledger)),
            Err(problem) => Ok((Self::unknown(), failures, Some(problem), ledger)),
        }
    }

    /// Returns the slot the `RadarSystemGlobalParams` tree `tree` names.
    pub(crate) fn of_tree(tree: &RecordTree<'_>) -> Result<Self, SlotProblem> {
        let system = structure(Some(tree.root()), "signatureSystemParams");
        let types = elements(system, "signatureTypeParams");
        let mut matching = types.iter().enumerate().filter(|(_, entry)| {
            matches!(
                entry,
                Value::Struct(entry)
                    if locale_key(Some(entry), "displayName") == Some(SCAN_SIGNATURE_KEY)
            )
        });
        match (matching.next(), matching.next()) {
            (Some((index, _)), None) => Ok(Self(Some((index, types.len())))),
            (Some(_), Some(_)) => Err(SlotProblem::Ambiguous),
            (None, _) => Err(SlotProblem::NoEntry),
        }
    }

    /// Returns the scan signature among `signatures`; `None` when the slot is unknown, when
    /// `signatures` has not one number per signature type, or when every signature is zero.
    pub(crate) fn value(self, signatures: &[f32]) -> Option<f32> {
        let (index, types) = self.0?;
        if signatures.len() != types || signatures.iter().all(|signature| *signature == 0.0) {
            return None;
        }
        signatures.get(index).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_slot_picks_its_signature() {
        let slot = SignatureSlot::at(2, 4);
        assert_eq!(slot.value(&[0.0, 0.0, 3170.0, 0.0]), Some(3170.0));
    }

    #[test]
    fn an_unknown_slot_picks_nothing() {
        assert_eq!(SignatureSlot::unknown().value(&[0.0, 5.0]), None);
    }

    #[test]
    fn signatures_of_another_length_than_the_types_pick_nothing() {
        let slot = SignatureSlot::at(1, 3);
        assert_eq!(slot.value(&[0.0, 5.0]), None);
        assert_eq!(slot.value(&[0.0, 5.0, 0.0, 1.0]), None);
    }

    #[test]
    fn all_zero_signatures_pick_nothing() {
        assert_eq!(SignatureSlot::at(1, 3).value(&[0.0, 0.0, 0.0]), None);
        assert_eq!(SignatureSlot::at(0, 0).value(&[]), None);
    }

    #[test]
    fn a_zero_slot_among_other_signatures_is_zero() {
        assert_eq!(SignatureSlot::at(1, 2).value(&[7.0, 0.0]), Some(0.0));
    }

    #[test]
    fn each_problem_has_its_own_warning() {
        let warnings = [
            SlotProblem::NoRecord.warning(),
            SlotProblem::NoEntry.warning(),
            SlotProblem::Ambiguous.warning(),
        ];
        assert!(
            warnings[0] != warnings[1] && warnings[1] != warnings[2] && warnings[0] != warnings[2]
        );
    }
}

#[cfg(test)]
#[path = "scan_signature_health_tests.rs"]
mod health_tests;
