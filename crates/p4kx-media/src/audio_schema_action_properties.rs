// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::Value;

use crate::audio_bank_action_property_payload::{
    ActionPropertyPayload, PROPERTY_NAMESPACE, SUMMARY_NAMESPACE,
};
use crate::{MediaError, schema_merge};

use super::{Binding, StatusRule};

const PAYLOAD: &str = "ActionPropertyPayload";

pub(super) fn insert_payload(base: &mut Value) -> Result<(), MediaError> {
    schema_merge::insert_payload(
        base,
        PAYLOAD,
        schema_merge::serialized_payload::<ActionPropertyPayload<'static>>(),
    )
}

pub(super) const fn bindings() -> [Binding; 2] {
    [
        Binding::exact(
            PAYLOAD,
            SUMMARY_NAMESPACE,
            "bank-hirc-action-properties",
            2,
            StatusRule::MalformedLayout,
        ),
        Binding::exact(
            PAYLOAD,
            PROPERTY_NAMESPACE,
            "bank-hirc-action-property",
            4,
            StatusRule::Complete,
        ),
    ]
}
