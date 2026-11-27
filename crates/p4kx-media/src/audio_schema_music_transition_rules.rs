// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::Value;

use crate::audio_bank_music_transition_rule_payload::{
    ID, MusicTransitionRulePayload, RULE, SUMMARY,
};
use crate::{MediaError, schema_merge};

use super::{Binding, StatusRule};

const PAYLOAD: &str = "MusicTransitionRulePayload";

pub(super) fn insert_payload(base: &mut Value) -> Result<(), MediaError> {
    schema_merge::insert_payload(
        base,
        PAYLOAD,
        schema_merge::serialized_payload::<MusicTransitionRulePayload<'static>>(),
    )
}

pub(super) const fn bindings() -> [Binding; 3] {
    [
        Binding::exact(
            PAYLOAD,
            SUMMARY,
            "bank-music-transition-rules",
            2,
            StatusRule::MalformedLayout,
        ),
        Binding::exact(
            PAYLOAD,
            RULE,
            "bank-music-transition-rule",
            3,
            StatusRule::MalformedLayout,
        ),
        Binding::exact(
            PAYLOAD,
            ID,
            "bank-music-transition-id",
            5,
            StatusRule::Complete,
        ),
    ]
}
