// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    CancelToken, DataForge, DataForgeError, Filter, Record, RecordTree, TreeBuilder,
};

use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_number_capture::CapturedNumber;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_table::CatalogTable;
use crate::contract_recovery::{ItemRecoveryEconomy, ItemRecoveryOverrides};
use crate::export_diagnostic::ExportSourceRole;
use crate::export_ledger::{ExportLedger, SourceDisposition};
use crate::record_observation::RecordObservation;
use crate::selection::KindRule;
use crate::source_projection_notes;
use crate::{CatalogError, CatalogReadFailure};

/// Selects the Records that hold the item recovery rules.
const RECOVERY: KindRule = KindRule {
    prefixes: &["libs/foundry/records/entitlementpolicies/"],
    type_name: "ItemRecoveryConfigurationParams",
};

#[path = "recovery_economy.rs"]
mod economy;
#[path = "recovery_notification_read.rs"]
mod notification_read;
#[path = "recovery_overrides.rs"]
mod overrides;
#[path = "recovery_policy_contract.rs"]
mod policy_contract;
#[path = "recovery_policy_read.rs"]
mod policy_read;
#[path = "recovery_projection.rs"]
mod projection;

pub(crate) use policy_contract::ItemRecoveryPolicy;

/// The item recovery rules of a DataForge and the Records that could not be read.
pub(crate) struct Recovery<'f> {
    pub(crate) ledger: ExportLedger,
    pub(crate) economy: CatalogTable<ItemRecoveryEconomy<'f>>,
    pub(crate) overrides: CatalogTable<ItemRecoveryOverrides>,
    pub(crate) policy: CatalogTable<ItemRecoveryPolicy<'f>>,
    pub(crate) numbers: Vec<CapturedNumber>,
    /// The selected Records that could not be read; they are left out.
    pub(crate) failures: Vec<CatalogReadFailure<'f>>,
}

/// Reads the item recovery rules of `forge` from the `ItemRecoveryConfigurationParams` Records
/// that `filter` matches.
pub(crate) fn read_recovery<'f>(
    forge: &'f DataForge<'f>,
    projection: &CatalogProjection<'_, 'f>,
    filter: Option<&Filter>,
    cancel: Option<&CancelToken>,
) -> Result<Recovery<'f>, CatalogError> {
    let input = projection.original();
    input.check_forge(forge)?;
    let mut trees = TreeBuilder::new();
    let mut best: Option<(Record<'f>, RecordTree<'f>)> = None;
    let mut failures = Vec::new();
    let mut ledger = ExportLedger::default();
    for record in forge.records() {
        if cancel.is_some_and(CancelToken::is_cancelled) {
            return Err(CatalogError::Cancelled);
        }
        if !selected(&record, input, filter, &mut ledger, &mut failures)? {
            continue;
        }
        let admission = RecordObservation::capture(input, &record)?;
        match trees.build(&record) {
            Ok(tree) => {
                ledger.merge(crate::export_field_walk::inspect_observed(
                    Some(input),
                    &tree,
                    &admission,
                    None,
                    ExportSourceRole::Dependency,
                    cancel,
                )?);
                ledger.observe(admission.source(
                    None,
                    ExportSourceRole::Dependency,
                    SourceDisposition::SkippedAfterRead,
                )?);
                if best
                    .as_ref()
                    .is_none_or(|(_, have)| tree.record_path() < have.record_path())
                {
                    best = Some((record, tree));
                }
            }
            Err(source) => {
                ledger.observe(admission.source(
                    None,
                    ExportSourceRole::Dependency,
                    SourceDisposition::ReadFailed,
                )?);
                failures.push(CatalogReadFailure::of(&record, &source));
            }
        }
    }
    finish(forge, projection, best, ledger, failures, cancel)
}

fn finish<'f>(
    forge: &DataForge<'_>,
    projection: &CatalogProjection<'_, 'f>,
    best: Option<(Record<'f>, RecordTree<'f>)>,
    mut ledger: ExportLedger,
    failures: Vec<CatalogReadFailure<'f>>,
    cancel: Option<&CancelToken>,
) -> Result<Recovery<'f>, CatalogError> {
    let Some((record, tree)) = best else {
        return Ok(Recovery {
            economy: CatalogTable::new(forge, &[RECOVERY.type_name], Vec::new()),
            overrides: CatalogTable::new(forge, &[RECOVERY.type_name], Vec::new()),
            policy: CatalogTable::new(forge, &[RECOVERY.type_name], Vec::new()),
            numbers: Vec::new(),
            ledger,
            failures,
        });
    };
    let admission = RecordObservation::capture(projection.original(), &record)?;
    let selected = projection::read(&tree, projection, &admission, cancel)?;
    source_projection_notes::record_dependency(&admission, &tree, selected.notes, &mut ledger)?;
    for source in &mut ledger.sources {
        if source.record_index == record.index() {
            source.disposition = SourceDisposition::Projected;
        }
    }
    Ok(Recovery {
        economy: CatalogTable::new(forge, &[RECOVERY.type_name], vec![selected.economy]),
        overrides: CatalogTable::new(forge, &[RECOVERY.type_name], vec![selected.overrides]),
        policy: CatalogTable::new(forge, &[RECOVERY.type_name], vec![selected.policy]),
        numbers: selected.numbers,
        ledger,
        failures,
    })
}

fn selected<'f>(
    record: &Record<'f>,
    input: &OriginalCatalogInput<'_, '_>,
    filter: Option<&Filter>,
    ledger: &mut ExportLedger,
    failures: &mut Vec<CatalogReadFailure<'f>>,
) -> Result<bool, CatalogError> {
    if record.record_path().is_err() {
        return Ok(false);
    }
    match is_selected(record, filter) {
        Ok(selected) => Ok(selected),
        Err(error) => {
            let admission = RecordObservation::capture(input, record)?;
            ledger.observe(admission.source(
                None,
                ExportSourceRole::SelectionCandidate,
                SourceDisposition::ReadFailed,
            )?);
            failures.push(CatalogReadFailure::of(record, &error));
            Ok(false)
        }
    }
}

fn is_selected(record: &Record<'_>, filter: Option<&Filter>) -> Result<bool, DataForgeError> {
    let path = record.record_path()?;
    Ok(RECOVERY.matches_path(path)
        && filter.is_none_or(|filter| filter.matches(path))
        && record.type_name()? == RECOVERY.type_name)
}
