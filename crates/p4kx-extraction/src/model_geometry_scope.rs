// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;
use std::ops::Range;

use p4kx_model::ModelFile;

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
        let Some(entry) = model
            .archive_sources()
            .iter()
            .find(|source| source.file() == file)
            .and_then(ModelArchiveSource::origin)
        else {
            return Ok(None);
        };
        Ok(Some(Self {
            scope: uses
                .observe(SourceInput::Archive(entry), InputScope::Payload)
                .map_err(io::Error::other)?,
            complete: entry.ancestors().all(|ancestor| ancestor.is_some()),
        }))
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
