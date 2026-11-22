// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use p4kx_extraction::{Archive, CancelToken, DataForge, SourceUseRole};

use self::hosts::{RoomAttachment, RoomOwners};
use self::installed::InstalledConfigurations;

use super::catalog_independent_configurations::flight_configuration_tables::FlightConfigurationTables;

use super::catalog_entity_configurations::{
    EntityConfigurationRows, loadout_facility_owner_source_type,
};
use super::facility_instance::{
    FacilityConfiguration, FacilityInstance, FacilityPlacementSource, LoadoutFacilityPlacement,
    ShipContext,
};
use super::item_component_owner::ComponentOwner;
use super::loadout_position;
use super::resource_topology_declarations;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_query_session::checkpoint;
use crate::catalog_read::original::EntryCapture;
use crate::catalog_table::CatalogTable;
use crate::contract_entities::Ship;
use crate::contract_loadout::{Loadout, LoadoutPort};
use crate::loadout::alignment::AttachmentIdentity;
use crate::loadout::staging::StagedLoadouts;
use crate::options::{CatalogRun, CatalogStage};
use crate::record_header_field::RecordHeaderField;
use crate::{CatalogError, ExportAccountingError};

#[path = "facility_interior_child.rs"]
mod child;
#[path = "facility_interior_hosts.rs"]
mod hosts;
#[path = "installed_configuration.rs"]
mod installed;
#[path = "loadout_walk_state.rs"]
mod loadout_walk_state;
pub(in crate::catalog_v3) use installed::InstalledConfiguration;
use loadout_walk_state::{Frame, Path};
#[path = "facility_interior_placements.rs"]
mod interior;
#[path = "facility_model_helper.rs"]
mod model_helper;
#[path = "facility_model_node_inputs.rs"]
mod node_inputs;
#[path = "facility_room_frame.rs"]
mod room_frame;
#[path = "facility_interior_routes.rs"]
mod routes;
#[path = "facility_interior_types.rs"]
mod types;
#[path = "facility_interior_walk.rs"]
mod walk;

pub(super) use types::{InteriorFacilityPlacement, InteriorRoomCoverage};

pub(super) struct LoadoutPlacements {
    pub(super) facilities: CatalogTable<FacilityInstance>,
    pub(super) rooms: Vec<InteriorRoomCoverage>,
    pub(super) flight: CatalogTable<InstalledConfiguration>,
}

#[derive(Clone, Copy)]
pub(super) struct ConfigurationSources<'rows, 'source> {
    pub(super) entities: &'rows EntityConfigurationRows<'source>,
    pub(super) flight: &'rows FlightConfigurationTables<'source>,
}

/// Links resolved vehicle occurrences to their physical configuration rows.
pub(super) fn table(
    archive: &Archive,
    forge: &DataForge<'_>,
    ships: &[Ship<'_>],
    loadouts: &StagedLoadouts,
    configurations: ConfigurationSources<'_, '_>,
    input: &OriginalCatalogInput<'_, '_>,
    run: CatalogRun<'_, '_>,
) -> Result<LoadoutPlacements, CatalogError> {
    let cancel = run.options.cancel;
    input.check_forge(forge)?;
    let mut project = Projection::new(configurations.entities, input.origins(), cancel)?;
    let mut flight_instances = InstalledConfigurations::new(configurations.flight)?;
    let rooms = RoomOwners::new(configurations.entities)?;
    let coverage = if project.indexed_definitions()?.is_empty()
        && rooms.is_empty()
        && flight_instances.is_empty()
    {
        Vec::new()
    } else {
        let mut attachments = Vec::new();
        for ship in loadouts.ships() {
            project.check_cancel()?;
            project.loadout(
                &ship.loadout,
                &rooms,
                &mut attachments,
                &mut flight_instances,
            )?;
        }
        run.begin(CatalogStage::ProjectFacilityInteriors);
        let capture = EntryCapture::for_input(input);
        interior::project(
            &mut project,
            archive,
            forge,
            ships,
            &rooms,
            attachments,
            &capture,
        )?
    };
    let instances = CatalogTable::matching_source_types(
        forge,
        |name| {
            loadout_facility_owner_source_type(name)
                || name == resource_topology_declarations::ITEM_ROOM
        },
        project.rows,
    );
    Ok(LoadoutPlacements {
        facilities: instances,
        rooms: coverage,
        flight: FlightConfigurationTables::installed_table(forge, flight_instances.rows),
    })
}

pub(crate) fn project_loadout<'lookup>(
    loadout: &Loadout,
    origins: &CatalogOrigins,
    cancel: Option<&CancelToken>,
    lookup: &'lookup mut ConfigurationLookup<'lookup>,
) -> Result<Vec<FacilityInstance>, CatalogError> {
    let mut projection = Projection {
        definitions: Definitions::Scoped(lookup),
        origins,
        cancel,
        rows: Vec::new(),
    };
    projection.loadout(
        loadout,
        &RoomOwners::default(),
        &mut Vec::new(),
        &mut InstalledConfigurations::default(),
    )?;
    Ok(projection.rows)
}

pub(crate) fn loadout_configuration_links(
    record: u32,
    declarations: &EntityConfigurationRows<'_>,
    cancel: Option<&CancelToken>,
) -> Result<Vec<FacilityConfiguration>, CatalogError> {
    let mut definitions = Vec::new();
    for definition in owner_definitions(declarations) {
        checkpoint(cancel)?;
        match definition.owner().key() {
            (Some(index), Some(_)) if index == record => definitions.push(definition),
            _ => return Err(ExportAccountingError::ConflictingSourceDatum.into()),
        }
    }
    sort_definitions(&mut definitions);
    link_definitions(&definitions, cancel)
}

#[derive(Clone, Copy)]
enum Definition<'rows> {
    Facility(&'rows ComponentOwner),
    Inventory(&'rows ComponentOwner),
}

impl<'rows> Definition<'rows> {
    const fn owner(self) -> &'rows ComponentOwner {
        match self {
            Self::Facility(owner) | Self::Inventory(owner) => owner,
        }
    }

    fn link(self) -> FacilityConfiguration {
        match self {
            Self::Facility(owner) => FacilityConfiguration::Facility(owner.clone()),
            Self::Inventory(owner) => FacilityConfiguration::Inventory(owner.clone()),
        }
    }
}

fn owner_definitions<'rows>(
    declarations: &'rows EntityConfigurationRows<'_>,
) -> impl Iterator<Item = Definition<'rows>> {
    declarations
        .facility_owners()
        .map(Definition::Facility)
        .chain(declarations.storage_owners().map(Definition::Inventory))
}

fn sort_definitions(definitions: &mut [Definition<'_>]) {
    definitions.sort_by_key(|definition| definition.owner().key().1);
}

fn link_definitions(
    definitions: &[Definition<'_>],
    cancel: Option<&CancelToken>,
) -> Result<Vec<FacilityConfiguration>, CatalogError> {
    definitions
        .iter()
        .map(|definition| {
            checkpoint(cancel)?;
            Ok(definition.link())
        })
        .collect()
}

pub(crate) type ConfigurationLookup<'lookup> =
    dyn FnMut(u32) -> Result<Vec<FacilityConfiguration>, CatalogError> + 'lookup;

type IndexedDefinitions<'rows> = HashMap<u32, Vec<Definition<'rows>>>;

enum Definitions<'rows> {
    Indexed(IndexedDefinitions<'rows>),
    Scoped(&'rows mut ConfigurationLookup<'rows>),
}

struct Projection<'rows, 'origin> {
    definitions: Definitions<'rows>,
    origins: &'origin CatalogOrigins,
    cancel: Option<&'origin CancelToken>,
    rows: Vec<FacilityInstance>,
}

impl<'rows, 'origin> Projection<'rows, 'origin> {
    fn new(
        declarations: &'rows EntityConfigurationRows<'_>,
        origins: &'origin CatalogOrigins,
        cancel: Option<&'origin CancelToken>,
    ) -> Result<Self, CatalogError> {
        let mut index = IndexedDefinitions::new();
        for definition in owner_definitions(declarations) {
            checkpoint(cancel)?;
            let (Some(record), Some(_)) = definition.owner().key() else {
                return Err(ExportAccountingError::ConflictingSourceDatum.into());
            };
            index.entry(record).or_default().push(definition);
        }
        for definitions in index.values_mut() {
            sort_definitions(definitions);
        }
        Ok(Self {
            definitions: Definitions::Indexed(index),
            origins,
            cancel,
            rows: Vec::new(),
        })
    }

    fn indexed_definitions(&self) -> Result<&IndexedDefinitions<'rows>, CatalogError> {
        match &self.definitions {
            Definitions::Indexed(definitions) => Ok(definitions),
            Definitions::Scoped(_) => Err(ExportAccountingError::ConflictingSourceDatum.into()),
        }
    }

    fn loadout(
        &mut self,
        loadout: &Loadout,
        rooms: &RoomOwners<'_, '_>,
        room_attachments: &mut Vec<RoomAttachment>,
        flight: &mut InstalledConfigurations<'_>,
    ) -> Result<(), CatalogError> {
        let root = loadout.root_record_index;
        let ship = ShipContext::read(self.origins, loadout.entity_guid.value, root)?;
        let selector = self
            .origins
            .record(root)
            .observe_header(RecordHeaderField::RecordIndex, SourceUseRole::Selector)?;
        let walk = Path::new(root, selector);
        let configurations = self.configuration_links(root)?;
        self.emit(&ship, &walk, None, configurations)?;
        flight.emit(&ship, root, || walk.position_cell(&self.origins.uses), None)?;
        self.walk_ports(loadout, &ship, walk, rooms, room_attachments, flight)
    }

    fn walk_ports(
        &mut self,
        loadout: &Loadout,
        ship: &ShipContext,
        mut walk: Path,
        rooms: &RoomOwners<'_, '_>,
        room_attachments: &mut Vec<RoomAttachment>,
        flight: &mut InstalledConfigurations<'_>,
    ) -> Result<(), CatalogError> {
        let mut stack = vec![Frame::new(&loadout.ports, &walk)];
        while let Some(frame) = stack.last_mut() {
            self.check_cancel()?;
            let Some((ordinal, port)) = frame.ports.next() else {
                walk.restore(frame.parent);
                stack.pop();
                continue;
            };
            if port.docked_vehicle || port.unresolved.is_some() {
                continue;
            }
            let Some((attachment, proof)) = port.source_constraints.placement_attachment() else {
                continue;
            };
            let Some(record) = attachment.target_record_index.value().copied() else {
                continue;
            };
            let has_rooms = rooms.contains(record);
            let configurations = self.configuration_links(record)?;
            if port.ports.is_empty()
                && !has_rooms
                && configurations.is_empty()
                && !flight.contains(record)
            {
                continue;
            }
            let parent = walk.checkpoint();
            walk.position
                .port_ordinals
                .push(loadout_position::ordinal(ordinal)?);
            walk.uses.extend_from_slice(&proof.source_uses);
            let name_complete = port.append_name_uses(&mut walk.uses)?;
            walk.complete &= proof.source_set_complete && name_complete;
            let attached = Some((port, attachment));
            self.emit(ship, &walk, attached, configurations)?;
            flight.emit(
                ship,
                record,
                || walk.position_cell(&self.origins.uses),
                attached,
            )?;
            if has_rooms {
                room_attachments.push(walk.room_attachment(record, ship, &self.origins.uses));
            }
            if port.ports.is_empty() {
                walk.restore(parent);
            } else {
                stack.push(Frame {
                    ports: port.ports.iter().enumerate(),
                    parent,
                });
            }
        }
        Ok(())
    }

    fn emit(
        &mut self,
        ship: &ShipContext,
        path: &Path,
        attachment: Option<(&LoadoutPort, &AttachmentIdentity)>,
        configurations: Vec<FacilityConfiguration>,
    ) -> Result<(), CatalogError> {
        if configurations.is_empty() {
            return Ok(());
        }
        let (original_port, attached_item) = if let Some((port, attachment)) = attachment {
            (
                port.source_constraints.physical_port()?,
                Some(attachment.clone()),
            )
        } else {
            (None, None)
        };
        let source = FacilityPlacementSource::Loadout(LoadoutFacilityPlacement {
            position: path.position_cell(&self.origins.uses),
            original_port,
            attached_item,
        });
        self.rows.push(FacilityInstance {
            ship: ship.clone(),
            source,
            configurations,
        });
        Ok(())
    }

    fn configuration_links(
        &mut self,
        record: u32,
    ) -> Result<Vec<FacilityConfiguration>, CatalogError> {
        match &mut self.definitions {
            Definitions::Indexed(index) => match index.get(&record) {
                Some(definitions) => link_definitions(definitions, self.cancel),
                None => Ok(Vec::new()),
            },
            Definitions::Scoped(lookup) => lookup(record),
        }
    }

    fn check_cancel(&self) -> Result<(), CatalogError> {
        if self.cancel.is_some_and(CancelToken::is_cancelled) {
            return Err(CatalogError::Cancelled);
        }
        Ok(())
    }
}
