// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::format::{legacy_uses_alpha_mask, legacy_uses_four_cc};
use crate::header::{dx10_is_volume, legacy_is_volume, legacy_volume_uses_depth_flag};
use crate::{DdsDx10Fields, DdsLegacyFields, DdsRawHeader};

const WORD_BYTES: u64 = 4;

/// A decoded DDS header field whose authored words can be addressed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum DdsHeaderField {
    /// The format marker that locates the legacy header body.
    Prefix,
    /// The pixel format, including the legacy or DX10 format selector.
    Format,
    /// The width of mip zero.
    Width,
    /// The height of mip zero.
    Height,
    /// The number of mip levels.
    MipCount,
    /// The face count and cubemap indicator.
    Faces,
    /// The number of array elements.
    ArraySize,
    /// The volume depth of mip zero.
    Depth,
    /// The original format selector and any authored DXGI transfer code.
    DxgiTransfer,
    /// The original `FourCC` and DX10 alpha-association word.
    AlphaAssociation,
    /// The original Cry marker word.
    CryImageMarker,
    /// The original Cry image flags word.
    CryImageFlags,
    /// The marker and, when recognised, the relevant Cry image flags.
    CryTransfer,
}

/// One authored DDS header word in the uncompressed source file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct DdsHeaderWord {
    /// Format-defined field name.
    pub name: &'static str,
    /// Byte offset from the beginning of the source file.
    pub offset: u64,
    /// Byte length of the word.
    pub length: u64,
}

#[derive(Clone, Copy)]
enum Word {
    Flags,
    Height,
    Width,
    Depth,
    MipMapCount,
    PixelFlags,
    FourCc,
    RgbBitCount,
    RBitMask,
    GBitMask,
    BBitMask,
    ABitMask,
    Caps2,
    DxgiFormat,
    ResourceDimension,
    MiscFlag,
    ArraySize,
    MiscFlags2,
    CryMarker,
    CryFlags,
}

impl Word {
    const fn location(self, magic: u64) -> DdsHeaderWord {
        // DDS_HEADER word offsets exclude the optional four-byte magic.
        let (name, body_offset) = match self {
            Self::Flags => ("flags", 4),
            Self::Height => ("height", 8),
            Self::Width => ("width", 12),
            Self::Depth => ("depth", 20),
            Self::MipMapCount => ("mipMapCount", 24),
            Self::PixelFlags => ("pixelFormat.flags", 76),
            Self::FourCc => ("pixelFormat.fourCc", 80),
            Self::RgbBitCount => ("pixelFormat.rgbBitCount", 84),
            Self::RBitMask => ("pixelFormat.rBitMask", 88),
            Self::GBitMask => ("pixelFormat.gBitMask", 92),
            Self::BBitMask => ("pixelFormat.bBitMask", 96),
            Self::ABitMask => ("pixelFormat.aBitMask", 100),
            Self::Caps2 => ("caps2", 108),
            Self::DxgiFormat => ("dx10.dxgiFormat", 124),
            Self::ResourceDimension => ("dx10.resourceDimension", 128),
            Self::MiscFlag => ("dx10.miscFlag", 132),
            Self::ArraySize => ("dx10.arraySize", 136),
            Self::MiscFlags2 => ("dx10.miscFlags2", 140),
            Self::CryMarker => ("reserved2", 120),
            Self::CryFlags => ("reserved1", 32),
        };
        DdsHeaderWord {
            name,
            offset: magic + body_offset,
            length: WORD_BYTES,
        }
    }
}

impl DdsRawHeader {
    /// Returns the authored byte words used to obtain one normalized header field.
    pub fn words_for(&self, field: DdsHeaderField) -> impl Iterator<Item = DdsHeaderWord> + use<> {
        use DdsHeaderField::{
            AlphaAssociation, ArraySize, CryImageFlags, CryImageMarker, CryTransfer, Depth,
            DxgiTransfer, Faces, Format, Height, MipCount, Prefix, Width,
        };
        let legacy = self.legacy();
        let dx10 = self.dx10();
        let (words, extra): (&[Word], Option<Word>) = match field {
            Prefix => (&[], None),
            Width => (&[Word::Width], None),
            Height => (&[Word::Height], None),
            MipCount => (&[Word::MipMapCount], None),
            Format | DxgiTransfer if dx10.is_some() => (&[Word::FourCc, Word::DxgiFormat], None),
            Format if legacy_uses_four_cc(legacy.pixel_format.flags) => {
                (&[Word::PixelFlags, Word::FourCc], None)
            }
            Format => (
                &[
                    Word::FourCc,
                    Word::PixelFlags,
                    Word::RgbBitCount,
                    Word::RBitMask,
                    Word::GBitMask,
                    Word::BBitMask,
                ],
                legacy_uses_alpha_mask(legacy.pixel_format.flags).then_some(Word::ABitMask),
            ),
            Faces if dx10.is_some() => (&[Word::FourCc, Word::MiscFlag], None),
            Faces => (&[Word::FourCc, Word::Caps2], None),
            ArraySize if dx10.is_some() => (&[Word::FourCc, Word::ArraySize], None),
            Depth => depth_words(legacy, dx10),
            AlphaAssociation if dx10.is_some() => (&[Word::FourCc, Word::MiscFlags2], None),
            AlphaAssociation => (&[Word::PixelFlags, Word::FourCc], None),
            ArraySize | DxgiTransfer => (&[Word::FourCc], None),
            CryImageFlags => (&[Word::CryFlags], None),
            CryTransfer if self.has_cry_image_flags() => (&[Word::CryMarker, Word::CryFlags], None),
            CryImageMarker | CryTransfer => (&[Word::CryMarker], None),
        };
        let magic = u64::from(self.magic_present()) * WORD_BYTES;
        let prefix = (field == Prefix).then_some(DdsHeaderWord {
            name: if self.magic_present() {
                "magic"
            } else {
                "size"
            },
            offset: 0,
            length: WORD_BYTES,
        });
        prefix.into_iter().chain(
            words
                .iter()
                .copied()
                .chain(extra)
                .map(move |word| word.location(magic)),
        )
    }
}

fn depth_words(
    legacy: &DdsLegacyFields,
    dx10: Option<&DdsDx10Fields>,
) -> (&'static [Word], Option<Word>) {
    dx10.map_or_else(
        || -> (&[Word], Option<Word>) {
            let words: &[Word] = if legacy_volume_uses_depth_flag(legacy.caps2) {
                &[Word::FourCc, Word::Flags, Word::Caps2]
            } else {
                &[Word::FourCc, Word::Caps2]
            };
            (
                words,
                legacy_is_volume(legacy.flags, legacy.caps2).then_some(Word::Depth),
            )
        },
        |fields| {
            (
                &[Word::FourCc, Word::ResourceDimension],
                dx10_is_volume(fields.resource_dimension).then_some(Word::Depth),
            )
        },
    )
}
