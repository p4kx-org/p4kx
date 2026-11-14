// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DataForge, Filter, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use crate::CatalogError;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_read::loose_archive_path::{self, LooseArchivePath, LooseArchivePathState};
use crate::catalog_read::loose_record_link::{self, LooseRecordCandidate};
use crate::catalog_read::original::EntryCapture;
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
        candidates: Vec<LooseRecordCandidate>,
    },
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
        let resolved = loose_record_link::resolve(
            path,
            original,
            forge.records_at_path(authored),
            PALETTE,
            filter,
            cancel,
        )?;
        Ok(Some(Self::DataForge {
            state: resolved.state,
            candidates: resolved.candidates,
        }))
    }
}
