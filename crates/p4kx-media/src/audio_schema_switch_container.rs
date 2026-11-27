// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::Value;

use crate::audio_bank_switch_container_payload::{
    CONFIG_NAMESPACE, GROUP_NAMESPACE, MEMBER_NAMESPACE, SUMMARY_NAMESPACE, SwitchContainerPayload,
};
use crate::{MediaError, schema_merge};

use super::{Binding, StatusRule};

const PAYLOAD: &str = "SwitchContainerPayload";

pub(super) fn insert_payload(base: &mut Value) -> Result<(), MediaError> {
    schema_merge::insert_payload(
        base,
        PAYLOAD,
        schema_merge::serialized_payload::<SwitchContainerPayload<'static>>(),
    )
}

pub(super) const fn bindings() -> [Binding; 4] {
    [
        Binding::exact(
            PAYLOAD,
            SUMMARY_NAMESPACE,
            "bank-hirc-switch-container",
            2,
            StatusRule::MalformedLayout,
        ),
        Binding::exact(
            PAYLOAD,
            GROUP_NAMESPACE,
            "bank-hirc-switch-membership-group",
            3,
            StatusRule::MalformedLayout,
        ),
        Binding::exact(
            PAYLOAD,
            MEMBER_NAMESPACE,
            "bank-hirc-switch-membership",
            4,
            StatusRule::Complete,
        ),
        Binding::exact(
            PAYLOAD,
            CONFIG_NAMESPACE,
            "bank-hirc-switch-child-configuration",
            3,
            StatusRule::MalformedLayout,
        ),
    ]
}
