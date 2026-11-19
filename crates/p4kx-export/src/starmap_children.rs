// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;
use std::mem;
use std::sync::Arc;

use p4kx_assembly::is_external_child_value;
use p4kx_extraction::{CancelToken, Guid, SourceUses};

use super::entxml::EntXmlIndex;
use super::{Bounds, CHILD, CHILDREN, Child, Level, Pose, hcd::HcdReader};
use crate::catalog_read::original::{AuthoredEntryPath, EntryXml, XmlNodeSource};
use crate::export_ledger::ExportLedger;
use crate::starmap_containers::location_declaration::ChildLocation;
use crate::starmap_containers::proof::ChildOrigins;
use crate::starmap_containers::unresolved::{DeferredChild, UnresolvedChild};
use crate::starmap_index::Index;
use crate::starmap_report::clip;
use crate::starmap_xml::{Node, Root, attribute, first_below, pose};
use crate::{CatalogError, ExportAccountingError};

pub(super) struct ChildReader<'a, 'read, 'capture, 'input, 'archive, 'cancel> {
    bounds: &'a Bounds<'cancel>,
    xml: &'a EntryXml<'read, 'capture, 'input>,
    hcd: &'a mut HcdReader<'archive, 'capture, 'input>,
    index: &'a Index<'archive>,
    ledger: &'a mut ExportLedger,
    pub(super) entity_guids: HashSet<Guid>,
    pub(super) pending_external: bool,
}

impl<'a, 'read, 'capture, 'input, 'archive, 'cancel>
    ChildReader<'a, 'read, 'capture, 'input, 'archive, 'cancel>
{
    pub(super) fn new(
        bounds: &'a Bounds<'cancel>,
        xml: &'a EntryXml<'read, 'capture, 'input>,
        hcd: &'a mut HcdReader<'archive, 'capture, 'input>,
        index: &'a Index<'archive>,
        ledger: &'a mut ExportLedger,
    ) -> Self {
        Self {
            bounds,
            xml,
            hcd,
            index,
            ledger,
            entity_guids: HashSet::new(),
            pending_external: false,
        }
    }

    /// Reads direct Child lists in source order, with the legacy wrapped-list fallback.
    pub(super) fn level<'node>(&mut self, node: Root<'node>) -> Result<Level, CatalogError> {
        let xml = self.xml;
        let elements = |list: Root<'node>| list.children();
        let mut level = ReadLevel::new(child_lists(node));
        let mut parents = Vec::new();
        loop {
            if self.bounds.cancel.is_some_and(CancelToken::is_cancelled) {
                return Err(CatalogError::Cancelled);
            }
            if let Some(node) = level.advance(xml, elements)? {
                let name = xml.name(node)?;
                if !name.as_str().eq_ignore_ascii_case(CHILD) {
                    continue;
                }
                if self.bounds.take() {
                    let child = ReadChild::parse(node, &name, xml)?;
                    let below = ReadLevel::new(child_lists(node));
                    parents.push(PendingLevel {
                        cursor: mem::replace(&mut level, below),
                        child,
                    });
                    continue;
                }
                level.output.truncated = true;
            }
            let Some(PendingLevel { mut cursor, child }) = parents.pop() else {
                return Ok(level.output);
            };
            let source = cursor
                .list_source
                .as_ref()
                .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
            cursor
                .output
                .children
                .push(self.finish_child(child, source, level.output)?);
            level = cursor;
        }
    }

    fn finish_child(
        &mut self,
        authored: ReadChild<'_>,
        list: &XmlNodeSource,
        below: Level,
    ) -> Result<Child, CatalogError> {
        let xml = self.xml;
        let ReadChild {
            record,
            external,
            pose,
            node: child,
        } = authored;
        let mut location = if self.bounds.scope.location_enrichment {
            ChildLocation::read(xml, child, xml.capture())?
        } else {
            None
        };
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
        let unresolved_external = external
            .as_ref()
            .filter(|path| {
                let candidates = self.index.candidates(path.as_str());
                candidates.is_empty()
                    || (self.bounds.scope.unresolved_links && candidates.len() > 1)
            })
            .map(|path| UnresolvedChild::capture(xml, child, path, self.bounds.directory.clone()))
            .transpose()?;
        let deferred_external = self.defer_external(child, external.as_ref())?;
        self.pending_external |= deferred_external.is_some();
        Ok(Child {
            pose,
            record,
            external,
            unresolved_external,
            deferred_external,
            origins,
            location: location.map(Arc::new),
            below,
        })
    }
    fn defer_external(
        &self,
        node: Root<'_>,
        path: Option<&AuthoredEntryPath>,
    ) -> Result<Option<Box<DeferredChild>>, CatalogError> {
        path.filter(|path| {
            self.bounds.scope.unresolved_links && self.index.candidates(path.as_str()).len() == 1
        })
        .map(|_| {
            DeferredChild::capture(self.xml, node, self.bounds.directory.clone()).map(Box::new)
        })
        .transpose()
    }
}

struct ReadLevel<L, C> {
    lists: L,
    children: Option<C>,
    list_source: Option<XmlNodeSource>,
    output: Level,
}

impl<'node, L, C> ReadLevel<L, C>
where
    L: Iterator<Item = Root<'node>>,
    C: Iterator<Item = Root<'node>>,
{
    fn new(lists: L) -> Self {
        Self {
            lists,
            children: None,
            list_source: None,
            output: Level::default(),
        }
    }

    fn advance(
        &mut self,
        xml: &EntryXml<'_, '_, '_>,
        elements: impl Fn(Root<'node>) -> C,
    ) -> Result<Option<Root<'node>>, CatalogError> {
        loop {
            if let Some(child) = self.children.as_mut().and_then(Iterator::next) {
                return Ok(Some(child));
            }
            let Some(list) = self.lists.next() else {
                return Ok(None);
            };
            self.list_source = Some(xml.node_source(list)?);
            self.children = Some(elements(list));
        }
    }
}

struct PendingLevel<'node, L, C> {
    cursor: ReadLevel<L, C>,
    child: ReadChild<'node>,
}

struct ReadChild<'node> {
    node: Root<'node>,
    record: Option<Guid>,
    external: Option<AuthoredEntryPath>,
    pose: Pose,
}

impl<'node> ReadChild<'node> {
    fn parse(
        node: Root<'node>,
        name: &AuthoredEntryPath,
        xml: &EntryXml<'_, '_, '_>,
    ) -> Result<Self, CatalogError> {
        let record = attribute(node, "starMapRecord")
            .and_then(|text| text.trim().parse::<Guid>().ok())
            .filter(|guid| guid.stored() != [0; 16]);
        Ok(Self {
            node,
            record,
            external: external(node, name, xml)?,
            pose: child_pose(node),
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
    cancel: Option<&CancelToken>,
) -> Result<SourceLoss, CatalogError> {
    let mut inventory = level.truncated;
    let mut malformed = false;
    let mut parents = Vec::new();
    let mut children = level.children.iter_mut();
    loop {
        if cancel.is_some_and(CancelToken::is_cancelled) {
            return Err(CatalogError::Cancelled);
        }
        if let Some(child) = children.next() {
            if let Some(location) = &mut child.location {
                Arc::get_mut(location)
                    .ok_or(ExportAccountingError::ConflictingSourceDatum)?
                    .attach_entity(entities, uses);
            }
            malformed |= matches!(&child.pose, Pose::Malformed { .. });
            inventory |= child.below.truncated;
            if !child.below.children.is_empty() {
                parents.push(children);
                children = child.below.children.iter_mut();
            }
        } else if let Some(parent) = parents.pop() {
            children = parent;
        } else {
            return Ok(SourceLoss {
                projection: inventory || malformed,
                inventory,
            });
        }
    }
}

pub(super) fn child_lists(node: Root<'_>) -> impl Iterator<Item = Root<'_>> {
    let mut direct = node
        .children()
        .filter(|section| section.name().eq_ignore_ascii_case(CHILDREN))
        .peekable();
    let fallback = if direct.peek().is_none() {
        first_below(node, CHILDREN)
    } else {
        None
    };
    direct.chain(fallback)
}

pub(super) fn external(
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
