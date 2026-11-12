// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{HashMap, HashSet};

use p4kx_extraction::{
    Archive, CancelToken, DataForge, Guid, PathIndex, SourceUse, SourceUseRole, SourceValue,
    SourcedValue,
};

use super::super::catalog_entity_configurations::EntityConfigurationRows;
use super::super::facility_instance::{FacilityPlacementSource, ShipContext};
use super::super::room_associations::RoomAssociation;
use super::Projection;
use super::routes::{self, OpenedRoom, StreamDeclaration};
use super::types::{
    InteriorFacilityPlacement, InteriorFinding, InteriorFindingKind, InteriorRoomCoverage,
    InteriorRoomRef, InteriorSocPosition, SocAttribute,
};
use crate::catalog_read::original::{
    EntryCapture, EntrySoc, EntrySocEntity, EntryXml, OriginalDatum, PhysicalEntryRef,
    SocSourceFields,
};
use crate::contract_entities::Ship;
use crate::record_header_field::RecordHeaderField;
use crate::starmap_xml::Node;
use crate::{CatalogError, ExportAccountingError};

#[derive(Clone, Copy)]
enum ClassTarget {
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
            coverage.push(reader.room(project, &ship, identity, room, path)?);
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

struct RoomReader<'input> {
    archive: &'input Archive,
    index: &'input PathIndex<'input>,
    capture: &'input EntryCapture<'input>,
    targets: &'input HashMap<Guid, ClassTarget>,
    cancel: Option<&'input CancelToken>,
}

impl RoomReader<'_> {
    fn room(
        &self,
        project: &mut Projection<'_, '_>,
        ship: &ShipContext,
        identity: InteriorRoomRef,
        room: &RoomAssociation<'_>,
        path: &str,
    ) -> Result<InteriorRoomCoverage, CatalogError> {
        let mut coverage = InteriorRoomCoverage {
            room: identity,
            findings: Vec::new(),
        };
        let opened =
            match routes::open_room(self.archive, self.index, self.capture, path, self.cancel)? {
                Ok(opened) => opened,
                Err(kind) => return Ok(coverage.with_finding(kind, None)),
            };
        let index = PathIndex::new(&opened.archive);
        let read = match opened.descriptor(self.archive, &index, self.capture, self.cancel)? {
            Ok(read) => read,
            Err(kind) => return Ok(coverage.with_finding(kind, None)),
        };
        let Ok(xml) = EntryXml::parse(&read, self.capture)? else {
            return Ok(coverage.with_finding(InteriorFindingKind::UnreadableDescriptor, None));
        };
        if xml.is_lossy() || !xml.root().name().eq_ignore_ascii_case("ObjectContainer") {
            return Ok(coverage.with_finding(InteriorFindingKind::UnreadableDescriptor, None));
        }
        let descriptor = read.receipt().payload_reference(self.capture)?;
        let (streams, findings) = routes::declared_streams(&xml, self.cancel)?;
        for kind in findings {
            coverage.note(kind, None);
        }
        let room_selector = room
            .file_use
            .as_ref()
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        for stream in streams {
            let context = StreamContext {
                ship,
                opened: &opened,
                index: &index,
                descriptor: &descriptor,
                room_selector,
            };
            self.stream(project, &mut coverage, &context, &stream)?;
        }
        Ok(coverage)
    }

    fn stream(
        &self,
        project: &mut Projection<'_, '_>,
        coverage: &mut InteriorRoomCoverage,
        context: &StreamContext<'_>,
        declaration: &StreamDeclaration,
    ) -> Result<(), CatalogError> {
        routes::check_cancel(self.cancel)?;
        let source_name = declaration
            .name
            .retain_in(self.capture)?
            .map(|text| text.as_str().to_owned())?
            .into_cell(&project.origins.uses);
        let read = match context.opened.stream(
            self.archive,
            context.index,
            self.capture,
            declaration.name.as_str(),
            self.cancel,
        )? {
            Ok(read) => read,
            Err(kind) => {
                coverage.note(kind, Some(source_name));
                return Ok(());
            }
        };
        let Ok(soc) = EntrySoc::parse(&read, self.capture)? else {
            coverage.note(InteriorFindingKind::UnreadableStream, Some(source_name));
            return Ok(());
        };
        let stream = read.receipt().payload_reference(self.capture)?;
        let route = StreamRoute::new(context, declaration, &stream, source_name)?;
        for chunk in soc.chunks() {
            routes::check_cancel(self.cancel)?;
            match chunk {
                Ok(chunk) => {
                    if chunk.is_lossy() {
                        coverage.note(
                            InteriorFindingKind::LossyEntityChunk,
                            Some(route.name.clone()),
                        );
                    }
                    for entity in chunk.entities() {
                        routes::check_cancel(self.cancel)?;
                        self.entity(project, coverage, context.ship, &route, &entity)?;
                    }
                }
                Err(_) => coverage.note(
                    InteriorFindingKind::UnreadableEntityChunk,
                    Some(route.name.clone()),
                ),
            }
        }
        Ok(())
    }

    fn entity(
        &self,
        project: &mut Projection<'_, '_>,
        coverage: &mut InteriorRoomCoverage,
        ship: &ShipContext,
        route: &StreamRoute,
        entity: &EntrySocEntity<'_>,
    ) -> Result<(), CatalogError> {
        let mut classes = entity.class_guids();
        let Some(class) = classes.next() else {
            return Ok(());
        };
        if classes.next().is_some() {
            coverage.note(
                InteriorFindingKind::AmbiguousClass,
                Some(route.name.clone()),
            );
            return Ok(());
        }
        let Ok(guid) = class.trim().parse::<Guid>() else {
            coverage.note(InteriorFindingKind::InvalidClass, Some(route.name.clone()));
            return Ok(());
        };
        let Some(target) = self.targets.get(&guid) else {
            return Ok(());
        };
        let ClassTarget::One(index) = *target else {
            coverage.note(
                InteriorFindingKind::AmbiguousClass,
                Some(route.name.clone()),
            );
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

struct StreamContext<'input> {
    ship: &'input ShipContext,
    opened: &'input OpenedRoom<'input>,
    index: &'input PathIndex<'input>,
    descriptor: &'input OriginalDatum<PhysicalEntryRef>,
    room_selector: &'input SourceUse,
}

struct StreamRoute {
    name: SourcedValue<String>,
    container: PhysicalEntryRef,
    descriptor: PhysicalEntryRef,
    descriptor_node_index: u32,
    stream: PhysicalEntryRef,
    uses: Vec<SourceUse>,
    complete: bool,
}

impl StreamRoute {
    fn new(
        context: &StreamContext<'_>,
        declaration: &StreamDeclaration,
        stream: &OriginalDatum<PhysicalEntryRef>,
        name: SourcedValue<String>,
    ) -> Result<Self, CatalogError> {
        let (mut uses, mut complete) = declaration.name.selector_inputs()?;
        uses.extend([context.room_selector.clone(), declaration.selection.clone()]);
        for datum in [&context.opened.container, context.descriptor, stream] {
            let (contributors, known) = datum.contributions();
            uses.extend_from_slice(contributors);
            complete &= known;
        }
        Ok(Self {
            name,
            container: physical_ref(&context.opened.container)?,
            descriptor: physical_ref(context.descriptor)?,
            descriptor_node_index: declaration.node_index,
            stream: physical_ref(stream)?,
            uses,
            complete,
        })
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

fn physical_ref(datum: &OriginalDatum<PhysicalEntryRef>) -> Result<PhysicalEntryRef, CatalogError> {
    match datum.value() {
        SourceValue::Present(entry) => Ok(entry.clone()),
        _ => Err(ExportAccountingError::ConflictingSourceDatum.into()),
    }
}

impl InteriorRoomCoverage {
    fn with_finding(
        mut self,
        kind: InteriorFindingKind,
        stream: Option<SourcedValue<String>>,
    ) -> Self {
        self.note(kind, stream);
        self
    }

    fn note(&mut self, kind: InteriorFindingKind, stream: Option<SourcedValue<String>>) {
        self.findings.push(InteriorFinding { kind, stream });
    }
}
