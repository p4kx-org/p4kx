// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ptr;

use p4kx_dataforge::{DataForge, StructValue, Value};
use p4kx_extraction::{
    FieldOrigin, RecordJsonLocation, SourceEvidenceError, SourceLocation, SourceScope,
    SourceUseRole, SourceUses,
};

/// A Loadout text captured at the actual Record getter.
#[derive(Debug)]
pub struct LoadoutRecordText {
    original: Option<SourceLocation>,
}

impl LoadoutRecordText {
    /// Captures the accepted value in its original parent before the getter copies it.
    #[must_use]
    pub fn capture(
        forge: &DataForge<'_>,
        parent: &StructValue<'_>,
        value: &Value<'_>,
    ) -> Option<Self> {
        if !parent
            .fields()
            .iter()
            .any(|field| ptr::eq(field.value(), value))
        {
            return None;
        }
        let original = match value {
            Value::String(text) | Value::Locale(text) | Value::Enum(text) => {
                RecordJsonLocation::borrowed_text(forge, text)
            }
            _ => None,
        };
        Some(Self { original })
    }

    /// Registers the original owned-value position and its retained text bytes.
    ///
    /// The caller supplies the position from the same Record tree as the getter.
    ///
    /// # Errors
    /// Returns an error if source registration fails.
    pub fn origin(
        &self,
        uses: &SourceUses,
        scope: &SourceScope,
        record_index: u32,
        ordinal: u64,
        role: SourceUseRole,
        complete: bool,
    ) -> Result<FieldOrigin, SourceEvidenceError> {
        let selected = scope.at(
            SourceLocation::Subresource {
                namespace: "dataforgeValue".into(),
                indices: vec![u64::from(record_index), ordinal],
                field: "$value".into(),
            },
            SourceUseRole::Selector,
        )?;
        Ok(if let Some(original) = &self.original {
            let text = scope.at(original.clone(), role)?;
            uses.origin(&[selected, text], complete)
        } else {
            uses.origin(&[selected], false)
        })
    }
}
