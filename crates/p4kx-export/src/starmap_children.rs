// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;
use std::sync::Arc;

use p4kx_assembly::is_external_child_value;
use p4kx_extraction::{Guid, SourceUses};

use super::entxml::EntXmlIndex;
use super::{Bounds, CHILD, CHILDREN, Child, Level, Pose, hcd::HcdReader};
use crate::catalog_read::original::{AuthoredEntryPath, EntryXml, XmlNodeSource};
use crate::export_ledger::ExportLedger;
use crate::starmap_containers::location_declaration::ChildLocation;
use crate::starmap_containers::proof::ChildOrigins;
use crate::starmap_report::clip;
use crate::starmap_xml::{Node, Root, attribute, first_below, pose};
use crate::{CatalogError, ExportAccountingError};

pub(super) struct ChildReader<'a, 'read, 'capture, 'input, 'archive, 'cancel> {
    bounds: &'a Bounds<'cancel>,
    xml: &'a EntryXml<'read, 'capture, 'input>,
    hcd: &'a mut HcdReader<'archive, 'capture, 'input>,
    ledger: &'a mut ExportLedger,
    pub(super) entity_guids: HashSet<Guid>,
}

impl<'a, 'read, 'capture, 'input, 'archive, 'cancel>
    ChildReader<'a, 'read, 'capture, 'input, 'archive, 'cancel>
{
    pub(super) fn new(
        bounds: &'a Bounds<'cancel>,
        xml: &'a EntryXml<'read, 'capture, 'input>,
        hcd: &'a mut HcdReader<'archive, 'capture, 'input>,
        ledger: &'a mut ExportLedger,
    ) -> Self {
        Self {
            bounds,
            xml,
            hcd,
            ledger,
            entity_guids: HashSet::new(),
        }
    }

    /// Reads direct Child lists in source order, with the legacy wrapped-list fallback.
    pub(super) fn level(&mut self, node: Root<'_>, depth: usize) -> Result<Level, CatalogError> {
        let xml = self.xml;
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
        if depth >= self.bounds.depth {
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
                if !self.bounds.take() {
                    level.truncated = true;
                    return Ok(level);
                }
                level
                    .children
                    .push(self.read_child(child, &name, &list_source, depth)?);
            }
        }
        Ok(level)
    }

    fn read_child(
        &mut self,
        child: Root<'_>,
        name: &AuthoredEntryPath,
        list: &XmlNodeSource,
        depth: usize,
    ) -> Result<Child, CatalogError> {
        let xml = self.xml;
        let record = attribute(child, "starMapRecord")
            .and_then(|text| text.trim().parse::<Guid>().ok())
            .filter(|guid| guid.stored() != [0; 16]);
        let external = external(child, name, xml)?;
        let pose = child_pose(child);
        let below = self.level(child, depth + 1)?;
        let mut location = ChildLocation::read(xml, child, xml.capture())?;
        if let Some(location) = &mut location {
            location.read_context(self.hcd, xml, child, self.ledger)?;
            self.entity_guids
                .extend(location.request_entity(xml, child)?);
        }
        let named = record.is_some()
            || external.is_some()
            || !below.children.is_empty()
            || location.is_some();
        let origins = named
            .then(|| ChildOrigins::capture(xml, list, child).map(Arc::new))
            .transpose()?;
        Ok(Child {
            pose,
            record,
            external,
            origins,
            location: location.map(Arc::new),
            below,
        })
    }
}

pub(super) struct SourceLoss {
    pub(super) projection: bool,
    pub(super) inventory: bool,
}

pub(super) fn source_loss(
    level: &mut Level,
    entities: &EntXmlIndex,
    uses: &SourceUses,
) -> Result<SourceLoss, CatalogError> {
    let inventory = level.capped || level.truncated;
    let mut loss = SourceLoss {
        projection: inventory,
        inventory,
    };
    for child in &mut level.children {
        if let Some(location) = &mut child.location {
            Arc::get_mut(location)
                .ok_or(ExportAccountingError::ConflictingSourceDatum)?
                .attach_entity(entities, uses);
        }
        let below = source_loss(&mut child.below, entities, uses)?;
        loss.projection |= matches!(&child.pose, Pose::Malformed { .. }) || below.projection;
        loss.inventory |= below.inventory;
    }
    Ok(loss)
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
