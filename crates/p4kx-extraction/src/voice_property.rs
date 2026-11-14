// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

pub use p4kx_audio::VoiceProperty;

/// Wire identity of a qualified Wwise node property, including its unit.
#[derive(Debug, Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[schemars(extend("x-p4kx-vocabularyClass" = "control"))]
#[non_exhaustive]
pub enum VoicePropertyName {
    /// Relative voice level in decibels.
    VolumeDecibels,
    /// Relative voice pitch in cents.
    PitchCents,
    /// Relative low-pass filtering in percent.
    LowPassFilterPercent,
    /// Relative high-pass filtering in percent.
    HighPassFilterPercent,
}

impl VoicePropertyName {
    /// Returns the wire identity for a qualified property.
    #[must_use]
    pub const fn from_property(property: VoiceProperty) -> Option<Self> {
        match property {
            VoiceProperty::VolumeDecibels => Some(Self::VolumeDecibels),
            VoiceProperty::PitchCents => Some(Self::PitchCents),
            VoiceProperty::LowPassFilterPercent => Some(Self::LowPassFilterPercent),
            VoiceProperty::HighPassFilterPercent => Some(Self::HighPassFilterPercent),
            _ => None,
        }
    }
}
