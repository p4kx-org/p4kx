// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::audio_bank_hirc_action;
use crate::audio_bank_hirc_container;
use crate::audio_bank_hirc_sound;
use crate::audio_bank_hirc_sound::loose as sound_loose;
use crate::audio_bank_sound_reference;
use crate::audio_schema_status::StatusRule;

use super::{ACTION_PAYLOAD, Binding, CONTAINER_PAYLOAD, SOUND_PAYLOAD};

pub(super) const fn bindings() -> [Binding; 8] {
    [
        Binding::exact(
            ACTION_PAYLOAD,
            audio_bank_hirc_action::NAMESPACE,
            "bank-hirc-action",
            2,
            StatusRule::MalformedLayout,
        ),
        Binding::exact(
            SOUND_PAYLOAD,
            audio_bank_hirc_sound::NAMESPACE,
            "bank-hirc-sound",
            2,
            StatusRule::MalformedLayout,
        ),
        Binding::exact(
            SOUND_PAYLOAD,
            audio_bank_sound_reference::RESOLUTION_NAMESPACE,
            "bank-hirc-sound-media-resolution",
            2,
            StatusRule::Complete,
        ),
        Binding::exact(
            SOUND_PAYLOAD,
            audio_bank_sound_reference::CANDIDATE_NAMESPACE,
            "bank-hirc-sound-media-candidate",
            3,
            StatusRule::Complete,
        ),
        Binding::exact(
            SOUND_PAYLOAD,
            sound_loose::RESOLUTION_NAMESPACE,
            "bank-hirc-sound-loose-wem-resolution",
            2,
            StatusRule::Complete,
        ),
        Binding::exact(
            SOUND_PAYLOAD,
            sound_loose::CANDIDATE_NAMESPACE,
            "bank-hirc-sound-loose-wem-candidate",
            3,
            StatusRule::Complete,
        ),
        Binding::exact(
            CONTAINER_PAYLOAD,
            audio_bank_hirc_container::NAMESPACE,
            "bank-hirc-container",
            2,
            StatusRule::MalformedLayout,
        ),
        Binding::exact(
            CONTAINER_PAYLOAD,
            audio_bank_hirc_container::CHILD_NAMESPACE,
            "bank-hirc-container-child",
            3,
            StatusRule::Complete,
        ),
    ]
}
