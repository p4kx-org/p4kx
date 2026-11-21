// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::{DdsHeader, DdsRawHeader, Mip, TextureError};

/// Exact decoded input byte spans at a fixed stride.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct MipByteSpans {
    /// Offset of the first span in the uncompressed source file.
    pub offset: u64,
    /// Number of bytes in each span.
    pub length: u64,
    /// Distance between the starts of adjacent spans, or zero for a single span.
    pub stride: u64,
    /// Number of spans.
    pub count: u64,
}

impl MipByteSpans {
    const fn single(offset: u64, length: u64) -> Self {
        Self {
            offset,
            length,
            stride: 0,
            count: 1,
        }
    }
}

/// The selected mip and the original bytes interpreted by its reader.
#[derive(Debug)]
#[non_exhaustive]
pub struct LocatedMip {
    /// The bytes available to the mip decoder.
    pub mip: Mip,
    /// The raw DDS header read to interpret this mip.
    pub raw_header: DdsRawHeader,
    /// The physical location of the retained mip bytes.
    pub payload: MipPayloadLocation,
}

/// Physical placement of the bytes returned by a successful mip read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MipPayloadLocation {
    part: Option<u32>,
    spans: MipByteSpans,
    images_per_span: u64,
}

impl MipPayloadLocation {
    pub(crate) const fn part(index: u32, length: u64, dds: &DdsHeader, level: u32) -> Self {
        Self {
            part: Some(index),
            spans: MipByteSpans::single(0, length),
            images_per_span: dds.images(level),
        }
    }

    pub(crate) fn tail(
        dds: &DdsHeader,
        level: u32,
        offset: u64,
        length: u64,
        gap: u64,
    ) -> Result<Self, TextureError> {
        let count = dds.surfaces();
        let stride = if count == 1 {
            0
        } else {
            length
                .checked_add(gap)
                .ok_or_else(|| TextureError::UnsupportedFormat(dds.format()))?
        };
        Ok(Self {
            part: None,
            spans: MipByteSpans {
                offset,
                length,
                stride,
                count,
            },
            images_per_span: u64::from(dds.slices(level)),
        })
    }

    /// Returns the opened split sibling's index, or None for the header Entry.
    #[must_use]
    pub const fn part_index(&self) -> Option<u32> {
        self.part
    }

    /// Returns every retained mip span, or the span of one selected decoded image.
    #[must_use]
    pub fn spans(&self, image: Option<u64>) -> Option<MipByteSpans> {
        let Some(image) = image else {
            return Some(self.spans);
        };
        let surface = image / self.images_per_span;
        if surface >= self.spans.count {
            return None;
        }
        let slice = image % self.images_per_span;
        let length = self.spans.length / self.images_per_span;
        let start = self
            .spans
            .offset
            .checked_add(surface.checked_mul(self.spans.stride)?)?
            .checked_add(slice.checked_mul(length)?)?;
        Some(MipByteSpans::single(start, length))
    }
}
