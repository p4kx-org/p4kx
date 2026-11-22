// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::super::catalog_independent_configurations::flight_configuration_tables::{
    FlightConfigurationKind, FlightConfigurationTables, FlightOwner,
};
use super::super::facility_instance::{LoadoutFacilityPlacement, ShipContext};
use super::super::item_component_owner::ComponentOwner;
use super::super::loadout_position::LoadoutPosition;
use crate::contract_loadout::LoadoutPort;
use crate::loadout::alignment::AttachmentIdentity;
use crate::schema_fields::every_field_required;
use crate::{CatalogError, ExportAccountingError};

/// Original flight configurations on one resolved vehicle Loadout occurrence.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(in crate::catalog_v3) struct InstalledConfiguration {
    #[serde(flatten)]
    vehicle: ShipContext,
    source: LoadoutFacilityPlacement,
    #[schemars(length(min = 1))]
    configurations: Vec<FlightConfigurationLink>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[schemars(transform = every_field_required)]
struct FlightConfigurationLink {
    kind: FlightConfigurationKind,
    owner: ComponentOwner,
}

#[derive(Default)]
pub(super) struct InstalledConfigurations<'rows> {
    definitions: HashMap<u32, Vec<FlightOwner<'rows>>>,
    pub(super) rows: Vec<InstalledConfiguration>,
}

impl<'rows> InstalledConfigurations<'rows> {
    pub(super) fn new(tables: &'rows FlightConfigurationTables<'_>) -> Result<Self, CatalogError> {
        let mut definitions = HashMap::<u32, Vec<FlightOwner<'_>>>::new();
        for definition in tables.owners() {
            let (Some(record), Some(_)) = definition.owner.key() else {
                return Err(ExportAccountingError::ConflictingSourceDatum.into());
            };
            definitions.entry(record).or_default().push(definition);
        }
        for definitions in definitions.values_mut() {
            definitions.sort_by_key(|definition| definition.owner.key().1);
        }
        Ok(Self {
            definitions,
            rows: Vec::new(),
        })
    }

    pub(super) fn is_empty(&self) -> bool {
        self.definitions.is_empty()
    }

    pub(super) fn contains(&self, record: u32) -> bool {
        self.definitions.contains_key(&record)
    }

    pub(super) fn emit(
        &mut self,
        vehicle: &ShipContext,
        record: u32,
        position: impl FnOnce() -> SourcedValue<LoadoutPosition>,
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
        self.rows.push(InstalledConfiguration {
            vehicle: vehicle.clone(),
            source: LoadoutFacilityPlacement {
                position: position(),
                original_port,
                attached_item,
            },
            configurations: definitions
                .iter()
                .map(|definition| FlightConfigurationLink {
                    kind: definition.kind,
                    owner: definition.owner.clone(),
                })
                .collect(),
        });
        Ok(())
    }
}
