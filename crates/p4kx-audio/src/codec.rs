// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt;

use crate::WemCodecSource;

const DSP_HEADER_LEN: u32 = 0x0C;
const DSP_CHANNEL_LEN: u32 = 0x2E;
const PTADPCM_FRAME_LEN: u32 = 0x104;

/// The codec of a WEM, detected from the format tag of its `fmt` chunk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Codec {
    /// Uncompressed 16-bit PCM, tags `0x0001` and `0xFFFE`.
    Pcm,
    /// IMA ADPCM, tags `0x0002`, `0x0011` and `0x0069`; a `0x0002` `fmt` chunk of the layout of
    /// [`Codec::Dsp`] or [`Codec::PtAdpcm`] is one of those instead.
    ImaAdpcm,
    /// Wwise Vorbis with stripped headers, tag `0xFFFF`.
    Vorbis,
    /// Opus, tags `0x3039`, `0x3040` and `0x3041`.
    Opus,
    /// Platinum ADPCM, tag `0x8311`, or tag `0x0002` with a block alignment of 260 bytes per
    /// channel.
    PtAdpcm,
    /// Xbox WMA, tags `0x0161` and `0x0162`.
    Xwma,
    /// Xbox XMA2, tags `0x0165` and `0x0166`.
    Xma2,
    /// AAC, tag `0xAAC0`.
    Aac,
    /// Nintendo DSP ADPCM, tag `0xFFF0`, or tag `0x0002` with `12 + 46 � channels` bytes after
    /// the `fmt` fields.
    Dsp,
    /// Sony HEVAG, tag `0xFFFB`.
    Hevag,
    /// Sony ATRAC9, tag `0xFFFC`.
    Atrac9,
    /// A format tag Wwise is not known to write.
    Unknown(u16),
}

impl Codec {
    /// Returns the codec a Wwise `fmt` chunk's format tag stands for.
    ///
    /// The tag `0x0002` alone is taken for IMA ADPCM; [`Codec::of_format`] also reads the rest of
    /// the `fmt` chunk and tells DSP and PTADPCM apart.
    #[must_use]
    pub const fn from_tag(tag: u16) -> Self {
        match tag {
            0x0001 | 0xFFFE => Self::Pcm,
            0x0002 | 0x0011 | 0x0069 => Self::ImaAdpcm,
            0xFFFF => Self::Vorbis,
            0x3039 | 0x3040 | 0x3041 => Self::Opus,
            0x8311 => Self::PtAdpcm,
            0x0161 | 0x0162 => Self::Xwma,
            0x0165 | 0x0166 => Self::Xma2,
            0xAAC0 => Self::Aac,
            0xFFF0 => Self::Dsp,
            0xFFFB => Self::Hevag,
            0xFFFC => Self::Atrac9,
            other => Self::Unknown(other),
        }
    }

    /// Returns the codec of a `fmt` chunk with the format tag `tag`, `channels` channels, a block
    /// alignment of `block_align` bytes and `extra_len` bytes after the common fields, if the chunk
    /// states them.
    ///
    /// Tag `0x0002` is IMA ADPCM unless the chunk holds DSP coefficients (`extra_len` is
    /// `12 + 46 � channels`) or its block alignment is 260 bytes per channel (PTADPCM).
    #[must_use]
    pub const fn of_format(
        tag: u16,
        channels: u16,
        block_align: u16,
        extra_len: Option<u16>,
    ) -> Self {
        Self::classify_format(tag, channels, block_align, extra_len).0
    }

    pub(crate) const fn classify_format(
        tag: u16,
        channels: u16,
        block_align: u16,
        extra_len: Option<u16>,
    ) -> (Self, WemCodecSource) {
        if tag != 0x0002 {
            return (Self::from_tag(tag), WemCodecSource::Tag);
        }
        let channels = channels as u32;
        let adpcm = WemCodecSource::AdpcmBlock {
            extra_length_present: extra_len.is_some(),
        };
        match extra_len {
            Some(extra) if extra as u32 == DSP_HEADER_LEN + channels * DSP_CHANNEL_LEN => {
                (Self::Dsp, WemCodecSource::DspCoefficients)
            }
            _ if block_align as u32 == PTADPCM_FRAME_LEN * channels => (Self::PtAdpcm, adpcm),
            _ => (Self::ImaAdpcm, adpcm),
        }
    }

    /// Returns a stable lower-case identifier for machine-readable output, e.g. `vorbis`.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Pcm => "pcm",
            Self::ImaAdpcm => "ima_adpcm",
            Self::Vorbis => "vorbis",
            Self::Opus => "opus",
            Self::PtAdpcm => "ptadpcm",
            Self::Xwma => "xwma",
            Self::Xma2 => "xma2",
            Self::Aac => "aac",
            Self::Dsp => "dsp",
            Self::Hevag => "hevag",
            Self::Atrac9 => "atrac9",
            Self::Unknown(_) => "unknown",
        }
    }
}

impl fmt::Display for Codec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Pcm => "PCM",
            Self::ImaAdpcm => "IMA ADPCM",
            Self::Vorbis => "Wwise Vorbis",
            Self::Opus => "Opus",
            Self::PtAdpcm => "PTADPCM",
            Self::Xwma => "XWMA",
            Self::Xma2 => "XMA2",
            Self::Aac => "AAC",
            Self::Dsp => "DSP ADPCM",
            Self::Hevag => "HEVAG",
            Self::Atrac9 => "ATRAC9",
            Self::Unknown(tag) => return write!(f, "unknown codec {tag:#06x}"),
        };
        f.write_str(name)
    }
}
