// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::{CancelToken, DataForge, SourceUse, SourceValueState, SourcedValue};

use super::{CONTAINER, ComponentOwner, ItemPortContainer};
use crate::CatalogError;
use crate::catalog_rows::staged::{OrderedComponentRows, StagedCatalogTable};
use crate::catalog_table::DatasetAvailability;

/// Stages complete port rows and retains the original facts used by later joins.
pub(crate) struct ItemPortStaging<'stage, 'source> {
    rows: OrderedComponentRows<'stage>,
    facts: Vec<PortContainerFacts<'source>>,
}

impl<'stage, 'source> ItemPortStaging<'stage, 'source> {
    pub(crate) const fn new(directory: &'stage Path, cancel: Option<&'stage CancelToken>) -> Self {
        Self {
            rows: OrderedComponentRows::new(directory, cancel),
            facts: Vec::new(),
        }
    }

    pub(super) fn append(&mut self, row: ItemPortContainer<'source>) -> Result<(), CatalogError> {
        self.rows.append(row.owner.key(), &row)?;
        self.facts.push(PortContainerFacts::retain(row));
        Ok(())
    }

    pub(super) fn finish(
        self,
        forge: &DataForge<'_>,
    ) -> Result<StagedItemPorts<'source>, CatalogError> {
        let table = self.rows.finish(forge, &[CONTAINER])?;
        Ok(StagedItemPorts {
            table,
            facts: self.facts,
        })
    }
}

pub(super) fn empty_availability(forge: &DataForge<'_>) -> DatasetAvailability {
    DatasetAvailability::from_source_declarations(forge, |name| name == CONTAINER)
}

pub(super) struct StagedItemPorts<'source> {
    table: StagedCatalogTable,
    facts: Vec<PortContainerFacts<'source>>,
}

impl<'source> StagedItemPorts<'source> {
    pub(super) fn facts(&self) -> &[PortContainerFacts<'source>] {
        &self.facts
    }

    pub(super) fn into_table(self) -> StagedCatalogTable {
        self.table
    }
}

pub(super) struct PortContainerFacts<'source> {
    pub(super) owner: ComponentOwner,
    pub(super) ports: Vec<PortSlotFacts<'source>>,
    pub(super) ports_use: Option<SourceUse>,
    pub(super) incomplete_names: bool,
}

pub(super) struct PortSlotFacts<'source> {
    pub(super) port_ordinal: SourcedValue<u32>,
    pub(super) name: Option<&'source str>,
    pub(super) name_use: Option<SourceUse>,
}

impl<'source> PortContainerFacts<'source> {
    fn retain(row: ItemPortContainer<'source>) -> Self {
        let mut incomplete_names = unreadable(row.ports.state());
        let mut ports = Vec::new();
        for cell in row.ports.value().into_iter().flatten() {
            incomplete_names |= unreadable(cell.state());
            if let Some(port) = cell.value() {
                incomplete_names |= unreadable(port.name.state());
                let name = port.name.value().copied();
                ports.push(PortSlotFacts {
                    port_ordinal: port.port_ordinal.clone(),
                    name,
                    name_use: name.and(port.name_use.as_ref()).cloned(),
                });
            }
        }
        Self {
            owner: row.owner,
            ports,
            ports_use: row.ports_use,
            incomplete_names,
        }
    }
}

const fn unreadable(state: SourceValueState) -> bool {
    matches!(
        state,
        SourceValueState::Unsupported | SourceValueState::Unreadable | SourceValueState::Invalid
    )
}

#[cfg(test)]
#[path = "item_port_staging_tests.rs"]
mod tests;
