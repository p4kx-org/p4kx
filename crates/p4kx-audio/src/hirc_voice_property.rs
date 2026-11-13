// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::hirc_source::NODE_BASE_VERSION;

/// A v154 shared node property whose binary32 representation and unit are established.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum VoiceProperty {
    /// Relative voice level in decibels.
    VolumeDecibels,
    /// Relative voice pitch in cents.
    PitchCents,
    /// Relative low-pass filtering in percent.
    LowPassFilterPercent,
    /// Relative high-pass filtering in percent.
    HighPassFilterPercent,
}

impl VoiceProperty {
    /// Resolves a known shared node property ID in its original bank version.
    #[must_use]
    pub const fn from_node_property(version: u32, id: u8) -> Option<Self> {
        if version != NODE_BASE_VERSION {
            return None;
        }
        match id {
            0x00 => Some(Self::VolumeDecibels),
            0x01 => Some(Self::PitchCents),
            0x02 => Some(Self::LowPassFilterPercent),
            0x03 => Some(Self::HighPassFilterPercent),
            _ => None,
        }
    }

    /// Interprets one original binary32 word without substituting for a nonfinite value.
    #[must_use]
    pub fn finite_value(self, bits: u32) -> Option<f32> {
        let value = f32::from_bits(bits);
        value.is_finite().then_some(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn voice_property_units_are_version_qualified_and_nonfinite_words_have_no_number() {
        let cases = [
            (0, VoiceProperty::VolumeDecibels),
            (1, VoiceProperty::PitchCents),
            (2, VoiceProperty::LowPassFilterPercent),
            (3, VoiceProperty::HighPassFilterPercent),
        ];
        for (id, property) in cases {
            assert_eq!(
                VoiceProperty::from_node_property(154, id),
                Some(property),
                "qualified ID"
            );
            assert_eq!(
                VoiceProperty::from_node_property(153, id),
                None,
                "older version"
            );
            assert_eq!(
                VoiceProperty::from_node_property(155, id),
                None,
                "newer version"
            );
            assert_eq!(
                property.finite_value((-12.5_f32).to_bits()),
                Some(-12.5),
                "finite word"
            );
            assert_eq!(property.finite_value(0x7FC1_2345), None, "original NaN");
            assert_eq!(
                property.finite_value(0xFF80_0000),
                None,
                "original infinity"
            );
        }
        assert_eq!(
            VoiceProperty::from_node_property(154, 0xFA),
            None,
            "unqualified ID"
        );
    }
}
