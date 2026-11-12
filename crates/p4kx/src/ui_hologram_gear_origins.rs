// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use anyhow::Result;
use p4kx_assembly::{GearPlacementSource, Part};
use p4kx_extraction::SourceUse;

use super::Projection;

pub(super) fn model(projection: &Projection, part: &Part, uses: &mut Vec<SourceUse>) -> Result<()> {
    let Some(gear) = part.landing_gear_source() else {
        return Ok(());
    };
    for field in gear.model_selectors() {
        projection.authored_selector(Some(field), uses)?;
    }
    Ok(())
}

pub(super) fn placement(
    projection: &Projection,
    part: &Part,
    uses: &mut Vec<SourceUse>,
) -> Result<()> {
    model(projection, part, uses)?;
    projection.authored_selector(
        part.landing_gear_source()
            .and_then(GearPlacementSource::bone_selector),
        uses,
    )
}
