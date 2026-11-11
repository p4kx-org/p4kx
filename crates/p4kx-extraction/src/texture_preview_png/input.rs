// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_archive::ObservedEntry;
use p4kx_texture::{DdsHeaderField, DdsRawHeader, MipByteSpans, MipPayloadLocation};

use crate::source::SourceEntry;
use crate::texture_parts::Parts;
use crate::{
    FieldOrigin, InputScope, SourceEvidenceError, SourceInput, SourceLocation, SourceScope,
    SourceUse, SourceUseRole, SourceUses,
};

#[derive(Debug)]
pub(crate) struct PreviewInputs<'a, Spans> {
    pub(crate) color: TexturePreviewInput<'a, Spans>,
    pub(crate) alpha: Option<TexturePreviewInput<'a, Spans>>,
}

#[derive(Debug)]
pub(crate) struct TexturePreviewInput<'a, Spans> {
    header: Option<ObservedEntry<'a>>,
    payload: PayloadEntry<'a>,
    raw: DdsRawHeader,
    spans: Spans,
}

#[derive(Debug)]
enum PayloadEntry<'a> {
    Header,
    Split(Option<ObservedEntry<'a>>),
}

impl<'a> TexturePreviewInput<'a, MipPayloadLocation> {
    pub(crate) fn of_read(
        parts: &Parts<'a>,
        raw: DdsRawHeader,
        location: MipPayloadLocation,
    ) -> Self {
        let (header, payload) = match parts {
            Parts::Source { header, parts, .. } => (
                header.observed_entry(),
                location.part_index().map_or(PayloadEntry::Header, |part| {
                    PayloadEntry::Split(parts.get(&part).and_then(SourceEntry::observed_entry))
                }),
            ),
            Parts::Files(_) => (
                None,
                location
                    .part_index()
                    .map_or(PayloadEntry::Header, |_| PayloadEntry::Split(None)),
            ),
        };
        Self {
            header,
            payload,
            raw,
            spans: location,
        }
    }

    fn selected(self, image: Option<u64>) -> Option<TexturePreviewInput<'a, MipByteSpans>> {
        Some(TexturePreviewInput {
            header: self.header,
            payload: self.payload,
            raw: self.raw,
            spans: self.spans.spans(image)?,
        })
    }
}

impl<'a> PreviewInputs<'a, MipPayloadLocation> {
    pub(crate) fn selected(self, image: Option<u64>) -> Option<PreviewInputs<'a, MipByteSpans>> {
        Some(PreviewInputs {
            color: self.color.selected(image)?,
            alpha: match self.alpha {
                Some(alpha) => Some(alpha.selected(image)?),
                None => None,
            },
        })
    }
}

impl PreviewInputs<'_, MipByteSpans> {
    pub(crate) fn register(
        &self,
        uses: &SourceUses,
        level: u32,
    ) -> Result<PreviewPngEvidence, SourceEvidenceError> {
        Ok(PreviewPngEvidence {
            color: self.color.register(uses, level)?,
            alpha: self
                .alpha
                .as_ref()
                .map(|alpha| alpha.register(uses, level))
                .transpose()?,
        })
    }
}

impl TexturePreviewInput<'_, MipByteSpans> {
    fn register(
        &self,
        uses: &SourceUses,
        level: u32,
    ) -> Result<PreviewTextureEvidence, SourceEvidenceError> {
        let header = PreviewHeaderEvidence::of_read(uses, self.header, self.raw)?;
        let sibling = match self.payload {
            PayloadEntry::Header => None,
            PayloadEntry::Split(entry) => Some(observe(uses, entry)?),
        };
        let payload = sibling.as_ref().unwrap_or(&header.scope);
        let spans = self.spans;
        let mip = payload.at(
            SourceLocation::Subresource {
                namespace: "ddsMipPayload".into(),
                indices: vec![
                    u64::from(level),
                    spans.offset,
                    spans.length,
                    spans.stride,
                    spans.count,
                ],
                field: "bytes".into(),
            },
            SourceUseRole::Conversion,
        )?;
        Ok(PreviewTextureEvidence { header, mip })
    }
}

fn observe(
    uses: &SourceUses,
    entry: Option<ObservedEntry<'_>>,
) -> Result<SourceScope, SourceEvidenceError> {
    uses.observe(
        entry.map_or(SourceInput::Unknown, SourceInput::ArchiveView),
        InputScope::Payload,
    )
}

/// Registered source evidence for the selected color and optional merged alpha texture.
#[derive(Debug)]
#[non_exhaustive]
pub struct PreviewPngEvidence {
    /// The primary texture, including previews that open an alpha DDS directly.
    pub color: PreviewTextureEvidence,
    /// The successfully merged alpha or smoothness tail.
    pub alpha: Option<PreviewTextureEvidence>,
}

impl PreviewPngEvidence {
    /// Registers the selected color and alpha inputs that supplied the preview pixels.
    ///
    /// # Errors
    ///
    /// Returns an error if the evidence inventory was interrupted.
    pub fn append_pixel_uses(&self, uses: &mut Vec<SourceUse>) -> Result<(), SourceEvidenceError> {
        use DdsHeaderField::{ArraySize, Depth, Faces, Format, Height, MipCount, Prefix, Width};
        let fields = [
            Prefix, Format, Width, Height, MipCount, Faces, ArraySize, Depth,
        ];
        for texture in std::iter::once(&self.color).chain(self.alpha.as_ref()) {
            uses.extend(texture.header_uses(&fields, SourceUseRole::Conversion)?);
            uses.push(texture.header_size_use()?);
            uses.push(texture.mip().clone());
        }
        Ok(())
    }

    /// Registers the stored names used to select the color header and its alpha sibling.
    ///
    /// # Errors
    ///
    /// Returns an error if the evidence inventory was interrupted.
    pub fn alpha_lookup_uses(&self) -> Result<Vec<SourceUse>, SourceEvidenceError> {
        std::iter::once(&self.color)
            .chain(self.alpha.as_ref())
            .map(|texture| texture.name_use(SourceUseRole::Selector))
            .collect()
    }
}

/// The observed header and selected mip bytes of one texture in the decoded image.
#[derive(Debug)]
pub struct PreviewTextureEvidence {
    header: PreviewHeaderEvidence,
    mip: SourceUse,
}

impl PreviewTextureEvidence {
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
        self.header.header_uses(fields, role)
    }

    /// Returns the original header evidence.
    #[must_use]
    pub const fn header(&self) -> &PreviewHeaderEvidence {
        &self.header
    }

    /// Returns the selected mip payload that supplied pixels to the preview.
    #[must_use]
    pub const fn mip(&self) -> &SourceUse {
        &self.mip
    }

    /// Registers the original stored filename used for this texture's lookup or display.
    ///
    /// # Errors
    ///
    /// Returns an error if the evidence inventory was interrupted.
    pub fn name_use(&self, role: SourceUseRole) -> Result<SourceUse, SourceEvidenceError> {
        self.header.name_use(role)
    }

    /// Registers the header Entry length used to locate the selected mip storage.
    ///
    /// # Errors
    ///
    /// Returns an error if the evidence inventory was interrupted.
    pub fn header_size_use(&self) -> Result<SourceUse, SourceEvidenceError> {
        self.header
            .metadata_use("uncompressedSize", SourceUseRole::Selector)
    }

    /// Registers the displayed stored names of the header and its containers.
    ///
    /// # Errors
    ///
    /// Returns an error if the evidence inventory was interrupted.
    pub fn path_origin(&self, uses: &SourceUses) -> Result<FieldOrigin, SourceEvidenceError> {
        self.header.path_origin(uses)
    }
}

/// The authored words of an actual opened texture header.
#[derive(Debug)]
pub struct PreviewHeaderEvidence {
    raw: DdsRawHeader,
    scope: SourceScope,
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
        Self::of_read(uses, entry, raw)
    }

    fn of_read(
        uses: &SourceUses,
        entry: Option<ObservedEntry<'_>>,
        raw: DdsRawHeader,
    ) -> Result<Self, SourceEvidenceError> {
        Ok(Self {
            raw,
            scope: observe(uses, entry)?,
            archive_ancestry: entry.map(|entry| entry.ancestors().len()),
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
        self.metadata_use("path", role)
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
