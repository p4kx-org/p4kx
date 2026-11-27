// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::Value;

use crate::audio_bank_inline_state as inline;
use crate::audio_bank_state as state;
use crate::audio_bank_state_payload::StatePayload;
use crate::{MediaError, schema_merge};

use super::{Binding, StatusRule};

const STATE_PAYLOAD: &str = "StatePayload";

pub(super) fn insert_payload(base: &mut Value) -> Result<(), MediaError> {
    schema_merge::insert_payload(
        base,
        STATE_PAYLOAD,
        schema_merge::serialized_payload::<StatePayload>(),
    )
}

const fn complete(namespace: &'static str, record_type: &'static str, indices: u8) -> Binding {
    Binding::exact(
        STATE_PAYLOAD,
        namespace,
        record_type,
        indices,
        StatusRule::Complete,
    )
}

pub(super) const fn bindings() -> [Binding; 9] {
    [
        Binding::exact(
            STATE_PAYLOAD,
            state::STATE_NAMESPACE,
            "bank-hirc-state",
            2,
            StatusRule::MalformedLayout,
        ),
        complete(state::PROPERTY_NAMESPACE, "bank-hirc-state-property", 3),
        complete(inline::INLINE_NAMESPACE, "bank-hirc-inline-state", 2),
        complete(
            inline::BINDING_COUNT_NAMESPACE,
            "bank-hirc-inline-state-binding-count",
            2,
        ),
        complete(
            inline::BINDING_NAMESPACE,
            "bank-hirc-inline-state-binding",
            3,
        ),
        complete(
            inline::GROUP_COUNT_NAMESPACE,
            "bank-hirc-inline-state-group-count",
            2,
        ),
        complete(inline::GROUP_NAMESPACE, "bank-hirc-inline-state-group", 3),
        complete(inline::ENTRY_NAMESPACE, "bank-hirc-inline-state-entry", 4),
        complete(
            state::INLINE_PROPERTY_NAMESPACE,
            "bank-hirc-inline-state-property",
            5,
        ),
    ]
}
