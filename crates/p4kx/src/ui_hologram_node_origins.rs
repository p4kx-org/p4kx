// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use anyhow::Result;
use p4kx_assembly::Assembly;
use p4kx_extraction::{
    InputScope, SourceInput, SourceLocation, SourceScope, SourceUse, SourceUseRole,
};

use super::Projection;

pub(super) fn placement(
    projection: &Projection,
    assembly: &Assembly,
    part: usize,
    uses: &mut Vec<SourceUse>,
) -> Result<()> {
    for source in assembly.model_placement_inputs(part) {
        projection.authored_selector(source.geometry_selector(), uses)?;
        if let Some(definition) = source.definition_origin() {
            uses.push(projection.opaque_archive(definition, SourceUseRole::Selector)?);
        }
        let scope = projection
            .uses()
            .observe(SourceInput::Archive(source.origin()), InputScope::Payload)?;
        for (step, (ordinal, node)) in source.nodes().enumerate() {
            let selected = if step == 0 {
                node.name().map(|name| ("name", name))
            } else {
                node.id().map(|id| ("chunkId", id))
            };
            let coordinates = (u64::try_from(node.chunk())?, u64::try_from(ordinal)?);
            if let Some((field, range)) = selected {
                uses.push(at(
                    &scope,
                    coordinates,
                    field,
                    range,
                    SourceUseRole::Selector,
                )?);
            }
            uses.push(at(
                &scope,
                coordinates,
                "parent",
                node.parent(),
                SourceUseRole::Selector,
            )?);
            uses.push(at(
                &scope,
                coordinates,
                "localTransform",
                node.matrix(),
                SourceUseRole::Conversion,
            )?);
        }
    }
    Ok(())
}

fn at(
    scope: &SourceScope,
    (chunk, node): (u64, u64),
    field: &str,
    range: Range<u64>,
    role: SourceUseRole,
) -> Result<SourceUse> {
    Ok(scope.at(
        SourceLocation::Subresource {
            namespace: "modelNodePlacementBytes".into(),
            indices: vec![chunk, node, range.start, range.end],
            field: field.into(),
        },
        role,
    )?)
}
