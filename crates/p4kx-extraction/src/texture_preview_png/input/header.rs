// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_texture::{DdsHeaderField, DdsRawHeader};

use crate::texture_parts::Parts;
use crate::{
    FieldOrigin, InputScope, SourceEvidenceError, SourceInput, SourceLocation, SourceScope,
    SourceUse, SourceUseRole, SourceUses,
};

/// The authored words of an actual opened texture header.
#[derive(Debug)]
pub struct PreviewHeaderEvidence {
    raw: DdsRawHeader,
    pub(crate) scope: SourceScope,
    archive_ancestry: Option<usize>,
}

impl PreviewHeaderEvidence {
    pub(crate) fn of_header(
        uses: &SourceUses,
        parts: &Parts<'_>,
        raw: DdsRawHeader,
    ) -> Result<Self, SourceEvidenceError> {
        let entry = match parts {
            Parts::Source { header, .. } => header.observed_entry(),
            Parts::Files(_) => None,
        };
        Self::of_read(
            uses,
            entry.map_or(SourceInput::Unknown, SourceInput::ArchiveView),
            raw,
        )
    }

    pub(crate) fn of_read(
        uses: &SourceUses,
        input: SourceInput<'_>,
        raw: DdsRawHeader,
    ) -> Result<Self, SourceEvidenceError> {
        Ok(Self {
            raw,
            scope: uses.observe(input, InputScope::Payload)?,
            archive_ancestry: match input {
                SourceInput::ArchiveView(entry) => Some(entry.ancestors().len()),
                _ => None,
            },
        })
    }

    /// Registers the exact original words used by the requested header fields.
    ///
    /// # Errors
    ///
    /// Returns an error if the evidence inventory was interrupted.
    pub fn header_uses(
        &self,
        fields: &[DdsHeaderField],
        role: SourceUseRole,
    ) -> Result<Vec<SourceUse>, SourceEvidenceError> {
        fields
            .iter()
            .flat_map(|&field| self.raw.words_for(field))
            .map(|word| {
                self.scope.at(
                    SourceLocation::Subresource {
                        namespace: "ddsHeader".into(),
                        indices: vec![word.offset, word.length],
                        field: word.name.into(),
                    },
                    role,
                )
            })
            .collect()
    }

    /// Registers only the opened Entry's stored name.
    ///
    /// # Errors
    ///
    /// Returns an error if the evidence inventory was interrupted.
    pub fn name_use(&self, role: SourceUseRole) -> Result<SourceUse, SourceEvidenceError> {
        let field = if self.archive_ancestry.is_some() {
            "path"
        } else {
            "name"
        };
        self.metadata_use(field, role)
    }

    pub(crate) fn size_use(&self) -> Result<SourceUse, SourceEvidenceError> {
        let field = if self.archive_ancestry.is_some() {
            "uncompressedSize"
        } else {
            "byteLength"
        };
        self.metadata_use(field, SourceUseRole::Selector)
    }

    fn metadata_use(
        &self,
        field: &'static str,
        role: SourceUseRole,
    ) -> Result<SourceUse, SourceEvidenceError> {
        let scope = if self.archive_ancestry.is_some() {
            InputScope::ArchiveDirectoryMetadata
        } else {
            InputScope::FileMetadata
        };
        self.scope.for_scope(scope).at(
            SourceLocation::Metadata {
                field: field.into(),
            },
            role,
        )
    }

    /// Registers the displayed stored names of the header and its containers.
    ///
    /// # Errors
    ///
    /// Returns an error if the evidence inventory was interrupted.
    pub fn path_origin(&self, uses: &SourceUses) -> Result<FieldOrigin, SourceEvidenceError> {
        let mut names = vec![self.name_use(SourceUseRole::Payload)?];
        let mut complete = true;
        for ordinal in 0..self.archive_ancestry.unwrap_or(0) {
            if let Some(ancestor) = self.scope.ancestor(ordinal)? {
                names.push(
                    ancestor
                        .for_scope(InputScope::ArchiveDirectoryMetadata)
                        .at(
                            SourceLocation::Metadata {
                                field: "path".into(),
                            },
                            SourceUseRole::Payload,
                        )?,
                );
            } else {
                complete = false;
            }
        }
        Ok(uses.origin(&names, complete))
    }
}
