// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::Value;

use crate::audio_bank_music_segment::{self, MusicSegmentPayload};
use crate::audio_bank_music_track::{self, MusicTrackPayload};
use crate::audio_schema_status::StatusRule;
use crate::{MediaError, schema_merge};

use super::Binding;

const MUSIC_SEGMENT_PAYLOAD: &str = "MusicSegmentPayload";
const MUSIC_TRACK_PAYLOAD: &str = "MusicTrackPayload";

pub(super) fn insert_payload(base: &mut Value) -> Result<(), MediaError> {
    schema_merge::insert_payload(
        base,
        MUSIC_SEGMENT_PAYLOAD,
        schema_merge::serialized_payload::<MusicSegmentPayload<'static>>(),
    )?;
    schema_merge::insert_payload(
        base,
        MUSIC_TRACK_PAYLOAD,
        schema_merge::serialized_payload::<MusicTrackPayload>(),
    )
}

pub(super) const fn bindings() -> [Binding; 8] {
    [
        Binding::exact(
            MUSIC_SEGMENT_PAYLOAD,
            audio_bank_music_segment::NAMESPACE,
            "bank-music-segment",
            2,
            StatusRule::MalformedLayout,
        ),
        Binding::exact(
            MUSIC_SEGMENT_PAYLOAD,
            audio_bank_music_segment::CHILD,
            "bank-music-segment-child",
            3,
            StatusRule::Complete,
        ),
        Binding::exact(
            MUSIC_SEGMENT_PAYLOAD,
            audio_bank_music_segment::STINGER,
            "bank-music-segment-stinger",
            3,
            StatusRule::Complete,
        ),
        Binding::exact(
            MUSIC_SEGMENT_PAYLOAD,
            audio_bank_music_segment::MARKER,
            "bank-music-segment-marker",
            3,
            StatusRule::Complete,
        ),
        Binding::exact(
            MUSIC_TRACK_PAYLOAD,
            audio_bank_music_track::NAMESPACE,
            "bank-music-track",
            2,
            StatusRule::MalformedLayout,
        ),
        Binding::exact(
            MUSIC_TRACK_PAYLOAD,
            audio_bank_music_track::SOURCE,
            "bank-music-track-source",
            3,
            StatusRule::Complete,
        ),
        Binding::exact(
            MUSIC_TRACK_PAYLOAD,
            audio_bank_music_track::CLIP,
            "bank-music-track-clip",
            3,
            StatusRule::Complete,
        ),
        Binding::exact(
            MUSIC_TRACK_PAYLOAD,
            audio_bank_music_track::SWITCH_MEMBER,
            "bank-music-track-switch-member",
            3,
            StatusRule::Complete,
        ),
    ]
}
