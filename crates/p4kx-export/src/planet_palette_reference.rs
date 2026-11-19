// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DataForge, Filter, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use crate::CatalogError;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_read::loose_archive_path::{
    self, LooseArchivePath, LooseArchivePathState, Resolver,
};
use crate::catalog_read::loose_record_link::{self, LooseRecordCandidate, LooseRecordLink};
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
#[expect(
    missing_docs,
    reason = "variant docs would change the shared schema descriptions"
)]
#[non_exhaustive]
pub enum PlanetPaletteReference {
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
    /// Returns the typed lookup outcome when one was established.
    #[must_use]
    pub const fn state(&self) -> Option<LooseArchivePathState> {
        match self {
            Self::Unclassified { state } | Self::DataForge { state, .. } => state.value().copied(),
            Self::ArchiveEntry { resolution } => resolution.state.value().copied(),
        }
    }

    pub(super) fn resolve(
        path: &SourcedValue<String>,
        paths: &mut Resolver<'_, '_, '_>,
        capture: &EntryCapture<'_>,
        palette_record: impl FnOnce(
            &SourcedValue<String>,
            &str,
        ) -> Result<LooseRecordLink, CatalogError>,
    ) -> Result<Self, CatalogError> {
        let Some(authored) = path.value().filter(|path| loose_archive_path::valid(path)) else {
            return Ok(Self::Unclassified {
                state: loose_archive_path::from_field(path, capture)?.state,
            });
        };
        if starts_with_folder(authored, RECORDS) {
            let resolved = palette_record(path, authored)?;
            return Ok(Self::DataForge {
                state: resolved.state,
                candidates: resolved.candidates,
            });
        }
        let resolution = paths.resolve(path, |_| true)?;
        Ok(Self::ArchiveEntry { resolution })
    }
}

pub(super) fn resolve_record(
    path: &SourcedValue<String>,
    authored: &str,
    forge: &DataForge<'_>,
    original: &OriginalCatalogInput<'_, '_>,
    filter: Option<&Filter>,
    cancel: Option<&CancelToken>,
) -> Result<LooseRecordLink, CatalogError> {
    loose_record_link::resolve(
        path,
        original,
        forge.records_at_path(authored),
        PALETTE,
        filter,
        cancel,
    )
}
