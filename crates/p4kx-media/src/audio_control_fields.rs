// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::sound;
use schemars::JsonSchema;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum AtlFamily {
    Configuration,
    Triggers,
    Trigger,
    WwiseEvent,
    Rtpcs,
    Rtpc,
    WwiseRtpc,
    Switches,
    Switch,
    SwitchState,
    WwiseSwitch,
    WwiseState,
    WwiseValue,
    Environments,
    Environment,
    WwiseAuxBus,
    Preloads,
    PreloadRequest,
    Preload,
    RtpcDependees,
    RtpcDependee,
    ConfigGroup,
    Platforms,
    Platform,
    Tags,
    Tag,
    WwiseFile,
    ExternalSources,
    ExternalSource,
    ExternalSourceEntry,
    WwiseExternalSource,
}

impl AtlFamily {
    pub(crate) fn from_tag(tag: &str) -> Option<Self> {
        Some(match tag {
            "ATLConfig" => Self::Configuration,
            "AudioTriggers" => Self::Triggers,
            "ATLTrigger" => Self::Trigger,
            "WwiseEvent" => Self::WwiseEvent,
            "AudioRTPCs" | "AudioRtpcs" => Self::Rtpcs,
            "ATLRtpc" => Self::Rtpc,
            "WwiseRtpc" => Self::WwiseRtpc,
            "AudioSwitches" => Self::Switches,
            "ATLSwitch" => Self::Switch,
            "ATLSwitchState" => Self::SwitchState,
            "WwiseSwitch" => Self::WwiseSwitch,
            "WwiseState" => Self::WwiseState,
            "WwiseValue" => Self::WwiseValue,
            "AudioEnvironments" => Self::Environments,
            "ATLEnvironment" => Self::Environment,
            "WwiseAuxBus" => Self::WwiseAuxBus,
            "AudioPreloads" => Self::Preloads,
            "ATLPreloadRequest" => Self::PreloadRequest,
            "ATLPreload" => Self::Preload,
            "RTPCDependees" => Self::RtpcDependees,
            "ATLRTPCDependee" => Self::RtpcDependee,
            "ATLConfigGroup" => Self::ConfigGroup,
            "ATLPlatforms" => Self::Platforms,
            "Platform" => Self::Platform,
            "Tags" => Self::Tags,
            "Tag" => Self::Tag,
            "WwiseFile" => Self::WwiseFile,
            "AudioExternalSources" => Self::ExternalSources,
            "ATLExternalSource" => Self::ExternalSource,
            "ATLExternalSourceEntry" => Self::ExternalSourceEntry,
            "WwiseExternalSource" => Self::WwiseExternalSource,
            _ => return None,
        })
    }
}

#[derive(Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum AtlField {
    Name,
    Path,
    WwiseName,
    Localised,
    DurationType,
    DurationMin,
    DurationMax,
    RadiusMax,
    ListenerRtpc,
    RtpcGroupName,
    RtpcMin,
    RtpcMax,
    RtpcDefault,
    ExternalFilename,
    Language,
    PreloadType,
    ConfigGroupName,
    Multiplier,
    Shift,
    WwiseValue,
}

#[derive(Serialize, JsonSchema)]
#[serde(
    tag = "type",
    content = "value",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
pub(crate) enum AtlScalar<'document> {
    Text(&'document str),
    Boolean(bool),
    Number(f64),
    WwiseEvent { name: &'document str, id: u32 },
    InvalidBoolean,
    InvalidNumber,
}

impl AtlField {
    pub(crate) fn from_attribute(family: AtlFamily, name: &str) -> Option<Self> {
        Some(match (family, name) {
            (_, "atl_name") => Self::Name,
            (AtlFamily::Rtpc | AtlFamily::WwiseRtpc, "listener_rtpc") => Self::ListenerRtpc,
            (AtlFamily::Rtpc | AtlFamily::WwiseRtpc, "rtpc_group_name") => Self::RtpcGroupName,
            (AtlFamily::PreloadRequest, "atl_type") => Self::PreloadType,
            (AtlFamily::Platform, "atl_config_group_name") => Self::ConfigGroupName,
            (
                AtlFamily::Trigger
                | AtlFamily::PreloadRequest
                | AtlFamily::Rtpc
                | AtlFamily::ExternalSource,
                "path",
            ) => Self::Path,
            (AtlFamily::ExternalSourceEntry, "language") => Self::Language,
            (AtlFamily::WwiseExternalSource, "wwise_filename") => Self::ExternalFilename,
            (AtlFamily::WwiseRtpc, "min") => Self::RtpcMin,
            (AtlFamily::WwiseRtpc, "max") => Self::RtpcMax,
            (AtlFamily::WwiseRtpc, "default") => Self::RtpcDefault,
            (
                AtlFamily::WwiseEvent
                | AtlFamily::WwiseFile
                | AtlFamily::WwiseExternalSource
                | AtlFamily::WwiseRtpc
                | AtlFamily::WwiseSwitch
                | AtlFamily::WwiseState
                | AtlFamily::WwiseAuxBus
                | AtlFamily::WwiseValue,
                "wwise_name",
            ) => Self::WwiseName,
            (AtlFamily::ExternalSource, "is_localised")
            | (
                AtlFamily::WwiseEvent | AtlFamily::WwiseFile | AtlFamily::WwiseExternalSource,
                "wwise_localised" | "wwise_localized",
            ) => Self::Localised,
            (AtlFamily::WwiseRtpc, "atl_mult") => Self::Multiplier,
            (AtlFamily::WwiseRtpc, "atl_shift") => Self::Shift,
            (AtlFamily::WwiseValue, "wwise_value") => Self::WwiseValue,
            (AtlFamily::WwiseEvent | AtlFamily::WwiseExternalSource, "wwise_duration_type") => {
                Self::DurationType
            }
            (AtlFamily::WwiseEvent | AtlFamily::WwiseExternalSource, "wwise_duration_min") => {
                Self::DurationMin
            }
            (AtlFamily::WwiseEvent | AtlFamily::WwiseExternalSource, "wwise_duration_max") => {
                Self::DurationMax
            }
            (AtlFamily::WwiseEvent, "wwise_radius_max") => Self::RadiusMax,
            _ => return None,
        })
    }

    pub(crate) fn value(self, family: AtlFamily, value: &str) -> AtlScalar<'_> {
        match self {
            Self::WwiseName if family == AtlFamily::WwiseEvent && !value.is_empty() => {
                AtlScalar::WwiseEvent {
                    name: value,
                    id: sound::wwise_id(value),
                }
            }
            Self::DurationMin
            | Self::DurationMax
            | Self::RadiusMax
            | Self::RtpcMin
            | Self::RtpcMax
            | Self::RtpcDefault
            | Self::Multiplier
            | Self::Shift => value
                .parse::<f64>()
                .ok()
                .filter(|number| number.is_finite())
                .map_or(AtlScalar::InvalidNumber, AtlScalar::Number),
            Self::Localised if value.eq_ignore_ascii_case("true") => AtlScalar::Boolean(true),
            Self::Localised if value.eq_ignore_ascii_case("false") => AtlScalar::Boolean(false),
            Self::Localised => AtlScalar::InvalidBoolean,
            _ => AtlScalar::Text(value),
        }
    }
}
