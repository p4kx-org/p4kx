// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use schemars::JsonSchema;
use serde::Serialize;

use crate::audio_bank_payload::{BankEntryRange, BankField};

#[derive(Serialize, JsonSchema)]
#[serde(tag = "record_type", content = "payload", deny_unknown_fields)]
pub(crate) enum RtpcPayload<'row> {
    #[serde(rename = "bank-hirc-sound-rtpc")]
    SoundSummary(&'row RtpcRecord),
    #[serde(rename = "bank-hirc-sound-rtpc-curve")]
    SoundCurve(&'row RtpcCurveRecord),
    #[serde(rename = "bank-hirc-sound-rtpc-point")]
    SoundPoint(&'row RtpcPointRecord),
    #[serde(rename = "bank-hirc-actor-mixer-rtpc")]
    ActorMixerSummary(&'row RtpcRecord),
    #[serde(rename = "bank-hirc-actor-mixer-rtpc-curve")]
    ActorMixerCurve(&'row RtpcCurveRecord),
    #[serde(rename = "bank-hirc-actor-mixer-rtpc-point")]
    ActorMixerPoint(&'row RtpcPointRecord),
}

#[derive(Serialize, JsonSchema)]
#[serde(tag = "record_type", content = "payload", deny_unknown_fields)]
pub(crate) enum ContainerRtpcPayload<'row> {
    #[serde(rename = "bank-hirc-container-rtpc")]
    Summary(&'row RtpcRecord),
    #[serde(rename = "bank-hirc-container-rtpc-curve")]
    Curve(&'row RtpcCurveRecord),
    #[serde(rename = "bank-hirc-container-rtpc-point")]
    Point(&'row RtpcPointRecord),
}

pub(crate) const BLEND_SUMMARY_NAMESPACE: &str = "audio-bank-hirc-blend-layer-rtpc";
pub(crate) const BLEND_CURVE_NAMESPACE: &str = "audio-bank-hirc-blend-layer-rtpc-curve";
pub(crate) const BLEND_POINT_NAMESPACE: &str = "audio-bank-hirc-blend-layer-rtpc-point";

#[derive(Serialize, JsonSchema)]
#[serde(tag = "record_type", content = "payload", deny_unknown_fields)]
pub(crate) enum BlendLayerRtpcPayload<'row> {
    #[serde(rename = "bank-hirc-blend-layer-rtpc")]
    Summary(&'row RtpcRecord),
    #[serde(rename = "bank-hirc-blend-layer-rtpc-curve")]
    Curve(&'row RtpcCurveRecord),
    #[serde(rename = "bank-hirc-blend-layer-rtpc-point")]
    Point(&'row RtpcPointRecord),
}

#[derive(Serialize)]
#[serde(untagged)]
pub(crate) enum RtpcExport<'row> {
    Shared(RtpcPayload<'row>),
    Container(ContainerRtpcPayload<'row>),
    BlendLayer(BlendLayerRtpcPayload<'row>),
}

#[derive(Clone, Copy)]
pub(crate) enum RtpcOwner {
    Sound,
    ActorMixer,
    Container,
    BlendLayer(BlendLayerSite),
}

#[derive(Clone, Copy)]
pub(crate) struct BlendLayerSite {
    ordinal: u32,
    id_start: u64,
    id_end: u64,
    checked_end: u64,
}

impl BlendLayerSite {
    pub(crate) const fn id_range(self) -> Range<u64> {
        self.id_start..self.id_end
    }

    pub(crate) const fn checked_range(self) -> Range<u64> {
        self.id_start..self.checked_end
    }
}

impl RtpcOwner {
    pub(crate) const fn blend_layer(ordinal: u32, id: &Range<u64>, checked_end: u64) -> Self {
        Self::BlendLayer(BlendLayerSite {
            ordinal,
            id_start: id.start,
            id_end: id.end,
            checked_end,
        })
    }

    pub(crate) fn indices(self, object: [u64; 2], tail: &[u64]) -> Vec<u64> {
        let layer_count = usize::from(matches!(self, Self::BlendLayer(_)));
        let mut indices = Vec::with_capacity(object.len() + layer_count + tail.len());
        indices.extend(object);
        if let Self::BlendLayer(layer) = self {
            indices.push(u64::from(layer.ordinal));
        }
        indices.extend_from_slice(tail);
        indices
    }

    pub(crate) const fn summary_namespace(self) -> &'static str {
        match self {
            Self::Sound => "audio-bank-hirc-sound-rtpc",
            Self::ActorMixer => "audio-bank-hirc-actor-mixer-rtpc",
            Self::Container => "audio-bank-hirc-container-rtpc",
            Self::BlendLayer(_) => BLEND_SUMMARY_NAMESPACE,
        }
    }

    pub(crate) const fn curve_namespace(self) -> &'static str {
        match self {
            Self::Sound => "audio-bank-hirc-sound-rtpc-curve",
            Self::ActorMixer => "audio-bank-hirc-actor-mixer-rtpc-curve",
            Self::Container => "audio-bank-hirc-container-rtpc-curve",
            Self::BlendLayer(_) => BLEND_CURVE_NAMESPACE,
        }
    }

    pub(crate) const fn point_namespace(self) -> &'static str {
        match self {
            Self::Sound => "audio-bank-hirc-sound-rtpc-point",
            Self::ActorMixer => "audio-bank-hirc-actor-mixer-rtpc-point",
            Self::Container => "audio-bank-hirc-container-rtpc-point",
            Self::BlendLayer(_) => BLEND_POINT_NAMESPACE,
        }
    }

    pub(crate) const fn summary(self, value: &RtpcRecord) -> RtpcExport<'_> {
        match self {
            Self::Sound => RtpcExport::Shared(RtpcPayload::SoundSummary(value)),
            Self::ActorMixer => RtpcExport::Shared(RtpcPayload::ActorMixerSummary(value)),
            Self::Container => RtpcExport::Container(ContainerRtpcPayload::Summary(value)),
            Self::BlendLayer(_) => RtpcExport::BlendLayer(BlendLayerRtpcPayload::Summary(value)),
        }
    }

    pub(crate) const fn curve(self, value: &RtpcCurveRecord) -> RtpcExport<'_> {
        match self {
            Self::Sound => RtpcExport::Shared(RtpcPayload::SoundCurve(value)),
            Self::ActorMixer => RtpcExport::Shared(RtpcPayload::ActorMixerCurve(value)),
            Self::Container => RtpcExport::Container(ContainerRtpcPayload::Curve(value)),
            Self::BlendLayer(_) => RtpcExport::BlendLayer(BlendLayerRtpcPayload::Curve(value)),
        }
    }

    pub(crate) const fn point(self, value: &RtpcPointRecord) -> RtpcExport<'_> {
        match self {
            Self::Sound => RtpcExport::Shared(RtpcPayload::SoundPoint(value)),
            Self::ActorMixer => RtpcExport::Shared(RtpcPayload::ActorMixerPoint(value)),
            Self::Container => RtpcExport::Container(ContainerRtpcPayload::Point(value)),
            Self::BlendLayer(_) => RtpcExport::BlendLayer(BlendLayerRtpcPayload::Point(value)),
        }
    }
}

#[derive(Serialize, JsonSchema)]
#[serde(tag = "layout", rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) enum RtpcRecord {
    Decoded {
        version: u32,
        curve_count: BankField<u16>,
        checked_curves: u16,
        checked_range: BankEntryRange,
    },
    Malformed {
        version: u32,
        curve_count: BankField<u16>,
        checked_curves: u16,
        checked_range: BankEntryRange,
    },
    Absent {
        version: u32,
    },
    Unavailable {
        version: u32,
    },
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct RtpcCurveRecord {
    pub(crate) rtpc_id: BankField<u32>,
    pub(crate) rtpc_type: BankField<u8>,
    pub(crate) accumulation: BankField<u8>,
    pub(crate) parameter_id: BankField<u32>,
    pub(crate) curve_id: BankField<u32>,
    pub(crate) scaling: BankField<u8>,
    pub(crate) point_count: BankField<u16>,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct RtpcPointRecord {
    pub(crate) from_bits: BankField<u32>,
    pub(crate) to_bits: BankField<u32>,
    pub(crate) interpolation: BankField<u32>,
    pub(crate) input: Option<f32>,
    pub(crate) output: Option<f32>,
}
