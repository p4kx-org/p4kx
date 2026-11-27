// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use anyhow::Result;
use p4kx_assembly::{Assembly, BonePlacementSource};
use p4kx_extraction::{
    InputScope, SourceInput, SourceLocation, SourceScope, SourceUse, SourceUseRole,
};

use super::{Projection, cdf_origins};

pub(super) fn placement(
    projection: &Projection,
    assembly: &Assembly,
    part: usize,
    uses: &mut Vec<SourceUse>,
) -> Result<()> {
    let Some(source) = assembly.bone_placement_source(part) else {
        return Ok(());
    };
    record(projection, &source, uses)
}

pub(super) fn record(
    projection: &Projection,
    source: &BonePlacementSource<'_>,
    uses: &mut Vec<SourceUse>,
) -> Result<()> {
    projection.authored_selector(source.geometry_selector(), uses)?;
    if let Some(definition) = source.definition_origin() {
        uses.push(projection.opaque_archive(definition, SourceUseRole::Selector)?);
        cdf_origins::model_selectors(
            projection,
            Some(definition),
            source.cdf_model_selectors(),
            uses,
        )?;
    }
    let scope = projection
        .uses()
        .observe(SourceInput::Archive(source.origin()), InputScope::Payload)?;
    for (step, (ordinal, bone)) in source.bones().enumerate() {
        let coordinates = (u64::try_from(source.chunk())?, u64::try_from(ordinal)?);
        if step == 0 {
            uses.push(at(
                &scope,
                coordinates,
                "name",
                bone.name(),
                SourceUseRole::Selector,
            )?);
        }
        uses.push(at(
            &scope,
            coordinates,
            "parent",
            bone.parent(),
            SourceUseRole::Selector,
        )?);
        uses.push(at(
            &scope,
            coordinates,
            "relativePose",
            bone.relative_pose(),
            SourceUseRole::Conversion,
        )?);
    }
    Ok(())
}

fn at(
    scope: &SourceScope,
    (chunk, bone): (u64, u64),
    field: &str,
    range: Range<u64>,
    role: SourceUseRole,
) -> Result<SourceUse> {
    Ok(scope.at(
        SourceLocation::Subresource {
            namespace: "modelBonePlacementBytes".into(),
            indices: vec![chunk, bone, range.start, range.end],
            field: field.into(),
        },
        role,
    )?)
}
