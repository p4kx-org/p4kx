// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::Value;

use crate::audio_bank_rtpc::RTPC_NAMESPACE;
use crate::audio_bank_rtpc_payload::RtpcPayload;
use crate::audio_bank_rtpc_rows::{CURVE_NAMESPACE, POINT_NAMESPACE};
use crate::{MediaError, schema_merge};

use super::{Binding, StatusRule};

const RTPC_PAYLOAD: &str = "RtpcPayload";

pub(super) fn insert_payload(base: &mut Value) -> Result<(), MediaError> {
    schema_merge::insert_payload(
        base,
        RTPC_PAYLOAD,
        schema_merge::serialized_payload::<RtpcPayload<'static>>(),
    )
}

pub(super) const fn bindings() -> [Binding; 3] {
    [
        Binding::exact(
            RTPC_PAYLOAD,
            RTPC_NAMESPACE,
            "bank-hirc-sound-rtpc",
            2,
            StatusRule::MalformedLayout,
        ),
        Binding::exact(
            RTPC_PAYLOAD,
            CURVE_NAMESPACE,
            "bank-hirc-sound-rtpc-curve",
            3,
            StatusRule::Complete,
        ),
        Binding::exact(
            RTPC_PAYLOAD,
            POINT_NAMESPACE,
            "bank-hirc-sound-rtpc-point",
            4,
            StatusRule::Complete,
        ),
    ]
}
