// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::{SourceEvidenceError, SourceLocation, SourceScope, SourceUse, SourceUseRole};

#[derive(Clone, Copy)]
pub(crate) enum ChunkHeaderField {
    Signature,
    Version,
    RowCount,
    TableOffset,
}

impl ChunkHeaderField {
    pub(crate) fn observe(
        self,
        scope: &SourceScope,
        role: SourceUseRole,
    ) -> Result<SourceUse, SourceEvidenceError> {
        let (field, bytes) = match self {
            Self::Signature => ("signature", 0..4),
            Self::Version => ("version", 4..8),
            Self::RowCount => ("rowCount", 8..12),
            Self::TableOffset => ("tableOffset", 12..16),
        };
        scope.at(
            SourceLocation::Subresource {
                namespace: "cryEngineChunkHeader".into(),
                indices: vec![bytes.start, bytes.end],
                field: field.into(),
            },
            role,
        )
    }
}
