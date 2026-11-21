// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeSet, HashMap};
use std::vec::IntoIter;

use p4kx_extraction::model::Transform;
use p4kx_extraction::{Archive, CancelToken, Guid, PathIndex, SourceUse, SourceUses, SourceValue};

use super::super::facility_instance::ShipContext;
use super::super::loadout_position;
use super::Projection;
use super::child::{self, ChildDeclaration, InteriorChild};
use super::hosts::RoomSelection;
use super::interior::{ClassTarget, StreamRoute};
use super::room_frame::RoomFrame;
use super::routes::{self, ContainerOwner, OpenedRoom, StreamDeclaration};
use super::types::{
    InteriorFindingKind, InteriorRoomContainer, InteriorRoomCoverage, InteriorRoomRef,
};
use crate::catalog_read::original::{
    EntryCapture, EntrySoc, EntryXml, OriginalDatum, PhysicalEntryRef,
};
use crate::starmap_xml::Node;
use crate::{CatalogError, ExportAccountingError};

pub(super) struct RoomReader<'input> {
    pub(super) archive: &'input Archive,
    pub(super) index: &'input PathIndex<'input>,
    pub(super) capture: &'input EntryCapture<'input>,
    pub(super) targets: &'input HashMap<Guid, ClassTarget>,
    pub(super) cancel: Option<&'input CancelToken>,
}

struct ActiveRoom {
    occurrence: Occurrence,
    descriptor: Option<PhysicalEntryRef>,
    children: IntoIter<ChildDeclaration>,
}

struct Occurrence {
    opened: OpenedRoom,
    ordinal: u32,
    selectors: Vec<SourceUse>,
    complete: bool,
    edge_transform_uses: Vec<SourceUse>,
    room_transform: Option<Transform>,
}

struct Edge {
    parent_ordinal: u32,
    declaration: InteriorChild,
    selectors: Vec<SourceUse>,
    transform_uses: Vec<SourceUse>,
    transform: Option<Transform>,
}

impl RoomReader<'_> {
    pub(super) fn room(
        &self,
        project: &mut Projection<'_, '_>,
        ship: &ShipContext,
        identity: InteriorRoomRef,
        room_selector: &RoomSelection<'_>,
        path: &str,
    ) -> Result<InteriorRoomCoverage, CatalogError> {
        let mut coverage = InteriorRoomCoverage {
            room: identity,
            containers: Vec::new(),
            findings: Vec::new(),
        };
        let opened =
            match routes::open_room(self.archive, self.index, self.capture, path, self.cancel)? {
                Ok(opened) => opened,
                Err(kind) => {
                    coverage.note(kind, None, None, None);
                    return Ok(coverage);
                }
            };
        let root = Occurrence::record(
            &mut coverage,
            &project.origins.uses,
            opened,
            room_selector,
            None,
        )?;
        self.walk(project, ship, &mut coverage, room_selector, root)?;
        Ok(coverage)
    }

    fn walk(
        &self,
        project: &mut Projection<'_, '_>,
        ship: &ShipContext,
        coverage: &mut InteriorRoomCoverage,
        room_selector: &RoomSelection<'_>,
        root: Occurrence,
    ) -> Result<(), CatalogError> {
        let mut active = BTreeSet::from([routes::physical_ref(&root.opened.container)?.clone()]);
        let mut rooms =
            vec![self.inspect(project, ship, coverage, room_selector.frame, &[], root)?];
        while let Some(room) = rooms.last_mut() {
            routes::check_cancel(self.cancel)?;
            let Some(declaration) = room.children.next() else {
                let reference = routes::physical_ref(&room.occurrence.opened.container)?;
                active.remove(reference);
                rooms.pop();
                continue;
            };
            let child = self.child(
                &project.origins.uses,
                coverage,
                room_selector,
                &rooms,
                &active,
                declaration,
            )?;
            if let Some(child) = child {
                active.insert(routes::physical_ref(&child.opened.container)?.clone());
                let frame =
                    self.inspect(project, ship, coverage, room_selector.frame, &rooms, child)?;
                rooms.push(frame);
            }
        }
        Ok(())
    }

    fn child(
        &self,
        sources: &SourceUses,
        coverage: &mut InteriorRoomCoverage,
        room_selector: &RoomSelection<'_>,
        ancestors: &[ActiveRoom],
        active: &BTreeSet<PhysicalEntryRef>,
        declaration: ChildDeclaration,
    ) -> Result<Option<Occurrence>, CatalogError> {
        let parent = ancestors
            .last()
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        let ordinal = parent.occurrence.ordinal;
        let descriptor = parent
            .descriptor
            .as_ref()
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        let target = match declaration.name() {
            Ok(name) => routes::open_child(
                self.archive,
                self.index,
                self.capture,
                ancestors.iter().map(|room| &room.occurrence.opened),
                active,
                name.as_str(),
                self.cancel,
            )?,
            Err(kind) => Err(kind),
        };
        let opened = match target {
            Ok(opened) => opened,
            Err(kind) => {
                let source = declaration.into_source(self.capture, sources, descriptor.clone())?;
                coverage.note(kind, Some(ordinal), None, Some(source));
                return Ok(None);
            }
        };
        let local = declaration.transform();
        let transform = parent
            .occurrence
            .room_transform
            .and_then(|parent| parent.then(&local?).finite());
        let invalid =
            local.is_none() || (parent.occurrence.room_transform.is_some() && transform.is_none());
        let edge = Edge {
            parent_ordinal: ordinal,
            selectors: declaration.selection_uses()?,
            transform_uses: declaration.transform_uses(self.capture)?,
            transform,
            declaration: declaration.into_source(self.capture, sources, descriptor.clone())?,
        };
        let occurrence = Occurrence::record(coverage, sources, opened, room_selector, Some(edge))?;
        if invalid {
            coverage.note(
                InteriorFindingKind::InvalidChildTransform,
                Some(occurrence.ordinal),
                None,
                None,
            );
        }
        Ok(Some(occurrence))
    }

    fn inspect(
        &self,
        project: &mut Projection<'_, '_>,
        ship: &ShipContext,
        coverage: &mut InteriorRoomCoverage,
        frame: RoomFrame<'_>,
        ancestors: &[ActiveRoom],
        occurrence: Occurrence,
    ) -> Result<ActiveRoom, CatalogError> {
        let opened = &occurrence.opened;
        let owner = opened.owner(
            self.archive,
            ancestors.iter().map(|room| &room.occurrence.opened),
        )?;
        let index = opened.index();
        let read = match opened.descriptor(owner, &index, self.capture, self.cancel)? {
            Ok(read) => read,
            Err(kind) => return Ok(occurrence.unreadable(coverage, kind)),
        };
        let Ok(xml) = EntryXml::parse(&read, self.capture)? else {
            return Ok(occurrence.unreadable(coverage, InteriorFindingKind::UnreadableDescriptor));
        };
        if xml.is_lossy() || !xml.root().name().eq_ignore_ascii_case("ObjectContainer") {
            return Ok(occurrence.unreadable(coverage, InteriorFindingKind::UnreadableDescriptor));
        }
        let descriptor = read.receipt().payload_reference(self.capture)?;
        let (streams, findings) = routes::declared_streams(&xml, self.cancel)?;
        for kind in findings {
            coverage.note(kind, Some(occurrence.ordinal), None, None);
        }
        let route = occurrence.route(ancestors);
        let context = StreamContext {
            opened,
            owner,
            index: &index,
            descriptor: &descriptor,
            route: &route,
            frame,
        };
        for stream in &streams {
            self.stream(project, ship, coverage, &context, stream)?;
        }
        let children = child::declarations(&xml, self.cancel)?;
        Ok(ActiveRoom {
            descriptor: Some(routes::physical_ref(&descriptor)?.clone()),
            children: children.into_iter(),
            occurrence,
        })
    }

    fn stream(
        &self,
        project: &mut Projection<'_, '_>,
        ship: &ShipContext,
        coverage: &mut InteriorRoomCoverage,
        context: &StreamContext<'_>,
        declaration: &StreamDeclaration,
    ) -> Result<(), CatalogError> {
        routes::check_cancel(self.cancel)?;
        let name = declaration
            .name
            .retain_in(self.capture)?
            .map(|text| text.as_str().to_owned())?
            .into_cell(&project.origins.uses);
        let ordinal = Some(context.route.ordinal);
        let read = match context.opened.stream(
            context.owner,
            context.index,
            self.capture,
            declaration.name.as_str(),
            self.cancel,
        )? {
            Ok(read) => read,
            Err(kind) => {
                coverage.note(kind, ordinal, Some(name), None);
                return Ok(());
            }
        };
        let Ok(soc) = EntrySoc::parse(&read, self.capture)? else {
            coverage.note(
                InteriorFindingKind::UnreadableStream,
                ordinal,
                Some(name),
                None,
            );
            return Ok(());
        };
        let stream = read.receipt().payload_reference(self.capture)?;
        let route = StreamRoute::new(context, declaration, &stream, name)?;
        for chunk in soc.chunks() {
            routes::check_cancel(self.cancel)?;
            match chunk {
                Ok(chunk) => {
                    if chunk.is_lossy() {
                        route.note(coverage, InteriorFindingKind::LossyEntityChunk);
                    }
                    for entity in chunk.entities() {
                        routes::check_cancel(self.cancel)?;
                        self.entity(project, coverage, ship, &route, &entity)?;
                    }
                }
                Err(_) => route.note(coverage, InteriorFindingKind::UnreadableEntityChunk),
            }
        }
        Ok(())
    }
}

impl Occurrence {
    fn record(
        coverage: &mut InteriorRoomCoverage,
        sources: &SourceUses,
        opened: OpenedRoom,
        room_selector: &RoomSelection<'_>,
        edge: Option<Edge>,
    ) -> Result<Self, CatalogError> {
        let ordinal = loadout_position::ordinal(coverage.containers.len())?;
        let (inputs, complete) = opened.container.contributions();
        let mut selectors = inputs.to_vec();
        let (parent_ordinal, child, edge_transform_uses, room_transform) = if let Some(edge) = edge
        {
            selectors.extend(edge.selectors);
            (
                Some(edge.parent_ordinal),
                Some(edge.declaration),
                edge.transform_uses,
                edge.transform,
            )
        } else {
            selectors.extend(room_selector.inputs().cloned());
            (None, None, Vec::new(), Some(Transform::IDENTITY))
        };
        let complete = complete && (parent_ordinal.is_some() || room_selector.complete());
        let value = SourceValue::Present(routes::physical_ref(&opened.container)?.clone());
        coverage.containers.push(InteriorRoomContainer {
            parent_ordinal,
            container: sources.value(value, &selectors, complete),
            child,
        });
        Ok(Self {
            opened,
            ordinal,
            selectors,
            complete,
            edge_transform_uses,
            room_transform,
        })
    }

    fn unreadable(
        self,
        coverage: &mut InteriorRoomCoverage,
        kind: InteriorFindingKind,
    ) -> ActiveRoom {
        coverage.note(kind, Some(self.ordinal), None, None);
        ActiveRoom {
            occurrence: self,
            descriptor: None,
            children: Vec::new().into_iter(),
        }
    }

    fn route(&self, ancestors: &[ActiveRoom]) -> ContainerRoute {
        let mut selectors = Vec::new();
        let mut transform_uses = Vec::new();
        let mut complete = true;
        for occurrence in ancestors
            .iter()
            .map(|room| &room.occurrence)
            .chain(std::iter::once(self))
        {
            selectors.extend_from_slice(&occurrence.selectors);
            transform_uses.extend_from_slice(&occurrence.edge_transform_uses);
            complete &= occurrence.complete;
        }
        ContainerRoute {
            ordinal: self.ordinal,
            selectors,
            complete,
            root_transform: (!ancestors.is_empty()).then_some(RootRoomTransform {
                value: self.room_transform,
                uses: transform_uses,
            }),
        }
    }
}

pub(super) struct ContainerRoute {
    pub(super) ordinal: u32,
    pub(super) selectors: Vec<SourceUse>,
    pub(super) complete: bool,
    pub(super) root_transform: Option<RootRoomTransform>,
}

pub(super) struct RootRoomTransform {
    pub(super) value: Option<Transform>,
    pub(super) uses: Vec<SourceUse>,
}

pub(super) struct StreamContext<'read> {
    pub(super) opened: &'read OpenedRoom,
    owner: ContainerOwner<'read>,
    index: &'read PathIndex<'read>,
    pub(super) descriptor: &'read OriginalDatum<PhysicalEntryRef>,
    pub(super) route: &'read ContainerRoute,
    pub(super) frame: RoomFrame<'read>,
}
