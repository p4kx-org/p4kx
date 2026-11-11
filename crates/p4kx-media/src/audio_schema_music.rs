// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::Value;

use crate::audio_bank_music_segment::{self, MusicSegmentPayload};
use crate::audio_schema_status::StatusRule;
use crate::{MediaError, schema_merge};

use super::Binding;

const MUSIC_SEGMENT_PAYLOAD: &str = "MusicSegmentPayload";

pub(super) fn insert_payload(base: &mut Value) -> Result<(), MediaError> {
    schema_merge::insert_payload(
        base,
        MUSIC_SEGMENT_PAYLOAD,
        schema_merge::serialized_payload::<MusicSegmentPayload<'static>>(),
    )
}

pub(super) const fn bindings() -> [Binding; 4] {
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
    ]
}
