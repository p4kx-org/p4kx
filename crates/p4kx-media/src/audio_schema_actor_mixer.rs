// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::Value;

use crate::audio_bank_actor_mixer::{CHILD_NAMESPACE, NAMESPACE};
use crate::audio_bank_actor_mixer_payload::ActorMixerPayload;
use crate::{MediaError, schema_merge};

use super::{Binding, StatusRule};

const ACTOR_MIXER_PAYLOAD: &str = "ActorMixerPayload";

pub(super) fn insert_payload(base: &mut Value) -> Result<(), MediaError> {
    schema_merge::insert_payload(
        base,
        ACTOR_MIXER_PAYLOAD,
        schema_merge::serialized_payload::<ActorMixerPayload<'static>>(),
    )
}

pub(super) const fn bindings() -> [Binding; 2] {
    [
        Binding::exact(
            ACTOR_MIXER_PAYLOAD,
            NAMESPACE,
            "bank-hirc-actor-mixer",
            2,
            StatusRule::MalformedLayout,
        ),
        Binding::exact(
            ACTOR_MIXER_PAYLOAD,
            CHILD_NAMESPACE,
            "bank-hirc-actor-mixer-child",
            3,
            StatusRule::Complete,
        ),
    ]
}
