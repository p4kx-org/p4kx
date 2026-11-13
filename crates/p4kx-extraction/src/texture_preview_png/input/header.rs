// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_texture::{DdsHeaderField, DdsPngEncoding, DdsRawHeader};

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
        file: Option<&crate::ObservedFileOrigin>,
    ) -> Result<Self, SourceEvidenceError> {
        let input = match parts {
            Parts::Source { header, .. } => header.observed_entry().map(SourceInput::ArchiveView),
            Parts::Files(_) => file.map(SourceInput::File),
        };
        Self::of_read(uses, input.unwrap_or(SourceInput::Unknown), raw)
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

    pub(crate) fn output_transfer_uses(
        &self,
        encoding: DdsPngEncoding,
    ) -> Result<Vec<SourceUse>, SourceEvidenceError> {
        let mut uses = self.header_uses(&[DdsHeaderField::Prefix], SourceUseRole::Selector)?;
        uses.extend(
            self.header_uses(encoding.output_transfer_fields(), SourceUseRole::Conversion)?,
        );
        Ok(uses)
    }

    /// Registers an exact retained byte range inside the observed DDS header.
    ///
    /// # Errors
    /// Returns an error if the evidence inventory was interrupted.
    pub fn byte_use(
        &self,
        offset: u64,
        length: u64,
        field: &str,
        role: SourceUseRole,
    ) -> Result<SourceUse, SourceEvidenceError> {
        self.scope.at(
            SourceLocation::Subresource {
                namespace: "ddsHeader".into(),
                indices: vec![offset, length],
                field: field.into(),
            },
            role,
        )
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

    /// Registers the original header Entry or file size used for its byte layout.
    ///
    /// # Errors
    /// Returns an error if the evidence inventory was interrupted.
    pub fn size_use(&self) -> Result<SourceUse, SourceEvidenceError> {
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
