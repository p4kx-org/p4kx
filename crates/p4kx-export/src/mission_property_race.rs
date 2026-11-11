// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::StructValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::mission_property_source;
use crate::record_read::ReadError;
use crate::source_fields::{FieldCapture, FieldChecks};

/// One authored `TimeTrialSplit`, without computing a race result.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = crate::contract::every_field_required)]
pub(crate) struct TimeTrialSplit {
    /// The finite source `Single` target split, without changing its unit.
    pub(crate) target_split: Option<f32>,
}

pub(crate) fn read<'source>(
    source: &StructValue<'source>,
    capture: Option<&dyn FieldCapture<'source>>,
) -> Result<Option<Vec<Option<TimeTrialSplit>>>, ReadError> {
    check(source, capture)?;
    mission_property_source::list(source, "targetSplits", "", capture, |split, path| {
        Ok(TimeTrialSplit {
            target_split: split_value(split, path, capture)?,
        })
    })
}

/// Checks nullable split slots and exact finite Single payloads without a DTO list.
#[cfg(test)]
pub(crate) fn validate(source: &StructValue<'_>) -> Result<(), ReadError> {
    validate_context(source, None)
}

/// Checks the same split source with its admitted original guard receiver.
pub(crate) fn validate_observed<'source>(
    source: &StructValue<'source>,
    capture: &dyn FieldCapture<'source>,
) -> Result<(), ReadError> {
    validate_context(source, Some(capture))
}

fn validate_context<'source>(
    source: &StructValue<'source>,
    capture: Option<&dyn FieldCapture<'source>>,
) -> Result<(), ReadError> {
    check(source, capture)?;
    mission_property_source::visit_context(source, "targetSplits", "", capture, |split, path| {
        if let Some(split) = split {
            split_value(split, path, capture)?;
        }
        Ok(())
    })?;
    Ok(())
}

fn split_value<'source>(
    source: &StructValue<'source>,
    path: &str,
    capture: Option<&dyn FieldCapture<'source>>,
) -> Result<Option<f32>, ReadError> {
    let fields = FieldChecks::for_context(source, path, capture)?;
    fields.check(&["targetSplit"])?;
    fields.single("targetSplit")
}

fn check<'source>(
    source: &StructValue<'source>,
    capture: Option<&dyn FieldCapture<'source>>,
) -> Result<(), ReadError> {
    FieldChecks::for_context(source, "", capture)?.check(&["targetSplits"])
}
