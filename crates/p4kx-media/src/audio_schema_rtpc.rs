// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::Value;

use crate::audio_bank_rtpc_payload::{ContainerRtpcPayload, RtpcOwner, RtpcPayload};
use crate::{MediaError, schema_merge};

use super::{Binding, StatusRule};

const RTPC_PAYLOAD: &str = "RtpcPayload";
const CONTAINER_PAYLOAD: &str = "ContainerRtpcPayload";

pub(super) fn insert_payload(base: &mut Value) -> Result<(), MediaError> {
    schema_merge::insert_payload(
        base,
        RTPC_PAYLOAD,
        schema_merge::serialized_payload::<RtpcPayload<'static>>(),
    )?;
    schema_merge::insert_payload(
        base,
        CONTAINER_PAYLOAD,
        schema_merge::serialized_payload::<ContainerRtpcPayload<'static>>(),
    )
}

pub(super) const fn bindings() -> [Binding; 9] {
    let [summary, curve, point] = container_bindings();
    [
        Binding::exact(
            RTPC_PAYLOAD,
            RtpcOwner::Sound.summary_namespace(),
            "bank-hirc-sound-rtpc",
            2,
            StatusRule::MalformedLayout,
        ),
        Binding::exact(
            RTPC_PAYLOAD,
            RtpcOwner::Sound.curve_namespace(),
            "bank-hirc-sound-rtpc-curve",
            3,
            StatusRule::Complete,
        ),
        Binding::exact(
            RTPC_PAYLOAD,
            RtpcOwner::Sound.point_namespace(),
            "bank-hirc-sound-rtpc-point",
            4,
            StatusRule::Complete,
        ),
        Binding::exact(
            RTPC_PAYLOAD,
            RtpcOwner::ActorMixer.summary_namespace(),
            "bank-hirc-actor-mixer-rtpc",
            2,
            StatusRule::MalformedLayout,
        ),
        Binding::exact(
            RTPC_PAYLOAD,
            RtpcOwner::ActorMixer.curve_namespace(),
            "bank-hirc-actor-mixer-rtpc-curve",
            3,
            StatusRule::Complete,
        ),
        Binding::exact(
            RTPC_PAYLOAD,
            RtpcOwner::ActorMixer.point_namespace(),
            "bank-hirc-actor-mixer-rtpc-point",
            4,
            StatusRule::Complete,
        ),
        summary,
        curve,
        point,
    ]
}

const fn container_bindings() -> [Binding; 3] {
    [
        Binding::exact(
            CONTAINER_PAYLOAD,
            RtpcOwner::Container.summary_namespace(),
            "bank-hirc-container-rtpc",
            2,
            StatusRule::MalformedLayout,
        ),
        Binding::exact(
            CONTAINER_PAYLOAD,
            RtpcOwner::Container.curve_namespace(),
            "bank-hirc-container-rtpc-curve",
            3,
            StatusRule::Complete,
        ),
        Binding::exact(
            CONTAINER_PAYLOAD,
            RtpcOwner::Container.point_namespace(),
            "bank-hirc-container-rtpc-point",
            4,
            StatusRule::Complete,
        ),
    ]
}
