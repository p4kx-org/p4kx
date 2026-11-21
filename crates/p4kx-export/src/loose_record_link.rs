// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    CancelToken, DataForge, Filter, Guid, Record, SourceUses, SourceValue, SourceValueState,
    SourcedValue,
};
use schemars::JsonSchema;
use serde::Serialize;

use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_read::loose_archive_path::LooseArchivePathState;
use crate::catalog_read::parsed_entry_json;
use crate::guid_text::CatalogGuid;
use crate::record_header_field::RecordHeaderField;
use crate::schema_fields::every_field_required;
use crate::{CatalogError, CatalogOptions};

/// Resolution of an authored loose JSON reference against original Record headers.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct LooseRecordLink {
    pub(crate) state: SourcedValue<LooseArchivePathState>,
    pub(crate) candidates: Vec<LooseRecordCandidate>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[expect(
    missing_docs,
    reason = "the original shared schema has no type description"
)]
pub struct LooseRecordCandidate {
    index: SourcedValue<u32>,
    guid: SourcedValue<CatalogGuid>,
    path: SourcedValue<String>,
    source_type: SourcedValue<String>,
}

impl LooseRecordCandidate {
    /// Borrows the original physical Record index.
    #[must_use]
    pub const fn index(&self) -> &SourcedValue<u32> {
        &self.index
    }

    /// Borrows the original Record GUID.
    #[must_use]
    pub const fn guid(&self) -> &SourcedValue<CatalogGuid> {
        &self.guid
    }

    /// Borrows the original Record path spelling.
    #[must_use]
    pub const fn path(&self) -> &SourcedValue<String> {
        &self.path
    }

    /// Borrows the original declared Record type.
    #[must_use]
    pub const fn source_type(&self) -> &SourcedValue<String> {
        &self.source_type
    }
}

impl LooseRecordLink {
    /// Borrows the sourced resolution state of the original Record reference.
    #[must_use]
    pub const fn state(&self) -> &SourcedValue<LooseArchivePathState> {
        &self.state
    }

    /// Borrows the original physical Record header candidates.
    #[must_use]
    pub const fn candidates(&self) -> &[LooseRecordCandidate] {
        self.candidates.as_slice()
    }
}

pub(crate) type GuidRecordLookup<'query> = dyn FnMut(Guid, &SourcedValue<CatalogGuid>, &str) -> Result<LooseRecordLink, CatalogError>
    + 'query;

pub(crate) fn by_guid(
    forge: &DataForge<'_>,
    original: &OriginalCatalogInput<'_, '_>,
    options: &CatalogOptions<'_>,
    selector: &SourcedValue<CatalogGuid>,
    raw: &SourcedValue<String>,
    expected_type: &str,
) -> Result<LooseRecordLink, CatalogError> {
    by_guid_using(
        &original.origins().uses,
        selector,
        raw,
        expected_type,
        &mut |guid, selector, expected_type| {
            resolve(
                selector,
                original,
                forge.records_by_guid(guid),
                expected_type,
                options.filter,
                options.cancel,
            )
        },
    )
}

pub(crate) fn by_guid_using(
    sources: &SourceUses,
    selector: &SourcedValue<CatalogGuid>,
    raw: &SourcedValue<String>,
    expected_type: &str,
    lookup: &mut GuidRecordLookup<'_>,
) -> Result<LooseRecordLink, CatalogError> {
    if let Some(guid) = selector.value().filter(|guid| guid.0 != Guid::default()) {
        return lookup(guid.0, selector, expected_type);
    }
    let state = match (selector.state(), raw.value()) {
        (SourceValueState::Absent, _) => LooseArchivePathState::Absent,
        (SourceValueState::Null, _) => LooseArchivePathState::Null,
        (SourceValueState::Present, _) => LooseArchivePathState::Empty,
        (_, Some(raw)) if raw.is_empty() => LooseArchivePathState::Empty,
        _ => LooseArchivePathState::Invalid,
    };
    Ok(LooseRecordLink {
        state: sources.value_with_origins(SourceValue::Present(state), &[selector.origin()])?,
        candidates: Vec::new(),
    })
}

pub(crate) fn resolve_guid(
    guid: Guid,
    selector: &SourcedValue<CatalogGuid>,
    original: &OriginalCatalogInput<'_, '_>,
    expected_type: &str,
    filter: Option<&Filter>,
    cancel: Option<&CancelToken>,
) -> Result<LooseRecordLink, CatalogError> {
    resolve(
        selector,
        original,
        original.forge().records_by_guid(guid),
        expected_type,
        filter,
        cancel,
    )
}

pub(crate) fn resolve<'record, T>(
    selector: &SourcedValue<T>,
    original: &OriginalCatalogInput<'_, '_>,
    records: impl Iterator<Item = Record<'record>>,
    expected_type: &str,
    filter: Option<&Filter>,
    cancel: Option<&CancelToken>,
) -> Result<LooseRecordLink, CatalogError> {
    let mut candidates = Vec::new();
    let mut unreadable = false;
    let mut matching = 0;
    let mut included = false;
    for record in records {
        parsed_entry_json::check_cancel(cancel)?;
        let (Ok(source_type), Ok(record_path)) = (record.type_name(), record.record_path()) else {
            unreadable = true;
            continue;
        };
        if source_type == expected_type {
            matching += 1;
            included |= filter.is_none_or(|filter| filter.matches(record_path));
        }
        let candidate = candidate(original, &record, record_path, source_type)?;
        parsed_entry_json::push(&mut candidates, candidate)?;
    }
    let state = match matching {
        0 | 1 if unreadable => LooseArchivePathState::Unreadable,
        0 if !candidates.is_empty() => LooseArchivePathState::WrongFamily,
        0 => LooseArchivePathState::Missing,
        1 if !included => LooseArchivePathState::Filtered,
        1 => LooseArchivePathState::Resolved,
        _ => LooseArchivePathState::Ambiguous,
    };
    let sources = &original.origins().uses;
    let unknown = sources.origin(&[], false);
    let mut origins = vec![selector.origin()];
    if matches!(
        state,
        LooseArchivePathState::Missing | LooseArchivePathState::Unreadable
    ) {
        origins.push(&unknown);
    }
    for candidate in &candidates {
        origins.extend([
            candidate.path.origin(),
            candidate.source_type.origin(),
            candidate.guid.origin(),
        ]);
    }
    let state = sources.value_with_origins(SourceValue::Present(state), &origins)?;
    Ok(LooseRecordLink { state, candidates })
}

fn candidate(
    original: &OriginalCatalogInput<'_, '_>,
    record: &Record<'_>,
    record_path: &str,
    source_type: &str,
) -> Result<LooseRecordCandidate, CatalogError> {
    let context = original.record_context(record)?;
    Ok(LooseRecordCandidate {
        index: context.header_selector(
            RecordHeaderField::RecordIndex,
            SourceValue::Present(record.index()),
        )?,
        guid: context.header_value(
            RecordHeaderField::Guid,
            SourceValue::Present(CatalogGuid(record.guid())),
        )?,
        path: context.header_conversion(
            RecordHeaderField::FileName,
            SourceValue::Present(record_path.to_owned()),
        )?,
        source_type: context.header_selector(
            RecordHeaderField::SourceType,
            SourceValue::Present(source_type.to_owned()),
        )?,
    })
}
