// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::{
    CatalogRecordSelection, DatasetAvailability, ParsedCatalogQueries,
};

use super::component_family_parity::{self, PhysicalComponentFamily, QueryWitness};
use crate::real_archive_catalog_table_parity::TestResult;

#[derive(Clone, Copy)]
enum Family {
    AttachableModifiers,
    Clothing,
    Consumables,
    HackableDevices,
    HackingChips,
    ItemRadars,
    MiningLasers,
    QuantumInterdiction,
    SuitArmor,
}

impl PhysicalComponentFamily for Family {
    const DOMAIN: &'static str = "Item gameplay";

    fn path(self) -> &'static str {
        match self {
            Self::AttachableModifiers => "/configurations/attachableModifiers",
            Self::Clothing => "/configurations/clothing",
            Self::Consumables => "/configurations/consumables",
            Self::HackableDevices => "/configurations/hackableDevices",
            Self::HackingChips => "/configurations/hackingChips",
            Self::ItemRadars => "/configurations/itemRadars",
            Self::MiningLasers => "/configurations/miningLasers",
            Self::QuantumInterdiction => "/configurations/quantumInterdiction",
            Self::SuitArmor => "/configurations/suitArmor",
        }
    }

    fn query(
        self,
        parsed: &ParsedCatalogQueries<'_>,
        witness: QueryWitness<'_>,
    ) -> TestResult<DatasetAvailability> {
        let selection = CatalogRecordSelection::All;
        match self {
            Self::AttachableModifiers => {
                parsed.with_attachable_modifiers(selection, None, |snapshot| {
                    witness.inspect(&snapshot, |row| row.owner())
                })?
            }
            Self::Clothing => parsed.with_clothing(selection, None, |snapshot| {
                witness.inspect(&snapshot, |row| row.owner())
            })?,
            Self::Consumables => parsed.with_consumables(selection, None, |snapshot| {
                witness.inspect(&snapshot, |row| row.owner())
            })?,
            Self::HackableDevices => parsed.with_hackable_devices(selection, None, |snapshot| {
                witness.inspect(&snapshot, |row| row.owner())
            })?,
            Self::HackingChips => parsed.with_hacking_chips(selection, None, |snapshot| {
                witness.inspect(&snapshot, |row| row.owner())
            })?,
            Self::ItemRadars => parsed.with_item_radars(selection, None, |snapshot| {
                witness.inspect(&snapshot, |row| row.owner())
            })?,
            Self::MiningLasers => parsed.with_mining_lasers(selection, None, |snapshot| {
                witness.inspect(&snapshot, |row| row.owner())
            })?,
            Self::QuantumInterdiction => {
                parsed.with_quantum_interdiction_generators(selection, None, |snapshot| {
                    witness.inspect(&snapshot, |row| row.owner())
                })?
            }
            Self::SuitArmor => parsed.with_suit_armor(selection, None, |snapshot| {
                witness.inspect(&snapshot, |row| row.owner())
            })?,
        }
    }
}

const FAMILIES: [Family; 9] = [
    Family::AttachableModifiers,
    Family::Clothing,
    Family::Consumables,
    Family::HackableDevices,
    Family::HackingChips,
    Family::ItemRadars,
    Family::MiningLasers,
    Family::QuantumInterdiction,
    Family::SuitArmor,
];

#[test]
#[ignore = "same Build: configure the installed Archive and exact completed Normal producer"]
fn installed_item_gameplay_tables_match_one_complete_catalog() -> TestResult {
    component_family_parity::verify_retained(
        FAMILIES,
        "P4KX_ITEM_GAMEPLAY_CATALOG_PRODUCER_REVISION",
    )
}

#[test]
#[ignore = "installed Archive: configure the expected Build and one exact dataset path"]
fn installed_item_gameplay_cold_query() -> TestResult {
    component_family_parity::verify_cold(FAMILIES, "P4KX_ITEM_GAMEPLAY_SDK_COLD_DATASET")
}
