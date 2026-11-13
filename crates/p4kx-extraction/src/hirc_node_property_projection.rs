// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_audio::{BankNodePropertyExtent, BankNodePropertyRow, BankNodePropertyValue};
use schemars::JsonSchema;
use serde::Serialize;

use crate::source_evidence_error::SourceEvidenceError;
use crate::source_value::SourcedValue;

#[path = "hirc_node_property_interpretation.rs"]
mod interpretation;

pub(super) use self::interpretation::PropertyInterpretation;
use super::source_fields::origins::ObjectOrigins;

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "NodePropertyCoverage")]
pub(super) struct PropertyExtent {
    pub(super) declared_count: SourcedValue<u8>,
    pub(super) checked_ids: SourcedValue<u8>,
    pub(super) checked_words: SourcedValue<u16>,
    pub(super) complete: SourcedValue<bool>,
}

impl PropertyExtent {
    pub(super) fn read(
        origins: &ObjectOrigins,
        extent: &BankNodePropertyExtent,
    ) -> Result<Self, SourceEvidenceError> {
        let count = &extent.declared_count;
        let ids = count.range.start..extent.ids_range.end;
        let words_end = extent
            .words_range
            .as_ref()
            .map_or(ids.end, |words| words.end);
        let inspected = count.range.start..words_end;
        Ok(Self {
            declared_count: origins.field(count, "nodePropertyCount")?,
            checked_ids: origins.derived(
                extent.checked_id_count(),
                &ids,
                "nodePropertyCheckedIds",
            )?,
            checked_words: origins.derived(
                extent.checked_word_count(),
                &inspected,
                "nodePropertyCheckedWords",
            )?,
            complete: origins.derived(extent.is_complete(), &inspected, "nodePropertyCoverage")?,
        })
    }
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "NodePropertyWords")]
pub(super) struct PropertyWords {
    pub(super) ordinal: SourcedValue<u8>,
    pub(super) property_id: SourcedValue<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) scalar_bits: Option<SourcedValue<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) minimum_bits: Option<SourcedValue<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) maximum_bits: Option<SourcedValue<u32>>,
}

impl PropertyWords {
    pub(super) fn read<E: serde::ser::Error>(
        origins: &ObjectOrigins,
        row: &BankNodePropertyRow,
    ) -> Result<Self, E> {
        let (scalar, minimum, maximum) = match &row.value {
            BankNodePropertyValue::Scalar(bits) => (bits.as_ref(), None, None),
            BankNodePropertyValue::Ranged {
                minimum, maximum, ..
            } => (None, minimum.as_ref(), maximum.as_ref()),
            _ => return Err(E::custom("unexpected original property layout")),
        };
        Ok(Self {
            ordinal: origins
                .derived(row.ordinal, &row.id.range, "nodePropertyPosition")
                .map_err(E::custom)?,
            property_id: origins
                .field(&row.id, "nodePropertyId")
                .map_err(E::custom)?,
            scalar_bits: origins
                .optional(scalar, "nodePropertyScalarBits")
                .map_err(E::custom)?,
            minimum_bits: origins
                .optional(minimum, "nodePropertyMinimumBits")
                .map_err(E::custom)?,
            maximum_bits: origins
                .optional(maximum, "nodePropertyMaximumBits")
                .map_err(E::custom)?,
        })
    }
}
