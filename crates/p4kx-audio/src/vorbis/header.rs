// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::{HEADER_COMMENT, HEADER_IDENT, VorbisError};
use crate::loops::LOOP_COMMENTS;
use crate::{ByteOrder, Loop};

pub(super) const VORB_LEN: usize = 0x2A;
const EMBEDDED_VORB_OFFSET: usize = 0x18;
const EMBEDDED_FMT_LEN: usize = EMBEDDED_VORB_OFFSET + VORB_LEN;
pub(super) const AVERAGE_BYTES_OFFSET: usize = 8;
const MOD_SIGNAL_OFFSET: usize = 0x04;
const SETUP_OFFSET: usize = 0x10;
const AUDIO_OFFSET: usize = 0x14;
const BLOCK_SIZES_OFFSET: usize = 0x28;
const UNMODIFIED_PACKET_SIGNALS: [u32; 4] = [0x4A, 0x4B, 0x69, 0x70];
const BLOCK_SIZE_BITS: std::ops::RangeInclusive<u8> = 6..=13;
const VENDOR: &[u8] = b"P4KX, rebuilt from Wwise Vorbis";

/// The fields of a Wwise Vorbis header that rebuilding the stream needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Header {
    pub(crate) channels: u8,
    pub(crate) sample_rate: u32,
    pub(crate) bitrate: u32,
    pub(crate) sample_count: u32,
    pub(crate) modified_packets: bool,
    pub(crate) setup_offset: usize,
    pub(crate) audio_offset: usize,
    pub(crate) short_bits: u8,
    pub(crate) long_bits: u8,
}

impl Header {
    /// Reads the header from the `vorb` chunk, or from the extended `fmt` chunk without one.
    pub(crate) fn read(
        order: ByteOrder,
        fmt: &[u8],
        vorb: Option<&[u8]>,
        channels: u16,
        sample_rate: u32,
    ) -> Result<Self, VorbisError> {
        let offset = configuration_offset(fmt.len(), vorb.map(<[u8]>::len))?;
        let configuration = vorb.unwrap_or(fmt).get(offset..).unwrap_or_default();
        let average_bytes = order.u32_at(fmt, AVERAGE_BYTES_OFFSET).unwrap_or_default();
        Self::parse(order, average_bytes, configuration, channels, sample_rate)
    }

    pub(super) fn parse(
        order: ByteOrder,
        average_bytes: u32,
        vorb: &[u8],
        channels: u16,
        sample_rate: u32,
    ) -> Result<Self, VorbisError> {
        let field = |offset| order.u32_at(vorb, offset).unwrap_or_default();
        let offset = |offset| usize::try_from(field(offset)).unwrap_or(usize::MAX);
        let short_bits = vorb.get(BLOCK_SIZES_OFFSET).copied().unwrap_or_default();
        let long_bits = vorb
            .get(BLOCK_SIZES_OFFSET + 1)
            .copied()
            .unwrap_or_default();
        if !BLOCK_SIZE_BITS.contains(&short_bits)
            || !BLOCK_SIZE_BITS.contains(&long_bits)
            || short_bits > long_bits
        {
            return Err(VorbisError::BlockSizes {
                short: short_bits,
                long: long_bits,
            });
        }
        Ok(Self {
            channels: u8::try_from(channels).map_err(|_| VorbisError::TooManyChannels(channels))?,
            sample_rate,
            bitrate: average_bytes.saturating_mul(8),
            sample_count: field(0),
            modified_packets: !UNMODIFIED_PACKET_SIGNALS.contains(&field(MOD_SIGNAL_OFFSET)),
            setup_offset: offset(SETUP_OFFSET),
            audio_offset: offset(AUDIO_OFFSET),
            short_bits,
            long_bits,
        })
    }

    /// Returns the samples of a short or a long block.
    pub(crate) const fn block_size(&self, long: bool) -> u64 {
        1 << if long {
            self.long_bits
        } else {
            self.short_bits
        }
    }

    /// Writes the Vorbis identification header.
    pub(crate) fn ident_packet(&self) -> [u8; 30] {
        let mut packet = [0; 30];
        packet[0] = HEADER_IDENT;
        packet[1..7].copy_from_slice(b"vorbis");
        packet[11] = self.channels;
        packet[12..16].copy_from_slice(&self.sample_rate.to_le_bytes());
        packet[20..24].copy_from_slice(&self.bitrate.to_le_bytes());
        packet[28] = self.short_bits | self.long_bits << 4;
        packet[29] = 1;
        packet
    }
}

pub(super) const fn configuration_offset(
    fmt_len: usize,
    vorb_len: Option<usize>,
) -> Result<usize, VorbisError> {
    match vorb_len {
        Some(VORB_LEN) => Ok(0),
        Some(len) => Err(VorbisError::UnsupportedHeader { len }),
        None if fmt_len == EMBEDDED_FMT_LEN => Ok(EMBEDDED_VORB_OFFSET),
        None => Err(VorbisError::UnsupportedHeader { len: fmt_len }),
    }
}

/// Writes a Vorbis comment header naming the tool and holding the comments of `points`.
pub(crate) fn comment_packet(points: Option<Loop>, out: &mut Vec<u8>) {
    out.clear();
    out.push(HEADER_COMMENT);
    out.extend_from_slice(b"vorbis");
    out.extend_from_slice(
        &u32::try_from(VENDOR.len())
            .unwrap_or_default()
            .to_le_bytes(),
    );
    out.extend_from_slice(VENDOR);
    match points {
        Some(points) => {
            out.extend_from_slice(&LOOP_COMMENTS.to_le_bytes());
            points.write_comments(out);
        }
        None => out.extend_from_slice(&0_u32.to_le_bytes()),
    }
    out.push(1);
}
