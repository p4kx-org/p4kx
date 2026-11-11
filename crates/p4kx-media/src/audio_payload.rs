// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::DecimalU64;
use schemars::JsonSchema;
use serde::Serialize;

use crate::{ByteOrder, SourceRange};

#[derive(Serialize, JsonSchema)]
#[serde(
    tag = "record_type",
    content = "payload",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
pub(crate) enum AudioPayload {
    Wem(WemHeader),
    WemChunk(WemChunk),
    WemOpaque(WemOpaque),
    WemFailure(WemFailure),
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct WemHeader {
    pub(crate) byte_order: ByteOrder,
    pub(crate) format_tag: u16,
    pub(crate) codec: &'static str,
    pub(crate) channels: u16,
    pub(crate) block_align: u16,
    pub(crate) bits_per_sample: u16,
    pub(crate) channel_layout: Option<u32>,
    pub(crate) channel_mask: u32,
    pub(crate) declared_sample_rate: u32,
    pub(crate) sample_rate: u32,
    /// Frames, with one sample per channel in each frame.
    pub(crate) sample_count: Option<DecimalU64>,
    pub(crate) duration: Option<WemDuration>,
    pub(crate) loop_points: Option<WemLoop>,
    /// First decoded byte after the declared RIFF or RIFX extent.
    pub(crate) riff_end_exclusive: u64,
    /// Bytes beyond the RIFF extent, available only through the retained raw Entry.
    pub(crate) trailing_length: DecimalU64,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct WemDuration {
    pub(crate) seconds: DecimalU64,
    #[schemars(range(max = 999_999_999))]
    pub(crate) nanoseconds: u32,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct WemLoop {
    /// First included frame.
    pub(crate) start: DecimalU64,
    /// First excluded frame.
    pub(crate) end: DecimalU64,
    pub(crate) frames: DecimalU64,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct WemChunk {
    pub(crate) ordinal: DecimalU64,
    /// Original `FourCC` bytes, including non-UTF-8 values.
    pub(crate) tag: [u8; 4],
    pub(crate) header_range: SourceRange,
    pub(crate) body_range: SourceRange,
    /// Pad bytes actually skipped by the RIFF traversal.
    pub(crate) padding_range: Option<SourceRange>,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct WemOpaque {
    pub(crate) class: WemOpaqueClass,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum WemOpaqueClass {
    PluginMedia,
    MidiMedia,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct WemFailure {
    pub(crate) phase: WemPhase,
    pub(crate) reason: WemFailureReason,
}

#[derive(Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum WemPhase {
    RiffHeader,
    Summary,
    ChunkDirectory,
}

#[derive(Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) enum WemFailureReason {
    InvalidSignature,
    Truncated {
        available: DecimalU64,
        needed: DecimalU64,
    },
    MissingChunk {
        name: &'static str,
    },
    InvalidFormat {
        detail: &'static str,
    },
    UnsupportedReaderVariant {
        detail: String,
    },
}
