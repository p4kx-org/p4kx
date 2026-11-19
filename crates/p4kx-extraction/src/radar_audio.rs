// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Source classification for `RadarAudioData` XML and CryXml declarations.

use schemars::JsonSchema;
use serde::Serialize;

use crate::cryxml::{SourceDeclaration, SourceDeclarationKind, SourceElement};

/// A recognised element in the authored `RadarAudioData` hierarchy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
#[schemars(extend("x-p4kx-vocabularyClass" = "control"))]
pub enum RadarAudioElement {
    /// The owning document root.
    Configuration,
    /// A declared zoom range.
    DefaultZoom,
    /// An ordered group of category declarations.
    CategoryTypes,
    /// A category with authored identification and display attributes.
    CategoryType,
    /// An ordered group of sound-event declarations.
    SoundEvents,
    /// A named slot and its authored sound-event value.
    SoundEvent,
}

impl RadarAudioElement {
    /// Identifies an unqualified `RadarAudioData` document root.
    #[must_use]
    pub fn root(element: SourceElement<'_>) -> Option<Self> {
        (element.namespace().is_none() && element.name() == "RadarAudioData")
            .then_some(Self::Configuration)
    }

    /// Identifies a recognised direct child at its authored position in the hierarchy.
    #[must_use]
    pub fn child(self, element: SourceElement<'_>) -> Option<Self> {
        if element.namespace().is_some() {
            return None;
        }
        Some(match (self, element.name()) {
            (Self::Configuration, "DefaultZoom") => Self::DefaultZoom,
            (Self::Configuration, "CategoryTypes") => Self::CategoryTypes,
            (Self::CategoryTypes, "CategoryType") => Self::CategoryType,
            (Self::Configuration, "SoundEvents") => Self::SoundEvents,
            (Self::SoundEvents, "SoundEvent") => Self::SoundEvent,
            _ => return None,
        })
    }

    /// Projects a recognised unqualified attribute while retaining its original text.
    #[must_use]
    pub fn attribute<'source>(
        self,
        declaration: &SourceDeclaration<'source>,
    ) -> Option<RadarAudioAttribute<'source>> {
        if !matches!(
            declaration.kind(),
            SourceDeclarationKind::Attribute { namespace: None }
        ) {
            return None;
        }
        let field = match (self, declaration.qualified_name()) {
            (Self::DefaultZoom, "min") => RadarAudioField::ZoomMin,
            (Self::DefaultZoom, "max") => RadarAudioField::ZoomMax,
            (Self::CategoryType, "name") => RadarAudioField::CategoryName,
            (Self::CategoryType, "value") => RadarAudioField::CategoryValue,
            (Self::CategoryType, "displayTime") => RadarAudioField::DisplayTime,
            (Self::CategoryType, "displayStartAlpha") => RadarAudioField::DisplayStartAlpha,
            (Self::SoundEvent, "name") => RadarAudioField::SoundEventName,
            (Self::SoundEvent, "value") => RadarAudioField::SoundEventValue,
            _ => return None,
        };
        Some(RadarAudioAttribute {
            field,
            value: field.read(declaration.value()),
        })
    }
}

/// The meaning of an unqualified attribute of a recognised `RadarAudioData` element.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
#[schemars(extend("x-p4kx-vocabularyClass" = "control"))]
pub enum RadarAudioField {
    /// The lower zoom limit.
    ZoomMin,
    /// The upper zoom limit.
    ZoomMax,
    /// The authored category name.
    CategoryName,
    /// The exact integral category value.
    CategoryValue,
    /// The authored display time.
    DisplayTime,
    /// The starting display alpha.
    DisplayStartAlpha,
    /// The named sound-event slot.
    SoundEventName,
    /// The sound-event token assigned to the slot.
    SoundEventValue,
}

impl RadarAudioField {
    fn read(self, raw: &str) -> RadarAudioValue<'_> {
        if raw.is_empty() {
            return RadarAudioValue::Empty;
        }
        match self {
            Self::CategoryName | Self::SoundEventName | Self::SoundEventValue => {
                RadarAudioValue::Text { raw }
            }
            Self::CategoryValue => {
                let digits = raw.strip_prefix(['+', '-']).unwrap_or(raw);
                if !digits.is_empty() && digits.bytes().all(|digit| digit.is_ascii_digit()) {
                    RadarAudioValue::Integer { raw }
                } else {
                    RadarAudioValue::Invalid { raw }
                }
            }
            Self::ZoomMin | Self::ZoomMax | Self::DisplayTime | Self::DisplayStartAlpha => {
                match raw.parse::<f64>() {
                    Ok(value) if value.is_finite() => RadarAudioValue::Number { raw, value },
                    _ => RadarAudioValue::Invalid { raw },
                }
            }
        }
    }
}

/// A typed attribute and its original declaration value.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[non_exhaustive]
pub struct RadarAudioAttribute<'source> {
    /// The meaning of the original attribute.
    pub field: RadarAudioField,
    /// The exact token and its typed interpretation.
    pub value: RadarAudioValue<'source>,
}

/// The state of an authored `RadarAudioData` attribute.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, JsonSchema)]
#[serde(tag = "state", rename_all = "kebab-case", deny_unknown_fields)]
#[non_exhaustive]
pub enum RadarAudioValue<'source> {
    /// A present attribute with no text.
    Empty,
    /// An authored text token.
    Text {
        /// The original decoded value.
        raw: &'source str,
    },
    /// An integral decimal token with storage-width-independent precision.
    Integer {
        /// The original decoded value, including any sign and leading zeros.
        raw: &'source str,
    },
    /// A finite numeric interpretation alongside the unmodified token.
    Number {
        /// The original decoded value.
        raw: &'source str,
        /// The value parsed as a binary64 number.
        value: f64,
    },
    /// A present token that cannot be interpreted as the declared numeric field.
    Invalid {
        /// The original decoded value that failed interpretation.
        raw: &'source str,
    },
}
