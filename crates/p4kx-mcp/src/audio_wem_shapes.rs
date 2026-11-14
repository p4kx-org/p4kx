// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::audio_shapes::LocationShape;

/// What the header of a WEM says.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct WemShape {
    /// The WEM ID; absent for a loose WEM Entry whose file name is no ID.
    pub(crate) id: Option<u32>,
    /// The size in bytes.
    pub(crate) size: u64,
    /// Whether it is only the prefetched start of a streamed WEM.
    pub(crate) prefetch: bool,
    /// A stable codec code such as `vorbis`; absent for a prefetched start or an unreadable WEM.
    pub(crate) codec: Option<&'static str>,
    /// The codec name.
    pub(crate) codec_name: Option<String>,
    /// The number of channels.
    pub(crate) channels: Option<u16>,
    /// The sample rate in Hz.
    pub(crate) sample_rate: Option<u32>,
    /// The frames per channel, where the header or the codec states it.
    pub(crate) sample_count: Option<u64>,
    /// The playing time in seconds.
    pub(crate) duration_seconds: Option<f64>,
    /// Whether the sound loops.
    pub(crate) looping: Option<bool>,
    /// Whether the WEM is plug-in media, which has no codec.
    pub(crate) plugin_media: bool,
    /// Why the WEM cannot be read, other than plug-in media.
    pub(crate) error: Option<String>,
}

/// The WEMs a soundbank embeds, one page.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct SoundbankWemsResult {
    /// The path of the Archive.
    pub(crate) archive: String,
    /// The Entry path of the soundbank.
    pub(crate) soundbank: String,
    /// The bank version.
    pub(crate) version: u32,
    /// The soundbank ID.
    pub(crate) bank_id: u32,
    /// How many WEMs the soundbank embeds.
    pub(crate) total: usize,
    /// The WEMs on this page in ascending ID order.
    pub(crate) wems: Vec<WemShape>,
    /// The cursor of the next page; absent on the last page.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) next_cursor: Option<String>,
}

/// A WEM with where the Archive holds it.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct WemInfoResult {
    /// The path of the Archive.
    pub(crate) archive: String,
    /// The WEM ID; absent for a loose WEM Entry whose file name is no ID.
    pub(crate) wem_id: Option<u32>,
    /// How many places hold the WEM.
    pub(crate) locations_total: usize,
    /// The places on this page that hold it, loose Entries first.
    pub(crate) locations: Vec<LocationShape>,
    /// The cursor of the next page of `locations`; absent on the last page.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) next_cursor: Option<String>,
    /// The place the description was read from: the first that holds the whole WEM.
    pub(crate) described: LocationShape,
    /// The WEM as read from `described`.
    pub(crate) wem: WemShape,
}

/// An audio excerpt; the WAV itself is the audio content block of the result.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct ExcerptResult {
    /// The path of the Archive.
    pub(crate) archive: String,
    /// The WEM ID; absent for a loose WEM Entry whose file name is no ID.
    pub(crate) wem_id: Option<u32>,
    /// The place the sound was read from.
    pub(crate) source: LocationShape,
    /// The codec of the WEM.
    pub(crate) codec: &'static str,
    /// The MIME type of the audio content block: always `audio/wav`.
    pub(crate) mime_type: &'static str,
    /// The channels of the WAV.
    pub(crate) channels: u16,
    /// The sample rate of the WAV in Hz.
    pub(crate) sample_rate: u32,
    /// The playing time of the whole sound in seconds.
    pub(crate) sound_seconds: f64,
    /// Where the excerpt starts, in seconds.
    pub(crate) start_seconds: f64,
    /// The length of the excerpt in seconds.
    pub(crate) duration_seconds: f64,
    /// The frames of the excerpt.
    pub(crate) frames: u64,
    /// The size of the WAV in bytes, at most `max_bytes`.
    pub(crate) bytes: usize,
    /// The size cap of the WAV in bytes.
    pub(crate) max_bytes: usize,
    /// Whether the excerpt is shorter than asked for: cut by the size cap or the sound's end.
    pub(crate) truncated: bool,
    /// Origin of each structured field and of the WAV content block.
    #[serde(rename = "fieldOrigins")]
    pub(crate) field_origins: crate::audio_excerpt_origin::ExcerptFieldOrigins,
    /// Original source and contribution coordinates used by this response.
    pub(crate) provenance: p4kx_extraction::SourceInventory,
}
