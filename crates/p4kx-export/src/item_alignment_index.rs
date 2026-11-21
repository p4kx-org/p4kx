// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use p4kx_assembly::PortDefinitionPosition;
use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::{ItemAlignmentRows, ItemAttachmentComponent, ItemPort, ItemPortContainer};
use crate::catalog_v3::item_component_owner::ComponentOwner;
use crate::schema_fields::every_field_required;
use crate::{CatalogError, ExportAccountingError};

#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "state",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
/// The link to an original physical port definition.
pub enum PortDefinitionLink {
    /// A link to a projected original declaration.
    Projected {
        #[serde(flatten)]
        /// The physical original configuration owner.
        owner: ComponentOwner,
        /// The physical original port occurrence.
        port_ordinal: SourcedValue<u32>,
    },
    /// The exact original declaration has no projected row.
    NotProjected,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(tag = "state", rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
/// The link to an original physical attachment definition.
pub enum AttachmentDefinitionLink {
    /// A link to a projected original declaration.
    Projected {
        #[serde(flatten)]
        /// The physical original configuration owner.
        owner: ComponentOwner,
    },
    /// The exact original declaration has no projected row.
    NotProjected,
    /// The resolved Entity has no authored attachable component.
    NotDeclared,
}

/// The coordinates of an original port in the canonical item-port table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct PhysicalPort {
    pub(crate) owner_record_index: u32,
    pub(crate) component_ordinal: u32,
    pub(crate) port_ordinal: u32,
}

impl PhysicalPort {
    /// Returns the Record containing the physical port.
    #[must_use]
    pub const fn owner_record_index(&self) -> u32 {
        self.owner_record_index
    }

    /// Returns the ordinal of the original port-container component.
    #[must_use]
    pub const fn component_ordinal(&self) -> u32 {
        self.component_ordinal
    }

    /// Returns the ordinal of the port within its original component.
    #[must_use]
    pub const fn port_ordinal(&self) -> u32 {
        self.port_ordinal
    }

    pub(super) fn read(
        owner: &ComponentOwner,
        port: &SourcedValue<u32>,
    ) -> Result<Self, CatalogError> {
        let (owner_record_index, component_ordinal) = physical_key(owner)?;
        let port_ordinal = port
            .value()
            .copied()
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        Ok(Self {
            owner_record_index,
            component_ordinal,
            port_ordinal,
        })
    }
}

impl AttachmentDefinitionLink {
    /// Resolves an exact physical link in configuration rows ordered by original component.
    pub(crate) fn from_original(
        rows: &[ItemAttachmentComponent<'_>],
        record: u32,
        component: usize,
    ) -> Result<Self, CatalogError> {
        let key = (Some(record), Some(ordinal(component)?));
        let Ok(row) = rows.binary_search_by_key(&key, |row| row.owner.key()) else {
            return Ok(Self::NotProjected);
        };
        let row = rows
            .get(row)
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        Ok(Self::projected(&row.owner))
    }

    fn projected(owner: &ComponentOwner) -> Self {
        Self::Projected {
            owner: owner.clone(),
        }
    }

    pub(crate) fn physical_key(&self) -> Result<Option<(u32, u32)>, CatalogError> {
        match self {
            Self::Projected { owner } => Ok(Some(physical_key(owner)?)),
            Self::NotProjected | Self::NotDeclared => Ok(None),
        }
    }
}

impl PortDefinitionLink {
    /// Resolves an exact physical link in configuration rows ordered by original component.
    pub(crate) fn from_original(
        rows: &[ItemPortContainer<'_>],
        position: PortDefinitionPosition,
    ) -> Result<Self, CatalogError> {
        let record = position.record_index();
        let component = ordinal(position.component_ordinal())?;
        let slot = ordinal(position.port_ordinal())?;
        let key = (Some(record), Some(component));
        let Ok(row) = rows.binary_search_by_key(&key, |row| row.owner.key()) else {
            return Ok(Self::NotProjected);
        };
        let row = rows
            .get(row)
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        let Some(port) = row
            .ports()
            .value()
            .and_then(|ports| ports.get(position.port_ordinal()))
            .and_then(SourcedValue::value)
        else {
            return Ok(Self::NotProjected);
        };
        if port.port_ordinal().value() != Some(&slot) {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        Ok(Self::projected(&row.owner, port.port_ordinal()))
    }

    fn projected(owner: &ComponentOwner, port_ordinal: &SourcedValue<u32>) -> Self {
        Self::Projected {
            owner: owner.clone(),
            port_ordinal: port_ordinal.clone(),
        }
    }

    pub(crate) fn physical_port(&self) -> Result<Option<PhysicalPort>, CatalogError> {
        match self {
            Self::Projected {
                owner,
                port_ordinal,
            } => PhysicalPort::read(owner, port_ordinal).map(Some),
            Self::NotProjected => Ok(None),
        }
    }
}

#[derive(Default)]
pub(crate) struct AlignmentIndex<'rows> {
    ports: HashMap<(u32, u32, u32), PortOwner<'rows>>,
    items: HashMap<(u32, u32), &'rows ComponentOwner>,
}

struct PortOwner<'rows> {
    component: &'rows ComponentOwner,
    port_ordinal: &'rows SourcedValue<u32>,
}

impl<'rows> AlignmentIndex<'rows> {
    pub(crate) fn new(rows: &'rows ItemAlignmentRows<'_>) -> Result<Self, CatalogError> {
        let mut this = Self::default();
        for row in &rows.ports {
            let ordinals = row
                .ports()
                .value()
                .into_iter()
                .flatten()
                .filter_map(SourcedValue::value)
                .map(ItemPort::port_ordinal);
            this.insert_ports(&row.owner, ordinals)?;
        }
        for row in rows.port_facts() {
            this.insert_ports(&row.owner, row.ports.iter().map(|port| &port.port_ordinal))?;
        }
        for row in &rows.attachments {
            let key = physical_key(&row.owner)?;
            if this.items.insert(key, &row.owner).is_some() {
                return Err(ExportAccountingError::ConflictingSourceDatum.into());
            }
        }
        Ok(this)
    }

    fn insert_ports(
        &mut self,
        owner: &'rows ComponentOwner,
        ordinals: impl Iterator<Item = &'rows SourcedValue<u32>>,
    ) -> Result<(), CatalogError> {
        let (record, component) = physical_key(owner)?;
        for port_ordinal in ordinals {
            let ordinal = port_ordinal
                .value()
                .copied()
                .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
            let source = PortOwner {
                component: owner,
                port_ordinal,
            };
            if self
                .ports
                .insert((record, component, ordinal), source)
                .is_some()
            {
                return Err(ExportAccountingError::ConflictingSourceDatum.into());
            }
        }
        Ok(())
    }

    pub(crate) fn port(
        &self,
        position: PortDefinitionPosition,
    ) -> Result<PortDefinitionLink, CatalogError> {
        let key = (
            position.record_index(),
            ordinal(position.component_ordinal())?,
            ordinal(position.port_ordinal())?,
        );
        Ok(self
            .ports
            .get(&key)
            .map_or(PortDefinitionLink::NotProjected, |found| {
                PortDefinitionLink::projected(found.component, found.port_ordinal)
            }))
    }

    pub(crate) fn attachment(
        &self,
        record: u32,
        component: Option<usize>,
    ) -> Result<AttachmentDefinitionLink, CatalogError> {
        let Some(component) = component else {
            return Ok(AttachmentDefinitionLink::NotDeclared);
        };
        Ok(self
            .items
            .get(&(record, ordinal(component)?))
            .map_or(AttachmentDefinitionLink::NotProjected, |owner| {
                AttachmentDefinitionLink::projected(owner)
            }))
    }
}

fn ordinal(value: usize) -> Result<u32, ExportAccountingError> {
    u32::try_from(value).map_err(|_| ExportAccountingError::CounterOverflow)
}

fn physical_key(owner: &ComponentOwner) -> Result<(u32, u32), ExportAccountingError> {
    let (record, component) = owner.key();
    record
        .zip(component)
        .ok_or(ExportAccountingError::ConflictingSourceDatum)
}
