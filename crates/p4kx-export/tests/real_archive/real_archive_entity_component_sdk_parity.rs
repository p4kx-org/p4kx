// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::{
    CatalogRecordSelection, DatasetAvailability, ParsedCatalogQueries,
};

use super::component_family_parity::{self, PhysicalComponentFamily, QueryWitness};
use crate::real_archive_catalog_table_parity::TestResult;

#[derive(Clone, Copy)]
enum Family {
    AnimationControllers,
    Explosives,
    FlightControllers,
    Ifcs,
    LootGeneration,
    Thrusters,
}

impl PhysicalComponentFamily for Family {
    const DOMAIN: &'static str = "Entity component";

    fn path(self) -> &'static str {
        match self {
            Self::AnimationControllers => "/configurations/entityAnimationControllers",
            Self::Explosives => "/configurations/explosives",
            Self::FlightControllers => "/configurations/flightControllers",
            Self::Ifcs => "/configurations/ifcs",
            Self::LootGeneration => "/configurations/lootGeneration",
            Self::Thrusters => "/configurations/thrusters",
        }
    }

    fn query(
        self,
        parsed: &ParsedCatalogQueries<'_>,
        witness: QueryWitness<'_>,
    ) -> TestResult<DatasetAvailability> {
        let selection = CatalogRecordSelection::All;
        match self {
            Self::AnimationControllers => {
                parsed.with_entity_animation_controllers(selection, None, |snapshot| {
                    witness.inspect(&snapshot, |row| row.owner())
                })?
            }
            Self::Explosives => parsed.with_explosives(selection, None, |snapshot| {
                witness.inspect(&snapshot, |row| row.owner())
            })?,
            Self::FlightControllers => {
                parsed.with_flight_controllers(selection, None, |snapshot| {
                    witness.inspect(&snapshot, |row| row.owner())
                })?
            }
            Self::Ifcs => parsed.with_ifcs(selection, None, |snapshot| {
                witness.inspect(&snapshot, |row| row.owner())
            })?,
            Self::LootGeneration => {
                parsed.with_loot_generation_components(selection, None, |snapshot| {
                    witness.inspect(&snapshot, |row| row.owner())
                })?
            }
            Self::Thrusters => parsed.with_thrusters(selection, None, |snapshot| {
                witness.inspect(&snapshot, |row| row.owner())
            })?,
        }
    }
}

const FAMILIES: [Family; 6] = [
    Family::AnimationControllers,
    Family::Explosives,
    Family::FlightControllers,
    Family::Ifcs,
    Family::LootGeneration,
    Family::Thrusters,
];

#[test]
#[ignore = "same Build: configure the installed Archive and exact completed Normal producer"]
fn installed_entity_component_tables_match_one_complete_catalog() -> TestResult {
    component_family_parity::verify_retained(
        FAMILIES,
        "P4KX_ENTITY_COMPONENT_CATALOG_PRODUCER_REVISION",
    )
}

#[test]
#[ignore = "installed Archive: configure the expected Build and one exact dataset path"]
fn installed_entity_component_cold_query() -> TestResult {
    component_family_parity::verify_cold(FAMILIES, "P4KX_ENTITY_COMPONENT_SDK_COLD_DATASET")
}
