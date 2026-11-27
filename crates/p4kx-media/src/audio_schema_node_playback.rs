// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::Value;

use crate::audio_bank_node_advanced as advanced;
use crate::audio_bank_node_playback_payload::NodePlaybackPayload;
use crate::audio_bank_node_positioning as positioning;
use crate::audio_bank_positioning_rows as path;
use crate::{MediaError, schema_merge};

use super::{Binding, StatusRule};

const PAYLOAD: &str = "NodePlaybackPayload";

pub(super) fn insert_payload(base: &mut Value) -> Result<(), MediaError> {
    schema_merge::insert_payload(
        base,
        PAYLOAD,
        schema_merge::serialized_payload::<NodePlaybackPayload<'static>>(),
    )
}

pub(super) const fn bindings() -> [Binding; 3] {
    [
        Binding::exact(
            PAYLOAD,
            positioning::NAMESPACE,
            "bank-hirc-node-positioning",
            2,
            StatusRule::MalformedLayout,
        ),
        Binding::exact(
            PAYLOAD,
            path::NAMESPACE,
            "bank-hirc-node-positioning-path",
            4,
            StatusRule::MalformedLayout,
        ),
        Binding::exact(
            PAYLOAD,
            advanced::NAMESPACE,
            "bank-hirc-node-advanced-settings",
            2,
            StatusRule::MalformedLayout,
        ),
    ]
}
