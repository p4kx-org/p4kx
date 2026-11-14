// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::DataForge;
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use super::catalog_fuel::{self, FuelIntake, FuelTank};
use super::catalog_resource_container::{self, ResourceContainer};
use super::item_component_owner::ComponentOwner;
use crate::CatalogError;
use crate::catalog_table::CatalogTable;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;

#[derive(Default)]
pub(super) struct FuelConfigurationRows<'source> {
    tanks: Vec<FuelTank<'source>>,
    intakes: Vec<FuelIntake<'source>>,
    containers: Vec<ResourceContainer<'source>>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct FuelConfigurationTables<'source> {
    fuel_tanks: CatalogTable<FuelTank<'source>>,
    fuel_intakes: CatalogTable<FuelIntake<'source>>,
    resource_containers: CatalogTable<ResourceContainer<'source>>,
}

#[derive(Clone, Copy)]
pub(super) enum FuelConfigurationKind {
    Tank,
    Intake,
    Container,
}

impl FuelConfigurationKind {
    pub(super) fn of(name: &str) -> Option<Self> {
        match name {
            catalog_fuel::TANK => Some(Self::Tank),
            catalog_fuel::INTAKE => Some(Self::Intake),
            catalog_resource_container::RESOURCE_CONTAINER => Some(Self::Container),
            _ => None,
        }
    }
}

impl<'source> FuelConfigurationRows<'source> {
    pub(super) fn extend(&mut self, other: Self) {
        self.tanks.extend(other.tanks);
        self.intakes.extend(other.intakes);
        self.containers.extend(other.containers);
    }

    pub(super) fn into_tables(mut self, forge: &DataForge<'_>) -> FuelConfigurationTables<'source> {
        self.tanks.sort_by_key(|row| row.owner().key());
        self.intakes.sort_by_key(|row| row.owner().key());
        self.containers.sort_by_key(|row| row.owner().key());
        FuelConfigurationTables {
            fuel_tanks: CatalogTable::new(forge, &[catalog_fuel::TANK], self.tanks),
            fuel_intakes: CatalogTable::new(forge, &[catalog_fuel::INTAKE], self.intakes),
            resource_containers: CatalogTable::new(
                forge,
                &[catalog_resource_container::RESOURCE_CONTAINER],
                self.containers,
            ),
        }
    }

    pub(super) fn push(
        &mut self,
        kind: FuelConfigurationKind,
        owner: ComponentOwner,
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<(), CatalogError> {
        match kind {
            FuelConfigurationKind::Tank => self.tanks.push(FuelTank::read(owner, source, notes)?),
            FuelConfigurationKind::Intake => {
                self.intakes.push(FuelIntake::read(owner, source, notes)?);
            }
            FuelConfigurationKind::Container => self
                .containers
                .push(ResourceContainer::read(owner, source, notes)?),
        }
        Ok(())
    }
}
