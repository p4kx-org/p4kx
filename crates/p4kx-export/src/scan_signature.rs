// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    CancelToken, Record, RecordTree, SourceUse, SourceUseRole, SourceUses, SourceValue,
    SourcedValue, TreeBuilder, Value,
};
use rayon::prelude::*;

use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_read::original::{OriginalFlowFields, OriginalValuePart};
use crate::crafting_warning::CraftingWarning;
use crate::export_ledger::ExportLedger;
use crate::record_observation::RecordObservation;
use crate::resource_float::ResourceFloat32;
use crate::{CatalogError, CatalogReadFailure, ExportAccountingError, Kind};

pub(crate) const RADAR_GLOBAL_PARAMS: &str = "RadarSystemGlobalParams";
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
    /// A malformed declaration prevents proving which slot is the scan signature.
    Unreadable,
}

impl SlotProblem {
    /// Returns the warning for the problem.
    pub(crate) const fn warning(self) -> CraftingWarning {
        CraftingWarning::UnknownScanSignatureSlot(match self {
            Self::NoRecord => "the Archive has no RadarSystemGlobalParams Record",
            Self::NoEntry => "no signature type of the radar parameters is the scan signature",
            Self::Unreadable => "a signature declaration cannot be read",
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
#[derive(Debug, Clone)]
pub(crate) struct SignatureSlot {
    layout: Option<(usize, usize)>,
    selector: Option<SourceUse>,
}

type CatalogSlotRead<'f> = (
    SignatureSlot,
    Vec<CatalogReadFailure<'f>>,
    Option<SlotProblem>,
    ExportLedger,
);

impl SignatureSlot {
    /// Returns the slot of an Archive that names none.
    pub(crate) const fn unknown() -> Self {
        Self {
            layout: None,
            selector: None,
        }
    }

    /// Returns the slot `index` of `types` signature types.
    #[cfg(test)]
    pub(crate) const fn at(index: usize, types: usize) -> Self {
        Self {
            layout: Some((index, types)),
            selector: None,
        }
    }

    /// Reads scan-signature dependencies and retains their canonical field inspections.
    #[cfg(test)]
    pub(crate) fn read_catalog<'f>(
        records: &[Record<'f>],
        cancel: Option<&CancelToken>,
    ) -> Result<CatalogSlotRead<'f>, CatalogError> {
        Self::read_observed(records, None, cancel)
    }

    pub(crate) fn read_captured<'f>(
        records: &[Record<'f>],
        input: &OriginalCatalogInput<'_, '_>,
        cancel: Option<&CancelToken>,
    ) -> Result<CatalogSlotRead<'f>, CatalogError> {
        Self::read_observed(records, Some(input), cancel)
    }

    fn read_observed<'f>(
        records: &[Record<'f>],
        input: Option<&OriginalCatalogInput<'_, '_>>,
        cancel: Option<&CancelToken>,
    ) -> Result<CatalogSlotRead<'f>, CatalogError> {
        let found = records
            .par_iter()
            .map_init(TreeBuilder::new, |trees, record| {
                if cancel.is_some_and(CancelToken::is_cancelled) {
                    return Err(CatalogError::Cancelled);
                }
                if record.type_name().ok() != Some(RADAR_GLOBAL_PARAMS) {
                    return Ok(None);
                }
                let admission = match input {
                    Some(input) => RecordObservation::capture(input, record)?,
                    None => RecordObservation::accounting(record),
                };
                let read = crate::export_dependency_read::read(
                    input,
                    trees,
                    &admission,
                    Kind::MineableEntities,
                    cancel,
                )?;
                let outcome = match read.tree {
                    Ok(tree) => Ok(Self::read_layout(input, &admission, &tree, cancel)?),
                    Err(error) => Err(CatalogReadFailure::of(record, &error)),
                };
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
        if !failures.is_empty() {
            return Ok((Self::unknown(), failures, None, ledger));
        }
        let outcome = match slots.as_slice() {
            [] => Err(SlotProblem::NoRecord),
            [only] => only.clone(),
            _ => Err(SlotProblem::Ambiguous),
        };
        match outcome {
            Ok(slot) => Ok((slot, failures, None, ledger)),
            Err(problem) => Ok((Self::unknown(), failures, Some(problem), ledger)),
        }
    }

    /// Returns the slot the `RadarSystemGlobalParams` tree `tree` names.
    fn of_tree(tree: &RecordTree<'_>) -> Result<(usize, usize), SlotProblem> {
        let system = match tree.field("signatureSystemParams") {
            Some(Value::Struct(system)) => system,
            None | Some(Value::Null) => return Err(SlotProblem::NoEntry),
            _ => return Err(SlotProblem::Unreadable),
        };
        let types = match system.field("signatureTypeParams") {
            Some(Value::Array(types)) => types,
            None | Some(Value::Null) => return Err(SlotProblem::NoEntry),
            _ => return Err(SlotProblem::Unreadable),
        };
        let mut selected = None;
        let mut unreadable = false;
        for (index, entry) in types.iter().enumerate() {
            let display_name = match entry {
                Value::Struct(entry) => entry.field("displayName"),
                Value::Null => None,
                _ => {
                    unreadable = true;
                    continue;
                }
            };
            match display_name {
                Some(Value::Locale(key)) if *key == SCAN_SIGNATURE_KEY => {
                    if selected.replace(index).is_some() {
                        return Err(SlotProblem::Ambiguous);
                    }
                }
                None | Some(Value::Null | Value::Locale(_)) => {}
                _ => unreadable = true,
            }
        }
        if unreadable {
            return Err(SlotProblem::Unreadable);
        }
        selected
            .map(|index| (index, types.len()))
            .ok_or(SlotProblem::NoEntry)
    }

    fn read_layout<'source>(
        input: Option<&OriginalCatalogInput<'_, 'source>>,
        admission: &RecordObservation<'_, 'source>,
        tree: &RecordTree<'source>,
        cancel: Option<&CancelToken>,
    ) -> Result<Result<Self, SlotProblem>, CatalogError> {
        let layout = match Self::of_tree(tree) {
            Ok(layout) => layout,
            Err(problem) => return Ok(Err(problem)),
        };
        let selector = input
            .map(|input| Self::capture_selector(input, admission, tree, layout.0, cancel))
            .transpose()?;
        Ok(Ok(Self {
            layout: Some(layout),
            selector,
        }))
    }

    fn capture_selector<'source>(
        input: &OriginalCatalogInput<'_, 'source>,
        admission: &RecordObservation<'_, 'source>,
        tree: &RecordTree<'source>,
        index: usize,
        cancel: Option<&CancelToken>,
    ) -> Result<SourceUse, CatalogError> {
        const MISSING: ExportAccountingError = ExportAccountingError::ConflictingSourceDatum;
        let scope = input.record_scope(admission.record(), tree)?;
        let fields = OriginalFlowFields::read(&scope, cancel)?;
        let (_, system) = fields
            .field(None, "signatureSystemParams")?
            .ok_or(MISSING)?;
        let (_, types) = fields
            .field(Some(system), "signatureTypeParams")?
            .ok_or(MISSING)?;
        let Value::Array(types) = types.value() else {
            return Err(MISSING.into());
        };
        let slot = fields.fact(types.get(index).ok_or(MISSING)?)?;
        let (_, key) = fields.field(Some(slot), "displayName")?.ok_or(MISSING)?;
        key.observe(OriginalValuePart::Value, SourceUseRole::Selector)
    }

    /// Projects the finite scan signature and binds the evidence for its slot selection.
    pub(crate) fn project(
        &self,
        signatures: &SourcedValue<Vec<SourcedValue<ResourceFloat32>>>,
        sources: &SourceUses,
    ) -> Result<SourcedValue<f32>, CatalogError> {
        let Some(selector) = &self.selector else {
            return Ok(sources.add_value_origins(
                sources.value(SourceValue::Absent, &[], false),
                &[signatures.origin()],
            )?);
        };
        let selector_origin = sources.origin(std::slice::from_ref(selector), true);
        let shape = [signatures.origin(), &selector_origin];
        let values = signatures.value().and_then(|values| {
            self.layout
                .filter(|(_, count)| *count == values.len())
                .map(|(index, _)| (values, index))
        });
        let Some((values, index)) = values else {
            return Ok(sources.value_with_origins(SourceValue::Absent, &shape)?);
        };
        let selected = values
            .get(index)
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        let nonzero = values.iter().find(|value| {
            value
                .value()
                .and_then(ResourceFloat32::finite)
                .is_some_and(|value| value != 0.0)
        });
        if let (Some(value), Some(nonzero)) =
            (selected.value().and_then(ResourceFloat32::finite), nonzero)
        {
            return Ok(sources.value_with_origins(
                SourceValue::Present(value),
                &[shape[0], shape[1], selected.origin(), nonzero.origin()],
            )?);
        }
        let mut evidence = Vec::from(shape);
        evidence.extend(values.iter().map(SourcedValue::origin));
        Ok(sources.value_with_origins(SourceValue::Absent, &evidence)?)
    }

    /// Returns the scan signature among `signatures`; `None` when the slot is unknown, when
    /// `signatures` has not one number per signature type, or when every signature is zero.
    #[cfg(test)]
    fn value(&self, signatures: &[f32]) -> Option<f32> {
        let (index, types) = self.layout?;
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
