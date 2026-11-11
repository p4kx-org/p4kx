// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::Value;

use crate::audio_bank_node_properties::{PROPERTY_NAMESPACE, SUMMARY_NAMESPACE};
use crate::audio_bank_node_property_payload::NodePropertyPayload;
use crate::{MediaError, schema_merge};

use super::{Binding, StatusRule};

const NODE_PROPERTY_PAYLOAD: &str = "NodePropertyPayload";

pub(super) fn insert_payload(base: &mut Value) -> Result<(), MediaError> {
    schema_merge::insert_payload(
        base,
        NODE_PROPERTY_PAYLOAD,
        schema_merge::serialized_payload::<NodePropertyPayload<'static>>(),
    )
}

pub(super) const fn bindings() -> [Binding; 2] {
    [
        Binding::exact(
            NODE_PROPERTY_PAYLOAD,
            SUMMARY_NAMESPACE,
            "bank-hirc-node-properties",
            2,
            StatusRule::MalformedLayout,
        ),
        Binding::exact(
            NODE_PROPERTY_PAYLOAD,
            PROPERTY_NAMESPACE,
            "bank-hirc-node-property",
            4,
            StatusRule::Complete,
        ),
    ]
}
