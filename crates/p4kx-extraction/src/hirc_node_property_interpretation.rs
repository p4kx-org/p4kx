// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_audio::{BankHircField, BankNodePropertyRow, BankNodePropertyValue};
use schemars::JsonSchema;
use serde::Serialize;
use std::ops::Range;

use crate::bank::{VoiceProperty, VoicePropertyName};
use crate::source_evidence_error::SourceEvidenceError;
use crate::source_value::SourcedValue;

use super::super::source_fields::origins::ObjectOrigins;

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(in crate::hirc_view) struct PropertyInterpretation {
    property: SourcedValue<VoicePropertyName>,
    #[serde(skip_serializing_if = "Option::is_none")]
    scalar: Option<SourcedValue<Option<f32>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    minimum: Option<SourcedValue<Option<f32>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    maximum: Option<SourcedValue<Option<f32>>>,
}

impl PropertyInterpretation {
    pub(in crate::hirc_view) fn read(
        origins: &ObjectOrigins,
        version: u32,
        version_range: &Range<u64>,
        original: &BankNodePropertyRow,
    ) -> Result<Option<Self>, SourceEvidenceError> {
        let Some(property) = VoiceProperty::from_node_property(version, original.id.value) else {
            return Ok(None);
        };
        let Some(name) = VoicePropertyName::from_property(property) else {
            return Ok(None);
        };
        let name = origins.property_type(name, version_range, &original.id.range)?;
        let interpret = |word: Option<&BankHircField<u32>>| {
            word.map(|word| {
                origins.property_value(
                    property.finite_value(word.value),
                    version_range,
                    &original.id.range,
                    &word.range,
                )
            })
            .transpose()
        };
        let (scalar, minimum, maximum) = match &original.value {
            BankNodePropertyValue::Scalar(bits) => (interpret(bits.as_ref())?, None, None),
            BankNodePropertyValue::Ranged {
                minimum, maximum, ..
            } => (
                None,
                interpret(minimum.as_ref())?,
                interpret(maximum.as_ref())?,
            ),
            _ => return Ok(None),
        };
        Ok(Some(Self {
            property: name,
            scalar,
            minimum,
            maximum,
        }))
    }
}
