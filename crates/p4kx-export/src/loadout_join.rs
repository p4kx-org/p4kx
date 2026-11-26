// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, SourceUse, SourcedValue};

use super::alignment::LoadoutAlignment;
use crate::CatalogError;
use crate::catalog_read::original::AuthoredEntryPath;
use crate::catalog_rows::spool::{RowSpoolError, checkpoint};
use crate::contract_loadout::{Loadout, LoadoutPort};
use crate::guid_text::CatalogGuid;
use crate::loadout_owner_value::LoadoutOwnerValue;

/// The fields needed by Ship Loadout joins.
pub(crate) struct LoadoutJoin<'rows, Port> {
    pub(crate) record_index: u32,
    pub(crate) source_record_index: &'rows SourcedValue<u32>,
    pub(crate) entity_guid: &'rows LoadoutOwnerValue<CatalogGuid>,
    pub(crate) port_inventory_complete: bool,
    pub(crate) ports: &'rows [Port],
}

impl<'rows> From<&'rows Loadout> for LoadoutJoin<'rows, LoadoutPort> {
    fn from(loadout: &'rows Loadout) -> Self {
        Self {
            record_index: loadout.root_record_index,
            source_record_index: &loadout.source_record_index,
            entity_guid: &loadout.entity_guid,
            port_inventory_complete: loadout.port_inventory_complete,
            ports: &loadout.ports,
        }
    }
}

/// The typed join inputs retained after staging a Ship's complete wire row.
#[derive(Debug)]
pub(crate) struct ShipLoadoutJoin {
    record_index: u32,
    source_record_index: SourcedValue<u32>,
    entity_guid: LoadoutOwnerValue<CatalogGuid>,
    port_inventory_complete: bool,
    ports: Vec<ShipPortJoin>,
}

impl ShipLoadoutJoin {
    pub(crate) fn view(&self) -> LoadoutJoin<'_, ShipPortJoin> {
        LoadoutJoin {
            record_index: self.record_index,
            source_record_index: &self.source_record_index,
            entity_guid: &self.entity_guid,
            port_inventory_complete: self.port_inventory_complete,
            ports: &self.ports,
        }
    }

    pub(crate) fn retain(
        loadout: Loadout,
        cancel: Option<&CancelToken>,
    ) -> Result<Self, RowSpoolError> {
        Ok(Self {
            record_index: loadout.root_record_index,
            source_record_index: loadout.source_record_index,
            entity_guid: loadout.entity_guid,
            port_inventory_complete: loadout.port_inventory_complete,
            ports: retain_ports(loadout.ports, cancel)?,
        })
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct PortJoinState {
    pub(crate) attachment_port_inventory_complete: bool,
    pub(crate) docked_vehicle: bool,
    pub(crate) unresolved: bool,
    item_selector: ItemSelector,
}

#[derive(Debug, Clone, Copy)]
enum ItemSelector {
    Present,
    Absent,
}

impl PortJoinState {
    pub(crate) const fn has_item_selector(self) -> bool {
        matches!(self.item_selector, ItemSelector::Present)
    }
}

impl From<&LoadoutPort> for PortJoinState {
    fn from(port: &LoadoutPort) -> Self {
        Self {
            attachment_port_inventory_complete: port.attachment_port_inventory_complete,
            docked_vehicle: port.docked_vehicle,
            unresolved: port.unresolved.is_some(),
            item_selector: if port.item_guid.is_some() || port.item_class_name.is_some() {
                ItemSelector::Present
            } else {
                ItemSelector::Absent
            },
        }
    }
}

/// A physical port used to join a resolved Loadout to its original definitions.
pub(crate) trait JoinPort: Sized {
    fn name_value(&self) -> Option<&str>;
    fn name_proof(&self) -> Option<&AuthoredEntryPath>;
    fn alignment(&self) -> &LoadoutAlignment;
    fn state(&self) -> PortJoinState;
    fn children(&self) -> &[Self];

    fn append_name_uses(&self, uses: &mut Vec<SourceUse>) -> Result<bool, CatalogError> {
        let Some(proof) = self.name_proof() else {
            return Ok(false);
        };
        let (original, complete) = proof.selector_inputs()?;
        uses.extend(original);
        Ok(complete)
    }
}

impl JoinPort for LoadoutPort {
    fn name_value(&self) -> Option<&str> {
        self.name.value().map(String::as_str)
    }

    fn name_proof(&self) -> Option<&AuthoredEntryPath> {
        self.name_proof.as_ref()
    }

    fn alignment(&self) -> &LoadoutAlignment {
        &self.source_constraints
    }

    fn state(&self) -> PortJoinState {
        self.into()
    }

    fn children(&self) -> &[Self] {
        &self.ports
    }
}

#[derive(Debug)]
pub(crate) struct ShipPortJoin {
    name: RetainedPortName,
    alignment: LoadoutAlignment,
    state: PortJoinState,
    children: Vec<Self>,
}

#[derive(Debug)]
enum RetainedPortName {
    Authored(AuthoredEntryPath),
    Other {
        value: Option<String>,
        proof: Option<AuthoredEntryPath>,
    },
}

impl ShipPortJoin {
    fn retain(port: LoadoutPort, cancel: Option<&CancelToken>) -> Result<Self, RowSpoolError> {
        let state = (&port).into();
        let name = match (port.name.into_value(), port.name_proof) {
            (Some(value), Some(proof)) if value == proof.as_str() => {
                RetainedPortName::Authored(proof)
            }
            (value, proof) => RetainedPortName::Other { value, proof },
        };
        Ok(Self {
            name,
            alignment: port.source_constraints,
            state,
            children: retain_ports(port.ports, cancel)?,
        })
    }
}

fn retain_ports(
    ports: Vec<LoadoutPort>,
    cancel: Option<&CancelToken>,
) -> Result<Vec<ShipPortJoin>, RowSpoolError> {
    checkpoint(cancel)?;
    // The retained tree must use the smaller port layout's allocation.
    let mut retained = Vec::with_capacity(ports.len());
    for port in ports {
        checkpoint(cancel)?;
        retained.push(ShipPortJoin::retain(port, cancel)?);
    }
    Ok(retained)
}

impl JoinPort for ShipPortJoin {
    fn name_value(&self) -> Option<&str> {
        match &self.name {
            RetainedPortName::Authored(proof) => Some(proof.as_str()),
            RetainedPortName::Other { value, .. } => value.as_deref(),
        }
    }

    fn name_proof(&self) -> Option<&AuthoredEntryPath> {
        match &self.name {
            RetainedPortName::Authored(proof) => Some(proof),
            RetainedPortName::Other { proof, .. } => proof.as_ref(),
        }
    }

    fn alignment(&self) -> &LoadoutAlignment {
        &self.alignment
    }

    fn state(&self) -> PortJoinState {
        self.state
    }

    fn children(&self) -> &[Self] {
        &self.children
    }
}
