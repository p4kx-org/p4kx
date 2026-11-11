// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Guid;

use super::{Bounds, CHILD, CHILDREN, Child, Level, Pose};
use crate::CatalogError;
use crate::catalog_read::original::{AuthoredEntryPath, EntryXml};
use crate::starmap_report::clip;
use crate::starmap_xml::{Node, Root, attribute, first_below, pose};

/// Reads the existing first Child list with the actual parse receiving external-path selections.
pub(super) fn level(
    node: Root<'_>,
    depth: usize,
    bounds: &Bounds,
    xml: &EntryXml<'_, '_, '_>,
) -> Result<Level, CatalogError> {
    let Some(list) = first_below(node, CHILDREN) else {
        return Ok(Level::default());
    };
    let extra_lists = node
        .children()
        .filter(|sibling| sibling.name().eq_ignore_ascii_case(CHILDREN))
        .nth(1)
        .is_some();
    if depth >= bounds.depth {
        return Ok(Level {
            capped: list
                .children()
                .any(|child| child.name().eq_ignore_ascii_case(CHILD)),
            extra_lists,
            ..Level::default()
        });
    }
    let mut children = Vec::new();
    let mut truncated = false;
    for child in list.children() {
        let name = xml.name(child)?;
        if !name.as_str().eq_ignore_ascii_case(CHILD) {
            continue;
        }
        if !bounds.take() {
            truncated = true;
            break;
        }
        children.push(read_child(child, &name, depth, bounds, xml)?);
    }
    Ok(Level {
        children,
        capped: false,
        extra_lists,
        truncated,
    })
}

fn read_child(
    child: Root<'_>,
    name: &AuthoredEntryPath,
    depth: usize,
    bounds: &Bounds,
    xml: &EntryXml<'_, '_, '_>,
) -> Result<Child, CatalogError> {
    let record = attribute(child, "starMapRecord")
        .and_then(|text| text.trim().parse::<Guid>().ok())
        .filter(|guid| guid.stored() != [0; 16]);
    let external_recipe = external(child, name, xml)?;
    let external = external_recipe
        .as_ref()
        .map(|path| path.as_str().to_owned());
    let pose = child_pose(child);
    let below = level(child, depth + 1, bounds, xml)?;
    let named = record.is_some() || external.is_some() || !below.children.is_empty();
    Ok(Child {
        pose,
        record,
        external,
        external_recipe,
        below,
        entity_name: if named {
            attribute(child, "entityName")
                .unwrap_or_default()
                .to_owned()
        } else {
            String::new()
        },
    })
}

fn external(
    child: Root<'_>,
    node_name: &AuthoredEntryPath,
    xml: &EntryXml<'_, '_, '_>,
) -> Result<Option<AuthoredEntryPath>, CatalogError> {
    let Some(external) = xml.attribute(child, "external")? else {
        return Ok(None);
    };
    if external.as_str() != "1" {
        return Ok(None);
    }
    let Some(path) = xml.attribute(child, "name")? else {
        return Ok(None);
    };
    if path.as_str().is_empty() {
        return Ok(None);
    }
    Ok(Some(path.selected_by(&external)?.selected_by(node_name)?))
}

fn child_pose(child: Root<'_>) -> Pose {
    match pose(child) {
        Some((position, rotation)) => Pose::Valid { position, rotation },
        None => Pose::Malformed {
            pos: clip(attribute(child, "pos").unwrap_or_default()),
            rot: clip(attribute(child, "rot").unwrap_or_default()),
            entity: clip(attribute(child, "entityName").unwrap_or_default()),
        },
    }
}
