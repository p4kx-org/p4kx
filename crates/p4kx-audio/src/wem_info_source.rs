// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

/// The original scanner winner and decisions used for WEM summary values.
#[derive(Debug, Clone, Copy)]
pub struct WemInfoSource {
    pub(crate) format_body_offset: u64,
    pub(crate) data_body_offset: u64,
    pub(crate) codec: WemCodecSource,
    pub(crate) sample_count: WemSampleCountSource,
    pub(crate) loop_points: WemLoopSource,
}

impl WemInfoSource {
    /// Returns the byte offset of the last fmt body before the first data chunk.
    #[must_use]
    pub const fn format_body_offset(&self) -> u64 {
        self.format_body_offset
    }

    /// Returns the byte offset of the first data body.
    #[must_use]
    pub const fn data_body_offset(&self) -> u64 {
        self.data_body_offset
    }

    /// Returns the original branch that classified the codec.
    #[must_use]
    pub const fn codec(&self) -> WemCodecSource {
        self.codec
    }

    /// Returns the original branch that supplied or ruled out the frame count.
    #[must_use]
    pub const fn sample_count(&self) -> WemSampleCountSource {
        self.sample_count
    }

    /// Returns the sample-loop decision and the prefix it consulted.
    #[must_use]
    pub const fn loop_points(&self) -> WemLoopSource {
        self.loop_points
    }
}

/// The fields consulted to classify the original format tag.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub enum WemCodecSource {
    /// The format tag determined the codec.
    Tag,
    /// Channel count and fmt extension size distinguished DSP.
    DspCoefficients,
    /// Channel count, fmt extension size and block alignment distinguished ADPCM layouts.
    AdpcmBlock {
        /// Whether the optional fmt extension-size field was present.
        extra_length_present: bool,
    },
}

/// The actual source of the WEM summary frame count or its absence.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub enum WemSampleCountSource {
    /// The first data length and the PCM frame alignment.
    Pcm,
    /// The channel and block layout of an ADPCM WEM.
    Adpcm {
        /// Whether that layout allowed the first data length to be used.
        supported_layout: bool,
    },
    /// A readable count at the start of the selected vorb body.
    VorbisChunk {
        /// The zero-based body offset in the decoded WEM.
        body_offset: u64,
    },
    /// The optional fmt count and its fmt-length condition.
    VorbisFormat {
        /// The selected vorb body that was too short to hold the count, if present.
        rejected_vorbis_body: Option<u64>,
        /// Whether the fmt count passed the original length test.
        accepted: bool,
    },
    /// The optional embedded fmt count and its Opus variant condition.
    Opus {
        /// Whether the Switch rate guard ran.
        rate_checked: bool,
        /// Whether the embedded count passed the original variant test.
        accepted: bool,
    },
    /// The codec supplies no known frame count through this summary.
    Unsupported,
}

/// The original sample-loop decision.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub enum WemLoopSource {
    /// No smpl chunk was selected before data.
    NoSampleChunk,
    /// A selected smpl body was left unread without a known frame count.
    NoSampleCount,
    /// The selected body did not hold a complete loop-count field.
    MissingCount {
        /// The zero-based smpl body offset.
        body_offset: u64,
    },
    /// The loop count was readable and the entries region was missing.
    MissingEntries {
        /// The zero-based smpl body offset.
        body_offset: u64,
    },
    /// The original bounded table prefix consulted for the first fitting forward loop.
    Entries {
        /// The zero-based smpl body offset.
        body_offset: u64,
        /// The number of actually examined entries, including a selected last entry.
        examined: u64,
        /// The source of the selected loop end, when a loop was retained.
        end: Option<WemLoopEnd>,
    },
}

/// The original interpretation of a selected loop's last frame.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub enum WemLoopEnd {
    /// The stored inclusive last frame converts to an exclusive end.
    LastFrame,
    /// The stored zero sentinel selects the known sample count.
    SampleCount,
}
