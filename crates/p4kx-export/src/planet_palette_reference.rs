// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DataForge, Filter, Record, SourceValue, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use crate::CatalogError;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_read::loose_archive_path::{self, LooseArchivePath, LooseArchivePathState};
use crate::catalog_read::original::EntryCapture;
use crate::catalog_read::parsed_entry_json;
use crate::guid_text::CatalogGuid;
use crate::record_header_field::RecordHeaderField;
use crate::schema_fields::every_field_required;
use crate::selection::starts_with_folder;

const PALETTE: &str = "TintPaletteTree";
const RECORDS: &str = "libs/foundry/records/";

/// A tint palette library path and its physical target candidates.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "targetKind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub(crate) enum PlanetPaletteReference {
    Unclassified {
        state: SourcedValue<LooseArchivePathState>,
    },
    ArchiveEntry {
        resolution: LooseArchivePath,
    },
    DataForge {
        state: SourcedValue<LooseArchivePathState>,
        candidates: Vec<PaletteRecordCandidate>,
    },
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct PaletteRecordCandidate {
    index: SourcedValue<u32>,
    guid: SourcedValue<CatalogGuid>,
    path: SourcedValue<String>,
    source_type: SourcedValue<String>,
}

impl PlanetPaletteReference {
    pub(crate) const fn state(&self) -> Option<LooseArchivePathState> {
        match self {
            Self::Unclassified { state } | Self::DataForge { state, .. } => state.value().copied(),
            Self::ArchiveEntry { resolution } => resolution.state.value().copied(),
        }
    }

    pub(crate) fn of_path(
        path: &SourcedValue<String>,
        forge: &DataForge<'_>,
        original: &OriginalCatalogInput<'_, '_>,
        capture: &EntryCapture<'_>,
        filter: Option<&Filter>,
        cancel: Option<&CancelToken>,
    ) -> Result<Option<Self>, CatalogError> {
        let Some(authored) = path.value().filter(|path| loose_archive_path::valid(path)) else {
            return Ok(Some(Self::Unclassified {
                state: loose_archive_path::from_field(path, capture)?.state,
            }));
        };
        if !starts_with_folder(authored, RECORDS) {
            return Ok(None);
        }
        let mut candidates = Vec::new();
        let mut matching_family = 0;
        let mut unreadable = false;
        let mut included = false;
        for record in forge.records_at_path(authored) {
            parsed_entry_json::check_cancel(cancel)?;
            let (Ok(source_type), Ok(record_path)) = (record.type_name(), record.record_path())
            else {
                unreadable = true;
                continue;
            };
            if source_type == PALETTE {
                matching_family += 1;
                included |= filter.is_none_or(|filter| filter.matches(record_path));
            }
            candidates.push(palette_candidate(
                original,
                &record,
                record_path,
                source_type,
            )?);
        }
        let status = match matching_family {
            0 | 1 if unreadable => LooseArchivePathState::Unreadable,
            0 if !candidates.is_empty() => LooseArchivePathState::WrongFamily,
            0 => LooseArchivePathState::Missing,
            1 if !included => LooseArchivePathState::Filtered,
            1 => LooseArchivePathState::Resolved,
            _ => LooseArchivePathState::Ambiguous,
        };
        let sources = &original.origins().uses;
        let unknown = sources.origin(&[], false);
        let mut origins = vec![path.origin()];
        if matches!(
            status,
            LooseArchivePathState::Missing | LooseArchivePathState::Unreadable
        ) {
            origins.push(&unknown);
        }
        for candidate in &candidates {
            origins.extend([candidate.path.origin(), candidate.source_type.origin()]);
        }
        let state = sources.value_with_origins(SourceValue::Present(status), &origins)?;
        Ok(Some(Self::DataForge { state, candidates }))
    }
}

fn palette_candidate(
    original: &OriginalCatalogInput<'_, '_>,
    record: &Record<'_>,
    record_path: &str,
    source_type: &str,
) -> Result<PaletteRecordCandidate, CatalogError> {
    let context = original.record_context(record)?;
    Ok(PaletteRecordCandidate {
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
