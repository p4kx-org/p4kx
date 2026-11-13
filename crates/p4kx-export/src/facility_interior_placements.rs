// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{HashMap, HashSet};

use p4kx_extraction::{
    Archive, DataForge, Guid, PathIndex, SourceUse, SourceUseRole, SourceUses, SourceValue,
    SourcedValue,
};

use super::super::catalog_entity_configurations::EntityConfigurationRows;
use super::super::facility_instance::{FacilityPlacementSource, ShipContext};
use p4kx_extraction::model::Transform;

use super::Projection;
use super::routes::{self, StreamDeclaration};
use super::types::{
    InteriorFacilityPlacement, InteriorFindingKind, InteriorRoomCoverage, InteriorRoomRef,
    InteriorSocPosition, SocAttribute,
};
use super::walk::{RoomReader, RootRoomTransform, StreamContext};
use crate::catalog_read::original::{
    EntryCapture, EntrySocEntity, OriginalDatum, PhysicalEntryRef, SocSourceFields,
};
use crate::contract_entities::Ship;
use crate::record_header_field::RecordHeaderField;
use crate::{CatalogError, ExportAccountingError};

#[derive(Clone, Copy)]
pub(super) enum ClassTarget {
    One(u32),
    Ambiguous,
}

pub(super) fn project(
    project: &mut Projection<'_, '_>,
    archive: &Archive,
    forge: &DataForge<'_>,
    ships: &[Ship<'_>],
    declarations: &EntityConfigurationRows<'_>,
    capture: &EntryCapture<'_>,
) -> Result<Vec<InteriorRoomCoverage>, CatalogError> {
    let hosts: HashMap<_, _> = ships
        .iter()
        .map(|ship| (ship.source_record_index, ship.guid))
        .collect();
    let mut roots: Vec<_> = declarations
        .topology
        .rooms
        .iter()
        .filter_map(|room| {
            let record = room.owner().key().0?;
            Some((record, *hosts.get(&record)?, room))
        })
        .collect();
    roots.sort_by_key(|(_, _, room)| room.owner().key());
    if roots.is_empty() {
        return Ok(Vec::new());
    }
    let targets = class_targets(project, forge)?;
    let index = PathIndex::new(archive);
    let reader = RoomReader {
        archive,
        index: &index,
        capture,
        targets: &targets,
        cancel: project.cancel,
    };
    let mut coverage = Vec::new();
    for (index, guid, declaration) in roots {
        project.check_cancel()?;
        let ship = ShipContext::read(project.origins, guid, index)?;
        for room in declaration.containers() {
            project.check_cancel()?;
            let Some(path) = room.file_name().filter(|path| !path.trim().is_empty()) else {
                continue;
            };
            let identity = InteriorRoomRef {
                owner: declaration.owner().clone(),
                object_container_ordinal: room.container_ordinal.clone(),
            };
            let selector = room
                .file_use
                .as_ref()
                .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
            coverage.push(reader.room(project, &ship, identity, selector, path)?);
        }
    }
    Ok(coverage)
}

fn class_targets(
    project: &Projection<'_, '_>,
    forge: &DataForge<'_>,
) -> Result<HashMap<Guid, ClassTarget>, CatalogError> {
    let configured: HashSet<_> = project
        .definitions
        .keys()
        .map(|&index| {
            forge
                .record(index)
                .map(|record| record.guid())
                .ok_or(ExportAccountingError::ConflictingSourceDatum)
        })
        .collect::<Result<_, _>>()?;
    let mut targets = HashMap::new();
    for record in forge.records() {
        project.check_cancel()?;
        let guid = record.guid();
        if configured.contains(&guid) {
            targets
                .entry(guid)
                .and_modify(|found| *found = ClassTarget::Ambiguous)
                .or_insert_with(|| ClassTarget::One(record.index()));
        }
    }
    Ok(targets)
}

impl RoomReader<'_> {
    pub(super) fn entity(
        &self,
        project: &mut Projection<'_, '_>,
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
        let Some(target) = self.targets.get(&guid) else {
            return Ok(());
        };
        let ClassTarget::One(index) = *target else {
            route.note(coverage, InteriorFindingKind::AmbiguousClass);
            return Ok(());
        };
        let Some(configurations) = project.configuration_links(index)? else {
            return Ok(());
        };
        let source = route.placement(project, coverage.room.clone(), entity.fields()?, index)?;
        project.rows.push(super::FacilityInstance {
            ship: ship.clone(),
            source: FacilityPlacementSource::Interior(Box::new(source)),
            configurations,
        });
        Ok(())
    }
}

pub(super) struct StreamRoute<'room> {
    name: SourcedValue<String>,
    ordinal: u32,
    root_transform: Option<&'room RootRoomTransform>,
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
        project: &Projection<'_, '_>,
        room: InteriorRoomRef,
        fields: SocSourceFields,
        index: u32,
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
