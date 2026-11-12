// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_assembly::is_external_child_value;
use p4kx_extraction::Guid;

use super::{Bounds, CHILD, CHILDREN, Child, Level, Pose};
use crate::CatalogError;
use crate::catalog_read::original::{AuthoredEntryPath, EntryXml, XmlNodeSource};
use crate::starmap_containers::proof::ChildOrigins;
use crate::starmap_report::clip;
use crate::starmap_xml::{Node, Root, attribute, first_below, pose};

/// Reads direct Child lists in source order, with the legacy wrapped-list fallback.
pub(super) fn level(
    node: Root<'_>,
    depth: usize,
    bounds: &Bounds,
    xml: &EntryXml<'_, '_, '_>,
) -> Result<Level, CatalogError> {
    let mut direct = node
        .children()
        .filter(|section| section.name().eq_ignore_ascii_case(CHILDREN))
        .peekable();
    let fallback = if direct.peek().is_none() {
        first_below(node, CHILDREN)
    } else {
        None
    };
    let mut lists = direct.chain(fallback);
    let mut level = Level::default();
    if depth >= bounds.depth {
        level.capped = lists.any(|list| {
            list.children()
                .any(|child| child.name().eq_ignore_ascii_case(CHILD))
        });
        return Ok(level);
    }
    for list in lists {
        let list_source = xml.node_source(list)?;
        for child in list.children() {
            let name = xml.name(child)?;
            if !name.as_str().eq_ignore_ascii_case(CHILD) {
                continue;
            }
            if !bounds.take() {
                level.truncated = true;
                return Ok(level);
            }
            level
                .children
                .push(read_child(child, &name, &list_source, depth, bounds, xml)?);
        }
    }
    Ok(level)
}

fn read_child(
    child: Root<'_>,
    name: &AuthoredEntryPath,
    list: &XmlNodeSource,
    depth: usize,
    bounds: &Bounds,
    xml: &EntryXml<'_, '_, '_>,
) -> Result<Child, CatalogError> {
    let record = attribute(child, "starMapRecord")
        .and_then(|text| text.trim().parse::<Guid>().ok())
        .filter(|guid| guid.stored() != [0; 16]);
    let external = external(child, name, xml)?;
    let pose = child_pose(child);
    let below = level(child, depth + 1, bounds, xml)?;
    let named = record.is_some() || external.is_some() || !below.children.is_empty();
    let origins = named
        .then(|| ChildOrigins::capture(xml, list, child).map(Arc::new))
        .transpose()?;
    Ok(Child {
        pose,
        record,
        external,
        origins,
        below,
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
    if !is_external_child_value(external.as_str()) {
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
    pose(child).map_or_else(
        || Pose::Malformed {
            pos: clip(attribute(child, "pos").unwrap_or_default()),
            rot: clip(attribute(child, "rot").unwrap_or_default()),
            entity: clip(attribute(child, "entityName").unwrap_or_default()),
        },
        Pose::Valid,
    )
}
