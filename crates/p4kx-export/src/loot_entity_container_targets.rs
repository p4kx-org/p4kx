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

#[derive(Debug, Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(extend("x-p4kx-vocabularyClass" = "control"))]
enum LootContainerTargetStatus {
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
    target_status: SourcedValue<LootContainerTargetStatus>,
    target_candidates: SourcedValue<Vec<SourcedValue<PhysicalEntryRef>>>,
}

pub(super) fn resolve(
    archive: &Archive,
    index: &Index<'_>,
    directory: Option<&SourceUse>,
    capture: &EntryCapture<'_>,
    selector: &AuthoredEntryPath,
    document_complete: bool,
) -> Result<LootContainerTargets, CatalogError> {
    let directory_complete =
        directory.is_some() && crate::export_archive_health::inventory(archive).is_none();
    let entries = index.candidates(selector.as_str());
    let status = match entries {
        [_, _, ..] => LootContainerTargetStatus::Ambiguous,
        [] | [_] if !directory_complete => LootContainerTargetStatus::IncompleteDirectory,
        [] if ChildContainerPath::new(selector.as_str()).is_none() => {
            LootContainerTargetStatus::UnsupportedSelector
        }
        [] => LootContainerTargetStatus::NotFound,
        [_] => LootContainerTargetStatus::Resolved,
    };
    let sources = capture.source_uses();
    let (mut inputs, complete) = selector.selector_inputs()?;
    inputs.extend(directory.cloned());
    let complete = complete
        && document_complete
        && directory_complete
        && !matches!(status, LootContainerTargetStatus::UnsupportedSelector);
    let shared = sources.shared_origin(&inputs, complete);
    let candidates = entries
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
        .collect::<Result<Vec<_>, _>>()?;
    Ok(LootContainerTargets {
        target_status: sources.value(SourceValue::Present(status), &inputs, complete),
        target_candidates: sources.value(SourceValue::Present(candidates), &inputs, complete),
    })
}
