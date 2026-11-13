// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::{AuthoredAttribute, AuthoredAttributeKind};

/// Recognized material input, independent of renderer defaults.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum EmissionAttributeKind {
    /// Original Glow scalar.
    Glow,
    /// Original Emissive colour.
    Emissive,
    /// Original shader permutation names.
    ShaderMask,
    /// Original shader family name.
    Shader,
    /// Original material flag bitset.
    Flags,
    /// Original alpha test cutoff.
    AlphaTest,
    /// Original opacity control.
    Opacity,
    /// Authored cutoff for alpha influence on glow.
    AlphaGlowCutoff,
    /// Authored emission colour parameter.
    EmissiveTint,
}

impl EmissionAttributeKind {
    pub(super) fn direct(attribute: &AuthoredAttribute) -> Option<Self> {
        if !matches!(
            attribute.kind,
            AuthoredAttributeKind::Attribute { namespace: None }
        ) {
            return None;
        }
        Some(match attribute.name.as_str() {
            "Glow" => Self::Glow,
            "Emissive" => Self::Emissive,
            "StringGenMask" => Self::ShaderMask,
            "Shader" => Self::Shader,
            "MtlFlags" => Self::Flags,
            "AlphaTest" => Self::AlphaTest,
            "Opacity" => Self::Opacity,
            "AlphaGlowCutoff" => Self::AlphaGlowCutoff,
            "EmissiveTint" => Self::EmissiveTint,
            _ => return None,
        })
    }

    pub(super) fn parameter(attribute: &AuthoredAttribute) -> Option<Self> {
        match Self::direct(attribute)? {
            kind @ (Self::AlphaGlowCutoff | Self::EmissiveTint) => Some(kind),
            _ => None,
        }
    }
}

/// Original emission value before applying shader defaults.
#[derive(Debug, PartialEq)]
#[non_exhaustive]
pub struct EmissionAttribute<'a> {
    /// Recognized emission or coverage input.
    pub kind: EmissionAttributeKind,
    /// Original XML declaration and interpreted value when recognized.
    pub value: EmissionValue<'a>,
}

impl<'a> EmissionAttribute<'a> {
    pub(super) fn new(
        kind: EmissionAttributeKind,
        element: Vec<usize>,
        ordinal: usize,
        raw: &'a AuthoredAttribute,
    ) -> Self {
        let interpreted = match kind {
            EmissionAttributeKind::Shader | EmissionAttributeKind::ShaderMask => {
                EmissionNumeric::Text
            }
            EmissionAttributeKind::Flags => {
                parse_flags(&raw.value).map_or(EmissionNumeric::Invalid, EmissionNumeric::Flags)
            }
            EmissionAttributeKind::Emissive | EmissionAttributeKind::EmissiveTint => {
                parse_colour(&raw.value).map_or(EmissionNumeric::Invalid, EmissionNumeric::Colour)
            }
            _ => parse_number(&raw.value).map_or(EmissionNumeric::Invalid, EmissionNumeric::Scalar),
        };
        Self {
            kind,
            value: EmissionValue {
                element,
                ordinal,
                raw,
                interpreted,
            },
        }
    }
}

/// One exact authored XML value with an optional typed interpretation.
#[derive(Debug, PartialEq)]
#[non_exhaustive]
pub struct EmissionValue<'a> {
    /// Original element-child ordinals from the library root.
    pub element: Vec<usize>,
    /// Ordinal in the element's original XML declaration list.
    pub ordinal: usize,
    /// Exact authored name, value and original plain-XML token when captured.
    pub raw: &'a AuthoredAttribute,
    /// Authored value parsed without evaluation or clamp.
    pub interpreted: EmissionNumeric,
}

impl EmissionValue<'_> {
    pub(super) const fn flags(&self) -> Option<u64> {
        match self.interpreted {
            EmissionNumeric::Flags(bits) => Some(bits),
            _ => None,
        }
    }
}

/// Explicit interpretation of the original emission input.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub enum EmissionNumeric {
    /// Declaration is textual.
    Text,
    /// The stored scalar.
    Scalar(EmissionNumber),
    /// The stored colour channels in original order.
    Colour([EmissionNumber; 3]),
    /// Original hexadecimal or decimal bitset.
    Flags(u64),
    /// The authored token was not in the corresponding value grammar.
    Invalid,
}

/// A numeric source token without a lossy finite substitution.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub enum EmissionNumber {
    /// Exact finite float read by the material's number parser.
    Finite(f32),
    /// Non-finite NaN.
    Nan,
    /// Positive infinity.
    PositiveInfinity,
    /// Negative infinity.
    NegativeInfinity,
}

/// One original emittance texture declaration with physical ordinals.
#[derive(Debug, PartialEq)]
#[non_exhaustive]
pub struct EmissionTextureSlot<'a> {
    /// Original texture Map declaration.
    pub map: EmissionValue<'a>,
    /// Original texture File declaration.
    pub file: Option<EmissionValue<'a>>,
}

impl<'a> EmissionTextureSlot<'a> {
    pub(super) fn new(
        element: Vec<usize>,
        ordinal: usize,
        raw: &'a AuthoredAttribute,
        file: Option<(usize, &'a AuthoredAttribute)>,
    ) -> Self {
        Self {
            file: file.map(|(ordinal, raw)| EmissionValue {
                element: element.clone(),
                ordinal,
                raw,
                interpreted: EmissionNumeric::Text,
            }),
            map: EmissionValue {
                element,
                ordinal,
                raw,
                interpreted: EmissionNumeric::Text,
            },
        }
    }
}

fn parse_number(raw: &str) -> Option<EmissionNumber> {
    let value = raw.trim().parse::<f32>().ok()?;
    Some(if value.is_finite() {
        EmissionNumber::Finite(value)
    } else if value.is_nan() {
        EmissionNumber::Nan
    } else if value.is_sign_negative() {
        EmissionNumber::NegativeInfinity
    } else {
        EmissionNumber::PositiveInfinity
    })
}

fn parse_colour(raw: &str) -> Option<[EmissionNumber; 3]> {
    let mut parts = raw.split(',').map(parse_number);
    let colour = [parts.next()??, parts.next()??, parts.next()??];
    parts.next().is_none().then_some(colour)
}

fn parse_flags(raw: &str) -> Option<u64> {
    let raw = raw.trim();
    if let Some(hex) = raw.strip_prefix("0x").or_else(|| raw.strip_prefix("0X")) {
        return u64::from_str_radix(hex, 16).ok();
    }
    raw.parse::<u64>().ok().or_else(|| {
        raw.parse::<i64>()
            .ok()
            .map(|bits| u64::from_ne_bytes(bits.to_ne_bytes()))
    })
}
