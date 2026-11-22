// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::DataForge;
use schemars::JsonSchema;
use serde::Serialize;

use crate::CatalogError;
use crate::catalog_table::CatalogTable;
use crate::catalog_v3::catalog_entity_component_read::{
    ComponentKind, catalog_flight_controller, catalog_ifcs, catalog_thruster,
};
use crate::catalog_v3::item_component_owner::ComponentOwner;
use crate::export_field_inspection::ConfigurationInspection;
use crate::schema_fields::every_field_required;

use super::PhysicalComponents;
use catalog_flight_controller::FlightController;
use catalog_ifcs::Ifcs;
use catalog_thruster::Thruster;

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct FlightConfigurationTables<'source> {
    flight_controllers: CatalogTable<FlightController<'source>>,
    ifcs: CatalogTable<Ifcs<'source>>,
    thrusters: CatalogTable<Thruster<'source>>,
}

pub(in crate::catalog_v3) const FLIGHT_CONTROLLERS: ComponentKind = ComponentKind {
    source_type: catalog_flight_controller::COMPONENT,
    inspection: ConfigurationInspection::FlightController,
};
pub(in crate::catalog_v3) const IFCS: ComponentKind = ComponentKind {
    source_type: catalog_ifcs::COMPONENT,
    inspection: ConfigurationInspection::Ifcs,
};
pub(in crate::catalog_v3) const THRUSTERS: ComponentKind = ComponentKind {
    source_type: catalog_thruster::COMPONENT,
    inspection: ConfigurationInspection::Thruster,
};
const SOURCE_TYPES: [&str; 3] = [
    FLIGHT_CONTROLLERS.source_type,
    IFCS.source_type,
    THRUSTERS.source_type,
];

impl<'source> FlightConfigurationTables<'source> {
    pub(super) fn read(
        components: &mut PhysicalComponents<'_, '_, 'source>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            flight_controllers: components.table(FLIGHT_CONTROLLERS, FlightController::read)?,
            ifcs: components.table(IFCS, Ifcs::read)?,
            thrusters: components.table(THRUSTERS, Thruster::read)?,
        })
    }

    pub(in crate::catalog_v3) fn owners(&self) -> impl Iterator<Item = FlightOwner<'_>> {
        self.flight_controllers
            .rows
            .iter()
            .map(|row| FlightOwner {
                kind: FlightConfigurationKind::FlightController,
                owner: row.owner(),
            })
            .chain(self.ifcs.rows.iter().map(|row| FlightOwner {
                kind: FlightConfigurationKind::Ifcs,
                owner: row.owner(),
            }))
            .chain(self.thrusters.rows.iter().map(|row| FlightOwner {
                kind: FlightConfigurationKind::Thruster,
                owner: row.owner(),
            }))
    }

    pub(in crate::catalog_v3) fn installed_table<T>(
        forge: &DataForge<'_>,
        rows: Vec<T>,
    ) -> CatalogTable<T> {
        CatalogTable::new(forge, &SOURCE_TYPES, rows)
    }
}

#[derive(Debug, Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(extend("x-p4kx-vocabularyClass" = "control"))]
#[non_exhaustive]
#[expect(
    missing_docs,
    reason = "variant descriptions would change the catalog schema representation"
)]
/// The original component family represented by the configuration.
#[schemars(description = "")]
pub enum FlightConfigurationKind {
    FlightController,
    Ifcs,
    Thruster,
}

pub(in crate::catalog_v3) struct FlightOwner<'rows> {
    pub(in crate::catalog_v3) kind: FlightConfigurationKind,
    pub(in crate::catalog_v3) owner: &'rows ComponentOwner,
}
