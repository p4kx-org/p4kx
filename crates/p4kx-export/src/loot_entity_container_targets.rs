// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::ChildContainerPath;
use p4kx_extraction::{Archive, SourceUse, SourceValue, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use crate::catalog_read::original::{AuthoredEntryPath, EntryCapture, PhysicalEntryRef};
use crate::entry_source::EntrySource;
use crate::schema_fields::every_field_required;
use crate::starmap_index::Index;
use crate::{CatalogError, ExportAccountingError};

#[path = "loot_authoring_path.rs"]
mod authoring_path;

use authoring_path::AuthoringPath;
pub use authoring_path::RepairKind as LootContainerLookupRepairKind;

#[derive(Debug, Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(description = "", extend("x-p4kx-vocabularyClass" = "control"))]
#[expect(
    missing_docs,
    reason = "variant docs would change the shared schema representation"
)]
#[non_exhaustive]
/// The observed outcome of one authored container selector.
pub enum LootContainerTargetStatus {
    Resolved,
    NotFound,
    Ambiguous,
    UnsupportedSelector,
    IncompleteDirectory,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct LootContainerTargets {
    pub(super) target_status: SourcedValue<LootContainerTargetStatus>,
    pub(super) target_candidates: SourcedValue<Vec<SourcedValue<PhysicalEntryRef>>>,
    #[serde(rename = "targetLookupRepair")]
    pub(super) lookup_repair: Option<Box<LootContainerLookupRepair>>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(description = "", transform = every_field_required)]
/// One proven conversion from an authored path to an exact physical Archive candidate.
pub struct LootContainerLookupRepair {
    /// The admitted authoring path form.
    kind: SourcedValue<LootContainerLookupRepairKind>,
    /// The complete normalized virtual Archive lookup key.
    normalized_path: SourcedValue<String>,
}

impl LootContainerLookupRepair {
    /// Borrows the admitted authoring path form.
    #[must_use]
    pub const fn kind(&self) -> &SourcedValue<LootContainerLookupRepairKind> {
        &self.kind
    }

    /// Borrows the exact normalized virtual Archive lookup key.
    #[must_use]
    pub const fn normalized_path(&self) -> &SourcedValue<String> {
        &self.normalized_path
    }
}

#[derive(Clone, Copy)]
pub(crate) struct SelectedTarget<'index> {
    pub(crate) candidates: &'index [usize],
    pub(crate) absence_proved: bool,
}

pub(super) struct ResolvedTargets<'index> {
    pub(super) targets: LootContainerTargets,
    candidates: &'index [usize],
}

impl<'index> ResolvedTargets<'index> {
    pub(super) fn selected_target(&self, selector: &[SourceUse]) -> Option<SelectedTarget<'index>> {
        let origin = self.targets.target_status.origin();
        if !origin.source_set_complete()
            || !self
                .targets
                .target_candidates
                .origin()
                .source_set_complete()
            || selector.is_empty()
            || !selector
                .iter()
                .all(|used| origin.source_uses_refs().contains(used.reference()))
        {
            return None;
        }
        Some(SelectedTarget {
            candidates: self.candidates,
            absence_proved: self.candidates.is_empty()
                && matches!(
                    self.targets.target_status.value(),
                    Some(LootContainerTargetStatus::NotFound)
                ),
        })
    }
}

pub(crate) fn literal_candidates<'index>(
    index: &'index Index<'_>,
    selector: &str,
) -> Option<&'index [usize]> {
    let candidates = index.candidates(selector);
    (!candidates.is_empty() || ChildContainerPath::new(selector).is_some()).then_some(candidates)
}

fn authoring_candidates<'index>(
    index: &'index Index<'_>,
    selector: &str,
) -> Option<(AuthoringPath, &'index [usize])> {
    let alias = AuthoringPath::parse(selector)?;
    let entries = index.exact_normalized_candidates(&alias.normalized);
    (!entries.is_empty()).then_some((alias, entries))
}

pub(super) fn resolve<'index>(
    archive: &Archive,
    index: &'index Index<'_>,
    directory: Option<&SourceUse>,
    capture: &EntryCapture<'_>,
    selector: &AuthoredEntryPath,
    document_complete: bool,
    permit_repair: bool,
) -> Result<ResolvedTargets<'index>, CatalogError> {
    let directory_complete =
        directory.is_some() && crate::export_archive_health::inventory(archive).is_none();
    let (mut inputs, selector_complete) = selector.selector_inputs()?;
    inputs.extend(directory.cloned());
    let complete = selector_complete && document_complete && directory_complete;
    let literal = literal_candidates(index, selector.as_str());
    let authoring = if permit_repair && complete && literal.is_none() {
        authoring_candidates(index, selector.as_str())
    } else {
        None
    };
    let matched = literal.or_else(|| authoring.as_ref().map(|(_, entries)| *entries));
    let entries = matched.unwrap_or_default();
    let status = match entries {
        [_, _, ..] => LootContainerTargetStatus::Ambiguous,
        [] | [_] if !directory_complete => LootContainerTargetStatus::IncompleteDirectory,
        [] if matched.is_none() => LootContainerTargetStatus::UnsupportedSelector,
        [] => LootContainerTargetStatus::NotFound,
        [_] => LootContainerTargetStatus::Resolved,
    };
    let conversion_complete = if let Some((alias, _)) = &authoring {
        observe_repair(capture, selector, alias, &mut inputs)?
    } else {
        true
    };
    let complete = complete
        && conversion_complete
        && !matches!(status, LootContainerTargetStatus::UnsupportedSelector);
    let sources = capture.source_uses();
    let candidates = project_candidates(archive, index, capture, entries, &inputs, complete)?;
    let repair = if let Some((alias, [_])) = authoring {
        Some(Box::new(LootContainerLookupRepair {
            kind: sources.value(SourceValue::Present(alias.kind), &inputs, complete),
            normalized_path: sources.value(
                SourceValue::Present(alias.normalized),
                &inputs,
                complete,
            ),
        }))
    } else {
        None
    };
    Ok(ResolvedTargets {
        targets: LootContainerTargets {
            target_status: sources.value(SourceValue::Present(status), &inputs, complete),
            target_candidates: sources.value(SourceValue::Present(candidates), &inputs, complete),
            lookup_repair: repair,
        },
        candidates: entries,
    })
}

fn project_candidates(
    archive: &Archive,
    index: &Index<'_>,
    capture: &EntryCapture<'_>,
    entries: &[usize],
    inputs: &[SourceUse],
    complete: bool,
) -> Result<Vec<SourcedValue<PhysicalEntryRef>>, CatalogError> {
    let sources = capture.source_uses();
    let shared = sources.shared_origin(inputs, complete);
    entries
        .iter()
        .map(|&position| {
            let entry = index
                .entries
                .get(position)
                .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
            let original = EntrySource::outer(capture, archive, entry)?
                .receipt()
                .physical_reference(capture)?;
            let (uses, known) = original.contributions();
            sources
                .value_with_shared_origin(original.value().clone(), &shared, uses, known)
                .map_err(CatalogError::from)
        })
        .collect()
}

fn observe_repair(
    capture: &EntryCapture<'_>,
    selector: &AuthoredEntryPath,
    alias: &AuthoringPath,
    inputs: &mut Vec<SourceUse>,
) -> Result<bool, CatalogError> {
    let converted = selector
        .clone()
        .convert(capture, |_| Some(alias.normalized.clone()))?
        .ok_or(ExportAccountingError::ConflictingSourceDatum)?
        .retain_in(capture)?;
    let (uses, known) = converted.contributions();
    inputs.extend(uses.iter().cloned());
    Ok(known)
}
