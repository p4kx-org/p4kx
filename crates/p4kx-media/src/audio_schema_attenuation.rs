// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::Value;

use crate::audio_bank_attenuation::payload::{AttenuationPayload, CURVE, POINT, SELECTOR, SUMMARY};
use crate::{MediaError, schema_merge};

use super::{Binding, StatusRule};

const PAYLOAD: &str = "AttenuationPayload";

pub(super) fn insert_payload(base: &mut Value) -> Result<(), MediaError> {
    schema_merge::insert_payload(
        base,
        PAYLOAD,
        schema_merge::serialized_payload::<AttenuationPayload<'static>>(),
    )
}

pub(super) const fn bindings() -> [Binding; 4] {
    [
        Binding::exact(
            PAYLOAD,
            SUMMARY,
            "bank-hirc-attenuation",
            2,
            StatusRule::MalformedLayout,
        ),
        Binding::exact(
            PAYLOAD,
            SELECTOR,
            "bank-hirc-attenuation-selector",
            3,
            StatusRule::Complete,
        ),
        Binding::exact(
            PAYLOAD,
            CURVE,
            "bank-hirc-attenuation-curve",
            3,
            StatusRule::Complete,
        ),
        Binding::exact(
            PAYLOAD,
            POINT,
            "bank-hirc-attenuation-point",
            4,
            StatusRule::Complete,
        ),
    ]
}
