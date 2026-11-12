// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::iter::Enumerate;
use std::slice::Iter;

use p4kx_extraction::{CancelToken, DataForge, SourceUse, SourceUseRole, SourceValue};

use super::catalog_entity_configurations::EntityConfigurationRows;
use super::catalog_seat;
use super::facility_instance::{
    FacilityConfiguration, FacilityInstance, FacilityPlacementSource, LoadoutFacilityPlacement,
    ShipContext,
};
use super::inventory_owner_storage;
use super::item_component_owner::ComponentOwner;
use super::loadout_position::{self, LoadoutPosition};
use super::room_associations;
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_table::CatalogTable;
use crate::contract_loadout::{EntityKind, Loadout, LoadoutPort};
use crate::loadout::alignment::AttachmentIdentity;
use crate::record_header_field::RecordHeaderField;
use crate::{CatalogError, ExportAccountingError};

/// Links resolved ship Loadout occurrences to their physical configuration rows.
pub(super) fn table(
    forge: &DataForge<'_>,
    loadouts: &[Loadout],
    declarations: &EntityConfigurationRows<'_>,
    origins: &CatalogOrigins,
    cancel: Option<&CancelToken>,
) -> Result<CatalogTable<FacilityInstance>, CatalogError> {
    let mut project = Projection::new(declarations, origins, cancel)?;
    if !project.definitions.is_empty() {
        for (ordinal, loadout) in loadouts.iter().enumerate() {
            project.check_cancel()?;
            if loadout.entity_kind == EntityKind::Ship {
                project.loadout(loadout, loadout_position::ordinal(ordinal)?)?;
            }
        }
    }
    Ok(CatalogTable::new(
        forge,
        &[
            catalog_seat::COMPONENT,
            inventory_owner_storage::ITEM_COMPONENT,
            room_associations::VEHICLE,
        ],
        project.rows,
    ))
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

struct Projection<'rows, 'origin> {
    definitions: HashMap<u32, Vec<Definition<'rows>>>,
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
        let mut projection = Self {
            definitions: HashMap::new(),
            origins,
            cancel,
            rows: Vec::new(),
        };
        for definition in declarations
            .facility_owners()
            .map(Definition::Facility)
            .chain(declarations.storage_owners().map(Definition::Inventory))
        {
            projection.check_cancel()?;
            let (Some(record), Some(_)) = definition.owner().key() else {
                return Err(ExportAccountingError::ConflictingSourceDatum.into());
            };
            projection
                .definitions
                .entry(record)
                .or_default()
                .push(definition);
        }
        for definitions in projection.definitions.values_mut() {
            definitions.sort_by_key(|definition| definition.owner().key().1);
        }
        Ok(projection)
    }

    fn loadout(&mut self, loadout: &Loadout, ordinal: u32) -> Result<(), CatalogError> {
        let root = loadout.root_record_index;
        let record = self.origins.record(root);
        let ship = ShipContext {
            ship_guid: record.header_value(
                RecordHeaderField::Guid,
                SourceValue::Present(loadout.entity_guid),
            )?,
            ship_record_index: record
                .header_selector(RecordHeaderField::RecordIndex, SourceValue::Present(root))?,
        };
        let selector =
            record.observe_header(RecordHeaderField::RecordIndex, SourceUseRole::Selector)?;
        let mut walk = Path::new(ordinal, selector);
        self.emit(&ship, root, &walk, None)?;
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
            if port.ports.is_empty() && !self.definitions.contains_key(&record) {
                continue;
            }
            let parent = walk.checkpoint();
            walk.position
                .port_ordinals
                .push(loadout_position::ordinal(ordinal)?);
            walk.uses.extend_from_slice(&proof.source_uses);
            let name_complete = port.append_name_uses(&mut walk.uses)?;
            walk.complete &= proof.source_set_complete && name_complete;
            self.emit(&ship, record, &walk, Some((port, attachment)))?;
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
        record: u32,
        path: &Path,
        attachment: Option<(&LoadoutPort, &AttachmentIdentity)>,
    ) -> Result<(), CatalogError> {
        let Some(definitions) = self.definitions.get(&record) else {
            return Ok(());
        };
        let (original_port, attached_item) = if let Some((port, attachment)) = attachment {
            (
                port.source_constraints.physical_port()?,
                Some(attachment.clone()),
            )
        } else {
            (None, None)
        };
        let source = FacilityPlacementSource::Loadout(LoadoutFacilityPlacement {
            position: self.origins.uses.value(
                SourceValue::Present(path.position.clone()),
                &path.uses,
                path.complete,
            ),
            original_port,
            attached_item,
        });
        for definition in definitions {
            self.check_cancel()?;
            self.rows.push(FacilityInstance {
                ship: ship.clone(),
                source: source.clone(),
                configuration: definition.link(),
            });
        }
        Ok(())
    }

    fn check_cancel(&self) -> Result<(), CatalogError> {
        if self.cancel.is_some_and(CancelToken::is_cancelled) {
            return Err(CatalogError::Cancelled);
        }
        Ok(())
    }
}

struct Frame<'loadout> {
    ports: Enumerate<Iter<'loadout, LoadoutPort>>,
    parent: Checkpoint,
}

impl<'loadout> Frame<'loadout> {
    fn new(ports: &'loadout [LoadoutPort], path: &Path) -> Self {
        Self {
            ports: ports.iter().enumerate(),
            parent: path.checkpoint(),
        }
    }
}

struct Path {
    position: LoadoutPosition,
    uses: Vec<SourceUse>,
    complete: bool,
}

#[derive(Clone, Copy)]
struct Checkpoint {
    ports: usize,
    uses: usize,
    complete: bool,
}

impl Path {
    fn new(loadout_ordinal: u32, root: SourceUse) -> Self {
        Self {
            position: LoadoutPosition {
                loadout_ordinal,
                port_ordinals: Vec::new(),
            },
            uses: vec![root],
            complete: true,
        }
    }

    const fn checkpoint(&self) -> Checkpoint {
        Checkpoint {
            ports: self.position.port_ordinals.len(),
            uses: self.uses.len(),
            complete: self.complete,
        }
    }

    fn restore(&mut self, checkpoint: Checkpoint) {
        self.position.port_ordinals.truncate(checkpoint.ports);
        self.uses.truncate(checkpoint.uses);
        self.complete = checkpoint.complete;
    }
}
