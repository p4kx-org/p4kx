// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourceLocation;
use p4kx_extraction::cryxml::{SourceElement, SourceElementPosition};
use p4kx_extraction::model::SocEntityFile;

use super::super::{LocationPhysicsFields, LocationPhysicsGrid, LocationPhysicsGridControls};
use super::TestResult;

const GRID: &str = "PhysicsGrid";
const PART: &str = "PhysicsGridPart";
const VOXEL: &str = "PhysicsGridVoxel";
const CHILD_PARTS: &str = "PhysicsGridChildParts";

pub(super) fn check_grid(
    root: SourceElement<'_>,
    projected: Option<&LocationPhysicsGrid>,
    expected: bool,
) -> TestResult {
    let radius = root
        .attribute("gridRadius")
        .map(str::parse::<f64>)
        .transpose()?;
    let children = root
        .elements()
        .enumerate()
        .filter(|(_, node)| matches!(node.name(), GRID | PART));
    let originals = declarations(root, children);
    if (radius.is_some() || !originals.is_empty()) != expected {
        return Err("original XML does not have the audited declaration state".into());
    }
    let Some(projected) = projected else {
        return if expected {
            Err("old XML lacks its SDK projection".into())
        } else {
            Ok(())
        };
    };
    if !expected
        || projected.grid_radius.value().copied() != radius
        || projected.declarations.len() != originals.len()
    {
        return Err("SDK grid radius or declaration count differs from the original XML".into());
    }
    for ((parent, index, node), row) in originals.into_iter().zip(&projected.declarations) {
        if !position_matches(row.position.value(), node)
            || !position_matches(row.owner.value(), parent)
            || row.sibling_ordinal.value().copied() != Some(u64::try_from(index)?)
            || !fields_match(&row.fields, node)?
        {
            return Err("SDK declarations differ from the original XML positions or fields".into());
        }
    }
    Ok(())
}

fn position_matches(claim: Option<&SourceLocation>, node: SourceElement<'_>) -> bool {
    let Some(SourceLocation::Subresource {
        namespace,
        indices,
        field,
    }) = claim
    else {
        return false;
    };
    if field != "$node" {
        return false;
    }
    match (node.position(), indices.as_slice()) {
        (SourceElementPosition::Text(range), [start, end]) => {
            namespace == "xmlSource.text"
                && u64::try_from(range.start).ok() == Some(*start)
                && u64::try_from(range.end).ok() == Some(*end)
        }
        (SourceElementPosition::CryXml(index), [position]) => {
            namespace == "xmlSource.cryXml" && u64::from(index) == *position
        }
        _ => false,
    }
}

fn declarations<'source>(
    root: SourceElement<'source>,
    children: impl Iterator<Item = (usize, SourceElement<'source>)>,
) -> Vec<(SourceElement<'source>, usize, SourceElement<'source>)> {
    let mut pending: Vec<_> = children.map(|(index, node)| (root, index, node)).collect();
    pending.reverse();
    let mut result = Vec::new();
    while let Some(item @ (_, _, node)) = pending.pop() {
        let start = pending.len();
        pending.extend(node.elements().enumerate().filter_map(|(i, child)| {
            matches!(child.name(), GRID | PART | VOXEL | CHILD_PARTS).then_some((node, i, child))
        }));
        if let Some(added) = pending.get_mut(start..) {
            added.reverse();
        }
        result.push(item);
    }
    result
}

fn fields_match(fields: &LocationPhysicsFields, original: SourceElement<'_>) -> TestResult<bool> {
    Ok(match (fields, original.name()) {
        (LocationPhysicsFields::Grid { controls }, GRID) => controls_match(controls, original)?,
        (LocationPhysicsFields::Part { shape }, PART) => {
            shape.value().map(String::as_str) == original.attribute("shape")
        }
        (LocationPhysicsFields::Voxel { filename, .. }, VOXEL) => {
            filename.value().map(String::as_str) == original.attribute("filename")
        }
        (LocationPhysicsFields::ChildParts, CHILD_PARTS) => true,
        _ => false,
    })
}

fn controls_match(
    controls: &LocationPhysicsGridControls,
    node: SourceElement<'_>,
) -> TestResult<bool> {
    let gravity = node
        .attribute("gravity")
        .map(original_gravity)
        .transpose()?;
    let cell_size = node
        .attribute("cellSize")
        .map(str::parse::<f64>)
        .transpose()?;
    let portal = node
        .attribute("portalExclusiveMode")
        .map(str::parse::<i64>)
        .transpose()?;
    Ok(controls.gravity.value().copied() == gravity
        && controls.cell_size.value().copied() == cell_size
        && controls.grid_type.value().map(String::as_str) == node.attribute("gridType")
        && controls.portal_exclusive_mode.value().copied() == portal
        && control_flag(node, "inheritGravity")? == controls.inherit_gravity.value().copied()
        && control_flag(node, "hasPart")? == controls.has_part.value().copied())
}

fn original_gravity(text: &str) -> TestResult<[f64; 3]> {
    let fields: Vec<f64> = text
        .split(',')
        .map(|field| field.trim().parse())
        .collect::<Result<_, _>>()?;
    let [x, y, z] = fields.as_slice() else {
        return Err("original gravity does not have exactly three components".into());
    };
    if fields.iter().any(|value| !value.is_finite()) {
        return Err("original gravity has a nonfinite component".into());
    }
    Ok([*x, *y, *z])
}

fn control_flag(node: SourceElement<'_>, name: &str) -> TestResult<Option<bool>> {
    match node.attribute(name) {
        None => Ok(None),
        Some("1" | "true") => Ok(Some(true)),
        Some("0" | "false") => Ok(Some(false)),
        Some(_) => Err("original XML control does not have its audited boolean form".into()),
    }
}

pub(super) fn check_cargo(bytes: &[u8], original_chunk: usize, target: &str) -> TestResult {
    let soc = SocEntityFile::parse(bytes).map_err(|_| "audited cargo SOC cannot be parsed")?;
    for chunk in soc.entity_chunks() {
        let chunk = chunk.map_err(|_| "audited cargo chunk cannot be parsed")?;
        if chunk.chunk_index() != original_chunk {
            continue;
        }
        let mut grids = chunk
            .entities()
            .flat_map(|entity| entity.element().elements())
            .filter(|manager| manager.name() == "SCLoadingPlatformManager")
            .flat_map(SourceElement::elements)
            .filter(|element| element.name() == "CargoGrid");
        let grid = grids
            .next()
            .ok_or("separate cargo grid is missing from the original chunk")?;
        if chunk.document().is_lossy()
            || grids.next().is_some()
            || grid.attribute("entitySuperGUID") != Some(target)
        {
            return Err("separate cargo relationship differs from the audited source".into());
        }
        return Ok(());
    }
    Err("audited cargo chunk is missing".into())
}
