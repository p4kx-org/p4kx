// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_archive::ObservedEntry;
use p4kx_texture::{DdsHeaderField, DdsRawHeader, MipByteSpans, MipPayloadLocation};

use crate::source::SourceEntry;
use crate::texture_parts::Parts;
use crate::{
    FieldOrigin, InputScope, ObservedFileOrigin, SourceEvidenceError, SourceInput, SourceLocation,
    SourceUse, SourceUseRole, SourceUses,
};

pub use crate::textures::preview_header::PreviewHeaderEvidence;

#[derive(Debug)]
pub(crate) struct PreviewInputs<'a, Spans> {
    pub(crate) color: TexturePreviewInput<'a, Spans>,
    pub(crate) alpha: Option<TexturePreviewInput<'a, Spans>>,
}

#[derive(Debug)]
pub(crate) struct TexturePreviewInput<'a, Spans> {
    header: ReadInput<'a>,
    payload: PayloadEntry<'a>,
    raw: DdsRawHeader,
    spans: Spans,
    disk_selection: Option<DiskSelection>,
}

#[derive(Debug)]
enum PayloadEntry<'a> {
    Header,
    Split(ReadInput<'a>),
}

#[derive(Debug)]
enum ReadInput<'archive> {
    Archive(ObservedEntry<'archive>),
    File(ObservedFileOrigin),
    Unknown,
}

impl ReadInput<'_> {
    const fn input(&self) -> SourceInput<'_> {
        match self {
            Self::Archive(entry) => SourceInput::ArchiveView(*entry),
            Self::File(file) => SourceInput::File(file),
            Self::Unknown => SourceInput::Unknown,
        }
    }

    const fn is_bound(&self) -> bool {
        !matches!(self, Self::Unknown)
    }
}

#[derive(Debug)]
pub(crate) struct DiskSelection {
    pub(crate) raw: DdsRawHeader,
    pub(crate) input: Option<ObservedFileOrigin>,
}

impl<'a> TexturePreviewInput<'a, MipPayloadLocation> {
    pub(crate) fn of_read(
        parts: &Parts<'a>,
        raw: DdsRawHeader,
        location: MipPayloadLocation,
    ) -> Self {
        let archive_input =
            |entry: Option<ObservedEntry<'a>>| entry.map_or(ReadInput::Unknown, ReadInput::Archive);
        let (header, payload) = match parts {
            Parts::Source { header, parts, .. } => (
                archive_input(header.observed_entry()),
                location.part_index().map_or(PayloadEntry::Header, |part| {
                    PayloadEntry::Split(archive_input(
                        parts.get(&part).and_then(SourceEntry::observed_entry),
                    ))
                }),
            ),
            Parts::Files(_) => (
                ReadInput::Unknown,
                location.part_index().map_or(PayloadEntry::Header, |_| {
                    PayloadEntry::Split(ReadInput::Unknown)
                }),
            ),
        };
        Self {
            header,
            payload,
            raw,
            spans: location,
            disk_selection: None,
        }
    }

    pub(crate) fn from_disk(
        raw: DdsRawHeader,
        location: MipPayloadLocation,
        header: Option<ObservedFileOrigin>,
        part: Option<ObservedFileOrigin>,
        selection: DiskSelection,
    ) -> Self {
        Self {
            header: header.map_or(ReadInput::Unknown, ReadInput::File),
            payload: location.part_index().map_or(PayloadEntry::Header, |_| {
                PayloadEntry::Split(part.map_or(ReadInput::Unknown, ReadInput::File))
            }),
            raw,
            spans: location,
            disk_selection: Some(selection),
        }
    }

    fn selected(self, image: Option<u64>) -> Option<TexturePreviewInput<'a, MipByteSpans>> {
        Some(TexturePreviewInput {
            header: self.header,
            payload: self.payload,
            raw: self.raw,
            spans: self.spans.spans(image)?,
            disk_selection: self.disk_selection,
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
        let header = PreviewHeaderEvidence::of_read(uses, self.header.input(), self.raw)?;
        let sibling = match &self.payload {
            PayloadEntry::Header => None,
            PayloadEntry::Split(entry) => Some(uses.observe(entry.input(), InputScope::Payload)?),
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
        let selection = self
            .disk_selection
            .as_ref()
            .map(|selection| {
                PreviewHeaderEvidence::of_read(
                    uses,
                    selection
                        .input
                        .as_ref()
                        .map_or(SourceInput::Unknown, SourceInput::File),
                    selection.raw,
                )
            })
            .transpose()?;
        let complete = self.header.is_bound()
            && match &self.payload {
                PayloadEntry::Header => true,
                PayloadEntry::Split(input) => input.is_bound(),
            }
            && self
                .disk_selection
                .as_ref()
                .is_none_or(|selection| selection.input.is_some());
        Ok(PreviewTextureEvidence {
            header,
            mip,
            selection,
            complete,
        })
    }
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
            if let Some(selection) = &texture.selection {
                uses.extend(selection.header_uses(&fields, SourceUseRole::Selector)?);
            }
            uses.extend(texture.header_uses(&fields, SourceUseRole::Conversion)?);
            uses.push(texture.header_size_use()?);
            uses.push(texture.mip().clone());
        }
        Ok(())
    }

    pub(crate) fn is_complete(&self) -> bool {
        self.color.complete && self.alpha.as_ref().is_none_or(|alpha| alpha.complete)
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
    selection: Option<PreviewHeaderEvidence>,
    complete: bool,
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
        self.selection
            .as_ref()
            .unwrap_or(&self.header)
            .name_use(role)
    }

    /// Registers the header Entry length used to locate the selected mip storage.
    ///
    /// # Errors
    ///
    /// Returns an error if the evidence inventory was interrupted.
    pub fn header_size_use(&self) -> Result<SourceUse, SourceEvidenceError> {
        self.selection.as_ref().unwrap_or(&self.header).size_use()
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
