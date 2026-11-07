// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::StructValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::mission_property_source;
use crate::record_read::ReadError;
use crate::source_fields::Fields;

/// One authored `TimeTrialSplit`, without computing a race result.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = crate::contract::every_field_required)]
pub(crate) struct TimeTrialSplit {
    /// The finite source `Single` target split, without changing its unit.
    pub(crate) target_split: Option<f32>,
}

pub(crate) fn read(
    source: &StructValue<'_>,
) -> Result<Option<Vec<Option<TimeTrialSplit>>>, ReadError> {
    mission_property_source::check(source, "", &["targetSplits"])?;
    mission_property_source::list(source, "targetSplits", "", |split, path| {
        mission_property_source::check(split, path, &["targetSplit"])?;
        Ok(TimeTrialSplit {
            target_split: Fields::new(split, path).single("targetSplit")?,
        })
    })
}
