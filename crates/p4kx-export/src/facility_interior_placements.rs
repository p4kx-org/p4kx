// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    Archive, DataForge, Guid, PathKeys, SourceUse, SourceUseRole, SourceUses, SourceValue,
    SourcedValue,
};

use super::super::facility_instance::{FacilityPlacementSource, ShipContext};
use super::classes::{ClassTarget, ClassTargets};
use super::hosts::{RoomAttachment, RoomHost, RoomOwners};
use super::model_helper::RoomHelperModels;
use p4kx_extraction::model::Transform;

use super::Projection;
use super::room_frame::RoomFrame;
use super::routes::{self, StreamDeclaration};
use super::types::{
    InteriorFacilityPlacement, InteriorFindingKind, InteriorRoomCoverage, InteriorRoomRef,
    InteriorSocPosition, SocAttribute,
};
use super::walk::{RoomReader, RootRoomTransform, StreamContext};
use crate::CatalogError;
use crate::catalog_read::original::{
    EntryCapture, EntrySocEntity, OriginalDatum, PhysicalEntryRef, SocSourceFields,
};
use crate::contract_entities::Ship;
use crate::record_header_field::RecordHeaderField;

pub(super) fn project(
    project: &mut Projection<'_, '_, '_>,
    archive: &Archive,
    forge: &DataForge<'_>,
    ships: &[Ship<'_>],
    rooms: &RoomOwners<'_, '_>,
    attachments: Vec<RoomAttachment>,
    capture: &EntryCapture<'_>,
) -> Result<Vec<InteriorRoomCoverage>, CatalogError> {
    let mut ship_hosts: Vec<_> = ships
        .iter()
        .filter(|ship| rooms.contains(ship.source_record_index))
        .collect();
    ship_hosts.sort_by_key(|ship| ship.source_record_index);
    if ship_hosts.is_empty() && attachments.is_empty() {
        return Ok(Vec::new());
    }
    let configured = project.indexed_definitions()?.keys().copied();
    let targets = ClassTargets::new(forge, configured, project.cancel)?;
    let keys = PathKeys::new(archive);
    let index = keys.index(archive);
    let mut reader = RoomReader {
        archive,
        index: &index,
        capture,
        targets,
        cancel: project.cancel,
    };
    let mut coverage = Vec::new();
    let mut models = RoomHelperModels::new(
        &index,
        &keys,
        forge,
        capture,
        project.origins,
        project.cancel,
    );
    for root in ship_hosts {
        project.check_cancel()?;
        let record = root.source_record_index;
        let ship = ShipContext::read(project.origins, root.identity.guid, record)?;
        let host = RoomHost {
            record,
            ship: &ship,
            attachment: None,
        };
        reader.project_host(project, &mut coverage, rooms, &host, Some(&mut models))?;
    }
    for attachment in attachments {
        project.check_cancel()?;
        let host = RoomHost {
            record: attachment.record,
            ship: &attachment.ship,
            attachment: Some(&attachment),
        };
        reader.project_host(project, &mut coverage, rooms, &host, Some(&mut models))?;
    }
    Ok(coverage)
}

pub(super) struct RoomPaths<'archive> {
    pub(super) archive: &'archive Archive,
    pub(super) keys: &'archive PathKeys,
}

pub(super) fn scoped<'host>(
    project: &mut Projection<'_, '_, '_>,
    paths: &RoomPaths<'_>,
    rooms: &RoomOwners<'_, '_>,
    hosts: impl IntoIterator<Item = RoomHost<'host>>,
    capture: &EntryCapture<'_>,
    targets: ClassTargets<'_, '_>,
    model_forge: Option<&DataForge<'_>>,
) -> Result<Vec<InteriorRoomCoverage>, CatalogError> {
    let index = paths.keys.index(paths.archive);
    let mut reader = RoomReader {
        archive: paths.archive,
        index: &index,
        capture,
        targets,
        cancel: project.cancel,
    };
    let mut coverage = Vec::new();
    let mut models = model_forge.map(|forge| {
        RoomHelperModels::new(
            &index,
            paths.keys,
            forge,
            capture,
            project.origins,
            project.cancel,
        )
    });
    for host in hosts {
        project.check_cancel()?;
        reader.project_host(project, &mut coverage, rooms, &host, models.as_mut())?;
    }
    Ok(coverage)
}

impl RoomReader<'_, '_> {
    pub(super) fn entity(
        &mut self,
        project: &mut Projection<'_, '_, '_>,
        coverage: &mut InteriorRoomCoverage,
        ship: &ShipContext,
        route: &StreamRoute<'_>,
        entity: &EntrySocEntity<'_>,
    ) -> Result<(), CatalogError> {
        let mut classes = entity.class_guids();
        let Some(class) = classes.next() else {
            return Ok(());
        };
        if classes.next().is_some() {
            route.note(coverage, InteriorFindingKind::AmbiguousClass);
            return Ok(());
        }
        let Ok(guid) = class.trim().parse::<Guid>() else {
            route.note(coverage, InteriorFindingKind::InvalidClass);
            return Ok(());
        };
        let Some(target) = self.targets.resolve(guid)? else {
            return Ok(());
        };
        let ClassTarget::One(index) = target else {
            route.note(coverage, InteriorFindingKind::AmbiguousClass);
            return Ok(());
        };
        let Some(frame) = route.ship_frame else {
            return Ok(());
        };
        let configurations = match self.targets.scoped_configuration_links(index) {
            Some(links) => links?,
            None => project.configuration_links(index)?,
        };
        if configurations.is_empty() {
            return Ok(());
        }
        let source = route.placement(
            project,
            coverage.room.clone(),
            entity.fields()?,
            index,
            frame,
        )?;
        (project.emit_instance)(super::FacilityInstance {
            ship: ship.clone(),
            source: FacilityPlacementSource::Interior(Box::new(source)),
            configurations,
        })
    }
}

pub(super) struct StreamRoute<'room> {
    name: SourcedValue<String>,
    ordinal: u32,
    root_transform: Option<&'room RootRoomTransform>,
    ship_frame: Option<RoomFrame<'room>>,
    container: PhysicalEntryRef,
    descriptor: PhysicalEntryRef,
    descriptor_node_index: u32,
    stream: PhysicalEntryRef,
    uses: Vec<SourceUse>,
    complete: bool,
}

impl<'room> StreamRoute<'room> {
    pub(super) fn new(
        context: &'room StreamContext<'_>,
        declaration: &StreamDeclaration,
        stream: &OriginalDatum<PhysicalEntryRef>,
        name: SourcedValue<String>,
    ) -> Result<Self, CatalogError> {
        let (mut uses, mut complete) = declaration.name.selector_inputs()?;
        uses.push(declaration.selection.clone());
        uses.extend_from_slice(&context.route.selectors);
        complete &= context.route.complete;
        for datum in [context.descriptor, stream] {
            let (contributors, known) = datum.contributions();
            uses.extend_from_slice(contributors);
            complete &= known;
        }
        Ok(Self {
            name,
            ordinal: context.route.ordinal,
            root_transform: context.route.root_transform.as_ref(),
            ship_frame: context.frame,
            container: routes::physical_ref(&context.opened.container)?.clone(),
            descriptor: routes::physical_ref(context.descriptor)?.clone(),
            descriptor_node_index: declaration.node_index,
            stream: routes::physical_ref(stream)?.clone(),
            uses,
            complete,
        })
    }

    pub(super) fn note(&self, coverage: &mut InteriorRoomCoverage, kind: InteriorFindingKind) {
        coverage.note(kind, Some(self.ordinal), Some(self.name.clone()), None);
    }

    fn placement(
        &self,
        project: &Projection<'_, '_, '_>,
        room: InteriorRoomRef,
        fields: SocSourceFields,
        index: u32,
        frame: RoomFrame<'_>,
    ) -> Result<InteriorFacilityPlacement, CatalogError> {
        let sources = &project.origins.uses;
        let mut route = self.uses.clone();
        route.extend([fields.selection, fields.class_selection.clone()]);
        let target = project.origins.record(index);
        let uses = [
            fields.class_selection,
            target.observe_header(RecordHeaderField::Guid, SourceUseRole::Selector)?,
            target.observe_header(RecordHeaderField::RecordIndex, SourceUseRole::Selector)?,
        ];
        let position = InteriorSocPosition {
            container_occurrence_ordinal: self.ordinal,
            container: self.container.clone(),
            descriptor: self.descriptor.clone(),
            descriptor_node_index: self.descriptor_node_index,
            stream: self.stream.clone(),
            chunk_index: fields.chunk_index,
            entity_ordinal: fields.entity_ordinal,
        };
        Ok(InteriorFacilityPlacement {
            room,
            position: sources.value(
                SourceValue::Present(position),
                &route,
                self.complete && fields.complete,
            ),
            stream_name: self.name.clone(),
            class_record_index: sources.value(SourceValue::Present(index), &uses, fields.complete),
            root_room_transform: self
                .root_transform
                .map(|root| root.for_entity(sources, &fields.local_transform)),
            ship_frame_transform: frame.for_entity(
                sources,
                self.root_transform,
                &fields.local_transform,
                &route,
                self.complete && fields.complete,
            ),
            local_transform: fields.local_transform.into_cell(sources),
            entity_attributes: fields
                .attributes
                .into_iter()
                .map(|attribute| SocAttribute {
                    name: attribute.name.into_cell(sources),
                    value: attribute.value.into_cell(sources),
                })
                .collect(),
        })
    }
}

impl RootRoomTransform {
    fn for_entity(
        &self,
        sources: &SourceUses,
        local: &OriginalDatum<[[f32; 4]; 3]>,
    ) -> SourcedValue<[[f32; 4]; 3]> {
        let (entity_inputs, complete) = local.contributions();
        let mut uses = self.uses.clone();
        uses.extend_from_slice(entity_inputs);
        let value = match (self.value, local.value()) {
            (Some(root), SourceValue::Present(rows)) => root
                .then(&Transform { rows: *rows })
                .finite()
                .map_or(SourceValue::Invalid, |transform| {
                    SourceValue::Present(transform.rows)
                }),
            _ => SourceValue::Invalid,
        };
        sources.value(value, &uses, complete)
    }
}
