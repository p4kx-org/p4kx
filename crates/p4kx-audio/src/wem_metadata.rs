// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::Duration;

use crate::wem::playing_time;
use crate::{ByteOrder, Codec, Loop, WemInfoSource, WemSourceChunk};

/// Decoded WEM summary and the original source occurrences it used.
#[derive(Debug, Clone)]
pub struct WemMetadata {
    pub(crate) order: ByteOrder,
    pub(crate) format_tag: u16,
    pub(crate) codec: Codec,
    pub(crate) channels: u16,
    pub(crate) declared_sample_rate: u32,
    pub(crate) sample_rate: u32,
    pub(crate) block_align: u16,
    pub(crate) bits_per_sample: u16,
    pub(crate) channel_layout: Option<u32>,
    pub(crate) channel_mask: u32,
    pub(crate) sample_count: Option<u64>,
    pub(crate) loop_points: Option<Loop>,
    pub(crate) data: WemSourceChunk,
    pub(crate) info_source: WemInfoSource,
}

impl WemMetadata {
    /// Returns the original source decisions for the decoded summary.
    #[must_use]
    pub const fn info_source(&self) -> &WemInfoSource {
        &self.info_source
    }

    /// Returns the original first data chunk.
    #[must_use]
    pub const fn data_chunk(&self) -> WemSourceChunk {
        self.data
    }

    /// Returns the codec selected by the format declaration.
    #[must_use]
    pub const fn codec(&self) -> Codec {
        self.codec
    }

    /// Returns the raw format tag.
    #[must_use]
    pub const fn format_tag(&self) -> u16 {
        self.format_tag
    }

    /// Returns the byte order declared by the RIFF signature.
    #[must_use]
    pub const fn byte_order(&self) -> ByteOrder {
        self.order
    }

    /// Returns the number of channels.
    #[must_use]
    pub const fn channels(&self) -> u16 {
        self.channels
    }

    /// Returns the original format sample rate in Hz.
    #[must_use]
    pub const fn declared_sample_rate(&self) -> u32 {
        self.declared_sample_rate
    }

    /// Returns the playback sample rate in Hz, including the Opus rate convention.
    #[must_use]
    pub const fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Returns the declared bytes per block or frame.
    #[must_use]
    pub const fn block_align(&self) -> u16 {
        self.block_align
    }

    /// Returns the original format bits-per-sample word.
    #[must_use]
    pub const fn bits_per_sample(&self) -> u16 {
        self.bits_per_sample
    }

    /// Returns the original channel-layout word when it exists in the selected format.
    #[must_use]
    pub const fn channel_layout(&self) -> Option<u32> {
        self.channel_layout
    }

    /// Returns the interpreted speaker mask, or zero when it is unknown.
    #[must_use]
    pub const fn channel_mask(&self) -> u32 {
        self.channel_mask
    }

    /// Returns the number of frames when the selected format supplies it.
    #[must_use]
    pub const fn sample_count(&self) -> Option<u64> {
        self.sample_count
    }

    /// Returns the first fitting forward loop, in frames with an exclusive end.
    #[must_use]
    pub const fn loop_points(&self) -> Option<Loop> {
        self.loop_points
    }

    /// Returns the playing time when the frame count is known.
    #[must_use]
    pub fn duration(&self) -> Option<Duration> {
        playing_time(self.sample_count?, self.sample_rate)
    }
}
