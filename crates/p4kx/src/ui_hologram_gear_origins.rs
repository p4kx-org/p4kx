// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use anyhow::Result;
use p4kx_assembly::{GearAnimationKey, GearPlacementSource, GearPoseSources, Part};
use p4kx_extraction::{
    InputScope, SourceInput, SourceLocation, SourceScope, SourceUse, SourceUseRole,
};

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

pub(super) fn pose(
    projection: &Projection,
    part: &Part,
    keys: impl Iterator<Item = GearAnimationKey>,
    uses: &mut Vec<SourceUse>,
) -> Result<()> {
    let Some(pose) = part.gear_pose_input() else {
        return Ok(());
    };
    let keys: BTreeSet<_> = keys.collect();
    if keys.is_empty() {
        return Ok(());
    }
    let source = pose.source();
    for (origin, location) in source.selectors() {
        let scope = projection
            .uses()
            .observe(SourceInput::Archive(origin), InputScope::Payload)?;
        uses.push(scope.at(location.clone(), SourceUseRole::Selector)?);
    }
    record_clip(projection, source, keys, uses)
}

fn record_clip(
    projection: &Projection,
    source: &GearPoseSources,
    keys: BTreeSet<GearAnimationKey>,
    uses: &mut Vec<SourceUse>,
) -> Result<()> {
    let Some(origin) = source.clip() else {
        return Ok(());
    };
    let scope = projection
        .uses()
        .observe(SourceInput::Archive(origin), InputScope::Payload)?;
    let clip = u64::try_from(source.clip_ordinal())?;
    uses.push(at(
        &scope,
        "gearDeployClip",
        vec![clip],
        "additive",
        SourceUseRole::Selector,
    )?);
    if origin
        .entry()
        .path()
        .rsplit_once('.')
        .is_some_and(|(_, extension)| extension.eq_ignore_ascii_case("dba"))
    {
        uses.push(at(
            &scope,
            "gearDeployClip",
            vec![clip],
            "name",
            SourceUseRole::Selector,
        )?);
    }
    let mut controllers = BTreeSet::new();
    for key in keys {
        let controller = u64::try_from(key.controller())?;
        if controllers.insert(controller) {
            uses.push(at(
                &scope,
                "gearDeployController",
                vec![clip, controller],
                "controllerId",
                SourceUseRole::Selector,
            )?);
        }
        uses.push(at(
            &scope,
            "gearDeployKey",
            vec![clip, controller, u64::try_from(key.ordinal())?],
            key.channel().field(),
            SourceUseRole::Conversion,
        )?);
    }
    Ok(())
}

fn at(
    scope: &SourceScope,
    namespace: &str,
    indices: Vec<u64>,
    field: &str,
    role: SourceUseRole,
) -> Result<SourceUse> {
    Ok(scope.at(
        SourceLocation::Subresource {
            namespace: namespace.into(),
            indices,
            field: field.into(),
        },
        role,
    )?)
}
