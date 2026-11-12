// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::DataForge;
use schemars::JsonSchema;
use serde::Serialize;

use super::catalog_mission_board::{COMPONENT, MissionBoard};
use super::catalog_mission_board_read::{self, MissionBoardRead};
use super::catalog_resource_network_configuration::ResourceNetworkConfiguration;
use super::catalog_resource_network_read::{self, ResourceNetworkRead};
use crate::catalog_number_capture::CapturedNumber;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_table::CatalogTable;
use crate::collected::Collected;
use crate::contract_recovery::{ItemRecoveryEconomy, ItemRecoveryOverrides};
use crate::recovery::{self, ItemRecoveryPolicy, Recovery};
use crate::schema_fields::every_field_required;
use crate::{CatalogError, CatalogOptions};

/// Independently scoped configurations with their physical owners.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct IndependentConfigurationTables<'source> {
    item_recovery_economy: CatalogTable<ItemRecoveryEconomy<'source>>,
    item_recovery_overrides: CatalogTable<ItemRecoveryOverrides>,
    item_recovery_policy: CatalogTable<ItemRecoveryPolicy<'source>>,
    resource_network_parameters: CatalogTable<ResourceNetworkConfiguration<'source>>,
    mission_boards: CatalogTable<MissionBoard<'source>>,
}

pub(crate) struct IndependentConfigurations<'source> {
    pub(crate) tables: IndependentConfigurationTables<'source>,
    pub(crate) recovery_numbers: Vec<CapturedNumber>,
    pub(crate) configuration_numbers: Vec<CapturedNumber>,
}

/// Collects source evidence and output rows for the global configuration tables.
pub(crate) fn read<'source>(
    forge: &'source DataForge<'source>,
    projection: &CatalogProjection<'_, 'source>,
    options: &CatalogOptions<'_>,
    collected: &mut Collected<'source>,
) -> Result<IndependentConfigurations<'source>, CatalogError> {
    let Recovery {
        economy: item_recovery_economy,
        overrides: item_recovery_overrides,
        policy: item_recovery_policy,
        numbers: recovery_numbers,
        failures,
        ledger,
    } = recovery::read_recovery(forge, projection, options.filter, options.cancel)?;
    collected.ledger.merge(ledger);
    collected.add_failures(failures);
    let ResourceNetworkRead {
        table: resource_network_parameters,
        numbers: mut configuration_numbers,
        failures,
        ledger,
    } = catalog_resource_network_read::read(
        forge,
        projection.original(),
        options.filter,
        options.cancel,
    )?;
    collected.ledger.merge(ledger);
    collected.add_failures(failures);
    let MissionBoardRead {
        rows,
        numbers,
        ledger,
        failures,
    } = catalog_mission_board_read::read(
        forge,
        projection.original(),
        options.filter,
        options.cancel,
    )?;
    collected.ledger.merge(ledger);
    collected.add_failures(failures);
    configuration_numbers.extend(numbers);
    Ok(IndependentConfigurations {
        tables: IndependentConfigurationTables {
            item_recovery_economy,
            item_recovery_overrides,
            item_recovery_policy,
            resource_network_parameters,
            mission_boards: CatalogTable::new(forge, &[COMPONENT], rows),
        },
        recovery_numbers,
        configuration_numbers,
    })
}
