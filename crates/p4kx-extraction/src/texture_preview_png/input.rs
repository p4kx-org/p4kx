// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_archive::ObservedEntry;
use p4kx_gltf::TextureComponents;
use p4kx_texture::{
    DdsHeader, DdsHeaderField, DdsPngEncoding, DdsRawHeader, MipByteSpans, MipPayloadLocation,
    TextureError,
};

use crate::source::SourceEntry;
use crate::texture_parts::Parts;
use crate::texture_preview_png::evidence::{PreviewPngEvidence, PreviewTextureEvidence};
use crate::{
    InputScope, ObservedFileOrigin, SourceEvidenceError, SourceInput, SourceLocation, SourceUse,
    SourceUseRole, SourceUses,
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

impl<Spans> PreviewInputs<'_, Spans> {
    pub(crate) fn color_header_evidence(
        &self,
        uses: &SourceUses,
    ) -> Result<(PreviewHeaderEvidence, bool), SourceEvidenceError> {
        let header = &self.color.header;
        Ok((
            PreviewHeaderEvidence::of_read(uses, header.input(), self.color.raw)?,
            header.is_bound(),
        ))
    }

    pub(crate) fn png_encoding(&self) -> Result<DdsPngEncoding, TextureError> {
        let raw = &self.color.raw;
        Ok(DdsPngEncoding::of(
            DdsHeader::from_raw(raw)?.format(),
            raw.transfer().effective,
        ))
    }
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
    pub(crate) fn register_image(
        &self,
        uses: &SourceUses,
        level: u32,
        image: u64,
    ) -> Result<Option<PreviewPngEvidence>, SourceEvidenceError> {
        let Some(color) = self.color.spans.spans(Some(image)) else {
            return Ok(None);
        };
        let alpha = match &self.alpha {
            Some(input) => {
                let Some(spans) = input.spans.spans(Some(image)) else {
                    return Ok(None);
                };
                Some(input.register_spans(uses, level, spans)?)
            }
            None => None,
        };
        Ok(Some(PreviewPngEvidence {
            png_encoding: None,
            color: self.color.register_spans(uses, level, color)?,
            alpha,
        }))
    }

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
    pub(crate) fn glb_uses(
        &self,
        uses: &SourceUses,
        level: u32,
        components: TextureComponents,
        alpha_limits_level: bool,
    ) -> Result<Vec<SourceUse>, SourceEvidenceError> {
        use TextureComponents::{Alpha, Color, ColorAndAlpha};
        let mut fields = Vec::new();
        let alpha = matches!(components, Alpha | ColorAndAlpha);
        let color = matches!(components, Color | ColorAndAlpha) || (alpha && self.alpha.is_none());
        self.color
            .append_glb_uses(uses, level, color, &mut fields)?;
        if let Some(tail) = &self.alpha
            && (alpha || alpha_limits_level)
        {
            tail.append_glb_uses(uses, level, alpha, &mut fields)?;
        }
        Ok(fields)
    }

    pub(crate) fn register(
        &self,
        uses: &SourceUses,
        level: u32,
    ) -> Result<PreviewPngEvidence, SourceEvidenceError> {
        Ok(PreviewPngEvidence {
            png_encoding: None,
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
    fn append_glb_uses(
        &self,
        uses: &SourceUses,
        level: u32,
        pixels: bool,
        output: &mut Vec<SourceUse>,
    ) -> Result<(), SourceEvidenceError> {
        use DdsHeaderField::{ArraySize, Depth, Faces, Format, Height, MipCount, Prefix, Width};
        let dimensions = [Width, Height, MipCount, Faces, ArraySize, Depth];
        if pixels {
            let decoded = self.register(uses, level)?;
            let fields = [
                Prefix, Format, Width, Height, MipCount, Faces, ArraySize, Depth,
            ];
            output.extend(decoded.header_uses(&fields, SourceUseRole::Conversion)?);
            output.push(decoded.header_size_use()?);
            output.push(decoded.mip().clone());
        } else {
            let header = PreviewHeaderEvidence::of_read(uses, self.header.input(), self.raw)?;
            output.extend(header.header_uses(&dimensions, SourceUseRole::Selector)?);
        }
        Ok(())
    }

    fn register(
        &self,
        uses: &SourceUses,
        level: u32,
    ) -> Result<PreviewTextureEvidence, SourceEvidenceError> {
        self.register_spans(uses, level, self.spans)
    }
}

impl<Spans> TexturePreviewInput<'_, Spans> {
    fn register_spans(
        &self,
        uses: &SourceUses,
        level: u32,
        spans: MipByteSpans,
    ) -> Result<PreviewTextureEvidence, SourceEvidenceError> {
        let header = PreviewHeaderEvidence::of_read(uses, self.header.input(), self.raw)?;
        let sibling = match &self.payload {
            PayloadEntry::Header => None,
            PayloadEntry::Split(entry) => Some(uses.observe(entry.input(), InputScope::Payload)?),
        };
        let payload = sibling.as_ref().unwrap_or(&header.scope);
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
        Ok(PreviewTextureEvidence::new(
            header, mip, selection, complete,
        ))
    }
}
