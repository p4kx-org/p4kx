// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::Value;

use crate::audio_bank_rtpc_payload::{
    BLEND_CURVE_NAMESPACE, BLEND_POINT_NAMESPACE, BLEND_SUMMARY_NAMESPACE, BlendLayerRtpcPayload,
    ContainerRtpcPayload, RtpcOwner, RtpcPayload,
};
use crate::{MediaError, schema_merge};

use super::{Binding, StatusRule};

const RTPC_PAYLOAD: &str = "RtpcPayload";
const CONTAINER_PAYLOAD: &str = "ContainerRtpcPayload";
const BLEND_PAYLOAD: &str = "BlendLayerRtpcPayload";

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
    )?;
    schema_merge::insert_payload(
        base,
        BLEND_PAYLOAD,
        schema_merge::serialized_payload::<BlendLayerRtpcPayload<'static>>(),
    )
}

pub(super) const fn bindings() -> [Binding; 15] {
    let [summary, curve, point] = container_bindings();
    let [music_summary, music_curve, music_point] = music_bindings();
    let [blend_summary, blend_curve, blend_point] = blend_bindings();
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
        blend_summary,
        blend_curve,
        blend_point,
        music_summary,
        music_curve,
        music_point,
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

const fn music_bindings() -> [Binding; 3] {
    [
        Binding::exact(
            RTPC_PAYLOAD,
            RtpcOwner::MusicNode.summary_namespace(),
            "bank-hirc-music-node-rtpc",
            2,
            StatusRule::MalformedLayout,
        ),
        Binding::exact(
            RTPC_PAYLOAD,
            RtpcOwner::MusicNode.curve_namespace(),
            "bank-hirc-music-node-rtpc-curve",
            3,
            StatusRule::Complete,
        ),
        Binding::exact(
            RTPC_PAYLOAD,
            RtpcOwner::MusicNode.point_namespace(),
            "bank-hirc-music-node-rtpc-point",
            4,
            StatusRule::Complete,
        ),
    ]
}

const fn blend_bindings() -> [Binding; 3] {
    [
        Binding::exact(
            BLEND_PAYLOAD,
            BLEND_SUMMARY_NAMESPACE,
            "bank-hirc-blend-layer-rtpc",
            3,
            StatusRule::MalformedLayout,
        ),
        Binding::exact(
            BLEND_PAYLOAD,
            BLEND_CURVE_NAMESPACE,
            "bank-hirc-blend-layer-rtpc-curve",
            4,
            StatusRule::Complete,
        ),
        Binding::exact(
            BLEND_PAYLOAD,
            BLEND_POINT_NAMESPACE,
            "bank-hirc-blend-layer-rtpc-point",
            5,
            StatusRule::Complete,
        ),
    ]
}
