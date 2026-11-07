// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::MediaError;

/// Source integer width, independent of JSON or JavaScript numeric precision.
#[derive(Debug, Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum IntegerWidth {
    /// Eight source bits.
    Bits8,
    /// Sixteen source bits.
    Bits16,
    /// Thirty-two source bits.
    Bits32,
    /// Sixty-four source bits.
    Bits64,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "kebab-case")]
enum IdentifierValue {
    Path {
        value: String,
    },
    Guid {
        value: String,
    },
    Text {
        namespace: String,
        value: String,
    },
    Unsigned {
        namespace: String,
        width: IntegerWidth,
        value: String,
    },
    Signed {
        namespace: String,
        width: IntegerWidth,
        value: String,
    },
}

/// An unmodified source identifier with a distinct domain and integer width.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(transparent)]
pub struct OriginalIdentifier(IdentifierValue);

impl OriginalIdentifier {
    /// Retains path text without separator or case normalisation.
    #[must_use]
    pub fn new_with_path(value: impl Into<String>) -> Self {
        Self(IdentifierValue::Path {
            value: value.into(),
        })
    }

    /// Retains raw GUID text, including malformed or dangling declarations.
    #[must_use]
    pub fn new_with_guid_text(value: impl Into<String>) -> Self {
        Self(IdentifierValue::Guid {
            value: value.into(),
        })
    }

    /// Retains namespaced text without reinterpretation.
    ///
    /// # Errors
    /// Returns an invalid namespace error.
    pub fn try_new_with_text(
        namespace: impl Into<String>,
        value: impl Into<String>,
    ) -> Result<Self, MediaError> {
        let namespace = namespace.into();
        validate_namespace(&namespace)?;
        Ok(Self(IdentifierValue::Text {
            namespace,
            value: value.into(),
        }))
    }

    /// Retains an unsigned value as exact decimal text with its source width.
    ///
    /// # Errors
    /// Returns an invalid namespace or out-of-width value error.
    pub fn try_new_with_unsigned(
        namespace: impl Into<String>,
        width: IntegerWidth,
        value: u64,
    ) -> Result<Self, MediaError> {
        let namespace = namespace.into();
        validate_namespace(&namespace)?;
        let maximum = match width {
            IntegerWidth::Bits8 => u64::from(u8::MAX),
            IntegerWidth::Bits16 => u64::from(u16::MAX),
            IntegerWidth::Bits32 => u64::from(u32::MAX),
            IntegerWidth::Bits64 => u64::MAX,
        };
        if value > maximum {
            return Err(MediaError::Contract(
                "identifier exceeds its unsigned source width".into(),
            ));
        }
        Ok(Self(IdentifierValue::Unsigned {
            namespace,
            width,
            value: value.to_string(),
        }))
    }

    /// Retains a signed value as exact decimal text with its source width.
    ///
    /// # Errors
    /// Returns an invalid namespace or out-of-width value error.
    pub fn try_new_with_signed(
        namespace: impl Into<String>,
        width: IntegerWidth,
        value: i64,
    ) -> Result<Self, MediaError> {
        let namespace = namespace.into();
        validate_namespace(&namespace)?;
        let (minimum, maximum) = match width {
            IntegerWidth::Bits8 => (i64::from(i8::MIN), i64::from(i8::MAX)),
            IntegerWidth::Bits16 => (i64::from(i16::MIN), i64::from(i16::MAX)),
            IntegerWidth::Bits32 => (i64::from(i32::MIN), i64::from(i32::MAX)),
            IntegerWidth::Bits64 => (i64::MIN, i64::MAX),
        };
        if !(minimum..=maximum).contains(&value) {
            return Err(MediaError::Contract(
                "identifier exceeds its signed source width".into(),
            ));
        }
        Ok(Self(IdentifierValue::Signed {
            namespace,
            width,
            value: value.to_string(),
        }))
    }
}

fn validate_namespace(value: &str) -> Result<(), MediaError> {
    if !crate::media_record::namespaced(value) {
        return Err(MediaError::Contract(
            "identifier needs a namespaced source domain".into(),
        ));
    }
    Ok(())
}
