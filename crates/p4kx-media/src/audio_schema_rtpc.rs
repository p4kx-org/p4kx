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

const ATTENUATION_RECORD_TYPES: [&str; 3] = [
    "bank-hirc-attenuation-rtpc",
    "bank-hirc-attenuation-rtpc-curve",
    "bank-hirc-attenuation-rtpc-point",
];

const PLUGIN_RECORD_TYPES: [&str; 3] = [
    "bank-hirc-plugin-rtpc",
    "bank-hirc-plugin-rtpc-curve",
    "bank-hirc-plugin-rtpc-point",
];

const MODULATOR_RECORD_TYPES: [&str; 3] = [
    "bank-hirc-modulator-rtpc",
    "bank-hirc-modulator-rtpc-curve",
    "bank-hirc-modulator-rtpc-point",
];

const BUS_RECORD_TYPES: [&str; 3] = [
    "bank-hirc-bus-rtpc",
    "bank-hirc-bus-rtpc-curve",
    "bank-hirc-bus-rtpc-point",
];

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

pub(super) const fn bindings() -> [Binding; 27] {
    let [sound_summary, sound_curve, sound_point] = shared_bindings(
        RtpcOwner::Sound,
        [
            "bank-hirc-sound-rtpc",
            "bank-hirc-sound-rtpc-curve",
            "bank-hirc-sound-rtpc-point",
        ],
    );
    let [actor_summary, actor_curve, actor_point] = shared_bindings(
        RtpcOwner::ActorMixer,
        [
            "bank-hirc-actor-mixer-rtpc",
            "bank-hirc-actor-mixer-rtpc-curve",
            "bank-hirc-actor-mixer-rtpc-point",
        ],
    );
    let [attenuation_summary, attenuation_curve, attenuation_point] =
        shared_bindings(RtpcOwner::Attenuation, ATTENUATION_RECORD_TYPES);
    let [plugin_summary, plugin_curve, plugin_point] =
        shared_bindings(RtpcOwner::Plugin, PLUGIN_RECORD_TYPES);
    let [modulator_summary, modulator_curve, modulator_point] =
        shared_bindings(RtpcOwner::Modulator, MODULATOR_RECORD_TYPES);
    let [bus_summary, bus_curve, bus_point] = shared_bindings(RtpcOwner::Bus, BUS_RECORD_TYPES);
    let [summary, curve, point] = container_bindings();
    let [music_summary, music_curve, music_point] = music_bindings();
    let [blend_summary, blend_curve, blend_point] = blend_bindings();
    [
        sound_summary,
        sound_curve,
        sound_point,
        actor_summary,
        actor_curve,
        actor_point,
        summary,
        curve,
        point,
        blend_summary,
        blend_curve,
        blend_point,
        music_summary,
        music_curve,
        music_point,
        attenuation_summary,
        attenuation_curve,
        attenuation_point,
        bus_summary,
        bus_curve,
        bus_point,
        plugin_summary,
        plugin_curve,
        plugin_point,
        modulator_summary,
        modulator_curve,
        modulator_point,
    ]
}

const fn shared_bindings(
    owner: RtpcOwner,
    [summary, curve, point]: [&'static str; 3],
) -> [Binding; 3] {
    [
        Binding::exact(
            RTPC_PAYLOAD,
            owner.summary_namespace(),
            summary,
            2,
            StatusRule::MalformedLayout,
        ),
        Binding::exact(
            RTPC_PAYLOAD,
            owner.curve_namespace(),
            curve,
            3,
            StatusRule::Complete,
        ),
        Binding::exact(
            RTPC_PAYLOAD,
            owner.point_namespace(),
            point,
            4,
            StatusRule::Complete,
        ),
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
