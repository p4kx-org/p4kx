// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::Value;

use crate::audio_bank_blend_payload::{
    ASSOCIATION_NAMESPACE, BlendPayload, LAYER_NAMESPACE, POINT_NAMESPACE, SUMMARY_NAMESPACE,
};
use crate::{MediaError, schema_merge};

use super::{Binding, StatusRule};

const PAYLOAD: &str = "BlendPayload";

pub(super) fn insert_payload(base: &mut Value) -> Result<(), MediaError> {
    schema_merge::insert_payload(
        base,
        PAYLOAD,
        schema_merge::serialized_payload::<BlendPayload<'static>>(),
    )
}

pub(super) const fn bindings() -> [Binding; 4] {
    [
        Binding::exact(
            PAYLOAD,
            SUMMARY_NAMESPACE,
            "bank-hirc-blend-container",
            2,
            StatusRule::MalformedLayout,
        ),
        Binding::exact(
            PAYLOAD,
            LAYER_NAMESPACE,
            "bank-hirc-blend-layer",
            3,
            StatusRule::MalformedLayout,
        ),
        Binding::exact(
            PAYLOAD,
            ASSOCIATION_NAMESPACE,
            "bank-hirc-blend-association",
            4,
            StatusRule::MalformedLayout,
        ),
        Binding::exact(
            PAYLOAD,
            POINT_NAMESPACE,
            "bank-hirc-blend-point",
            5,
            StatusRule::MalformedLayout,
        ),
    ]
}
