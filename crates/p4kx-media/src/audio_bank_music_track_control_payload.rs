// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::audio_bank_node_declaration_payload::NodeDeclaration;
use crate::audio_bank_payload::{BankEntryRange, BankField};

pub(crate) const CONTROLS: &str = "audio-bank-music-track-controls";
pub(crate) const AUTOMATION: &str = "audio-bank-music-track-automation";
pub(crate) const POINT: &str = "audio-bank-music-track-automation-point";

#[derive(Serialize, JsonSchema)]
#[serde(tag = "record_type", content = "payload", deny_unknown_fields)]
pub(crate) enum TrackControlPayload<'row> {
    #[serde(rename = "bank-music-track-controls")]
    Controls(&'row NodeDeclaration<TrackControlObservations>),
    #[serde(rename = "bank-music-track-automation")]
    Automation(&'row TrackAutomationRecord),
    #[serde(rename = "bank-music-track-automation-point")]
    Point(&'row TrackAutomationPoint),
}

#[derive(Default, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct TrackControlObservations {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) flags: Option<BankField<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) subtrack_count: Option<BankField<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) automation: Option<TrackAutomationTable>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) transition: Option<TrackTransition>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) look_ahead: Option<TrackSignedWord>,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct TrackTransition {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) source: Option<TrackFade>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) sync_type: Option<BankField<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) cue_filter_hash: Option<BankField<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) destination: Option<TrackFade>,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct TrackFade {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) time: Option<TrackSignedWord>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) curve: Option<BankField<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) offset: Option<TrackSignedWord>,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct TrackSignedWord {
    pub(crate) bits: BankField<u32>,
    pub(crate) signed_value: i32,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct TrackAutomationTable {
    pub(crate) declared_count: BankField<u32>,
    pub(crate) checked_range: BankEntryRange,
    pub(crate) coverage: TrackAutomationCoverage,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum TrackAutomationCoverage {
    Complete,
    Truncated,
}

#[derive(Serialize, JsonSchema)]
#[serde(tag = "layout", rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) enum TrackAutomationRecord {
    Decoded {
        clip_index: BankField<u32>,
        automation_type: BankField<u32>,
        points: TrackAutomationTable,
    },
    Malformed {
        clip_index: BankField<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        automation_type: Option<BankField<u32>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        points: Option<TrackAutomationTable>,
    },
}

#[derive(Serialize, JsonSchema)]
#[serde(tag = "layout", rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) enum TrackAutomationPoint {
    Decoded {
        from: TrackAutomationWord,
        to: TrackAutomationWord,
        interpolation: BankField<u32>,
    },
    Malformed {
        from: TrackAutomationWord,
        #[serde(skip_serializing_if = "Option::is_none")]
        to: Option<TrackAutomationWord>,
    },
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct TrackAutomationWord {
    pub(crate) bits: BankField<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) finite_value: Option<f32>,
}
