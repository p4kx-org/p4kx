// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;
use std::ops::Range;

use p4kx_model::ModelFile;

use crate::source_provenance::ChunkHeaderField;
use crate::{
    InputScope, ModelArchiveSource, ModelSource, SourceInput, SourceLocation, SourceScope,
    SourceUse, SourceUseRole, SourceUses,
};

pub(crate) struct ModelGeometryScope {
    scope: SourceScope,
    pub(crate) complete: bool,
}

impl ModelGeometryScope {
    pub(crate) fn capture(
        uses: &SourceUses,
        model: &ModelSource,
        file: ModelFile,
    ) -> io::Result<Option<Self>> {
        model
            .archive_sources()
            .iter()
            .find(|source| source.file() == file)
            .map_or_else(|| Ok(None), |source| Self::from_model_read(uses, source))
    }

    pub(crate) fn from_model_read(
        uses: &SourceUses,
        source: &ModelArchiveSource,
    ) -> io::Result<Option<Self>> {
        let Some(entry) = source.origin() else {
            return Ok(None);
        };
        Ok(Some(Self {
            scope: uses
                .observe(SourceInput::Archive(entry), InputScope::Payload)
                .map_err(io::Error::other)?,
            complete: entry.ancestors().all(|ancestor| ancestor.is_some()),
        }))
    }

    pub(crate) const fn scope(&self) -> &SourceScope {
        &self.scope
    }

    pub(crate) fn chunk_header(&self, field: ChunkHeaderField) -> io::Result<SourceUse> {
        field
            .observe(&self.scope, SourceUseRole::Payload)
            .map_err(io::Error::other)
    }

    pub(crate) fn directory_path_selector(&self) -> io::Result<SourceUse> {
        self.directory_path(SourceUseRole::Selector)
    }

    pub(crate) fn directory_path(&self, role: SourceUseRole) -> io::Result<SourceUse> {
        self.scope
            .for_scope(InputScope::ArchiveDirectoryMetadata)
            .at(
                SourceLocation::Metadata {
                    field: "path".into(),
                },
                role,
            )
            .map_err(io::Error::other)
    }

    pub(crate) fn selection_at(&self, original: &SourceUse) -> io::Result<SourceUse> {
        self.scope
            .at_observed_location(original, SourceUseRole::Selector)
            .map_err(io::Error::other)
    }

    pub(crate) fn word(
        &self,
        range: Range<u64>,
        field: &str,
        role: SourceUseRole,
    ) -> io::Result<SourceUse> {
        self.scope
            .at(
                SourceLocation::Subresource {
                    namespace: "model-byte-range".into(),
                    indices: vec![range.start, range.end],
                    field: field.into(),
                },
                role,
            )
            .map_err(io::Error::other)
    }
}
