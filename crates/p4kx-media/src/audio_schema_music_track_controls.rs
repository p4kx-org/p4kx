// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::Value;

use crate::audio_bank_music_track_control_payload::{
    AUTOMATION, CONTROLS, POINT, TrackControlPayload,
};
use crate::{MediaError, schema_merge};

use super::{Binding, StatusRule};

const PAYLOAD: &str = "TrackControlPayload";

pub(super) fn insert_payload(base: &mut Value) -> Result<(), MediaError> {
    schema_merge::insert_payload(
        base,
        PAYLOAD,
        schema_merge::serialized_payload::<TrackControlPayload<'static>>(),
    )
}

pub(super) const fn bindings() -> [Binding; 3] {
    [
        Binding::exact(
            PAYLOAD,
            CONTROLS,
            "bank-music-track-controls",
            2,
            StatusRule::MalformedLayout,
        ),
        Binding::exact(
            PAYLOAD,
            AUTOMATION,
            "bank-music-track-automation",
            3,
            StatusRule::MalformedLayout,
        ),
        Binding::exact(
            PAYLOAD,
            POINT,
            "bank-music-track-automation-point",
            4,
            StatusRule::MalformedLayout,
        ),
    ]
}
