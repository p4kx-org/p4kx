// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::{
    CatalogRecordSelection, DatasetAvailability, ParsedCatalogQueries,
};

use super::component_family_parity::{self, PhysicalComponentFamily, QueryWitness};
use crate::real_archive_catalog_table_parity::TestResult;

#[derive(Clone, Copy)]
enum Family {
    Durability,
    Degradation,
    Distortion,
    Emp,
}

impl PhysicalComponentFamily for Family {
    const DOMAIN: &'static str = "Item condition";

    fn path(self) -> &'static str {
        match self {
            Self::Durability => "/configurations/durability",
            Self::Degradation => "/configurations/degradation",
            Self::Distortion => "/configurations/distortion",
            Self::Emp => "/configurations/emp",
        }
    }

    fn query(
        self,
        parsed: &ParsedCatalogQueries<'_>,
        witness: QueryWitness<'_>,
    ) -> TestResult<DatasetAvailability> {
        let selection = CatalogRecordSelection::All;
        match self {
            Self::Durability => parsed.with_durability(selection, None, |snapshot| {
                witness.inspect(&snapshot, |row| row.owner())
            })?,
            Self::Degradation => parsed.with_degradation(selection, None, |snapshot| {
                witness.inspect(&snapshot, |row| row.owner())
            })?,
            Self::Distortion => parsed.with_distortion(selection, None, |snapshot| {
                witness.inspect(&snapshot, |row| row.owner())
            })?,
            Self::Emp => parsed.with_emp(selection, None, |snapshot| {
                witness.inspect(&snapshot, |row| row.owner())
            })?,
        }
    }
}

const FAMILIES: [Family; 4] = [
    Family::Durability,
    Family::Degradation,
    Family::Distortion,
    Family::Emp,
];

#[test]
#[ignore = "same Build: configure the installed Archive and exact completed Normal producer"]
fn installed_item_condition_tables_match_one_complete_catalog() -> TestResult {
    component_family_parity::verify_retained(
        FAMILIES,
        "P4KX_ITEM_CONDITION_CATALOG_PRODUCER_REVISION",
    )
}

#[test]
#[ignore = "installed Archive: configure the expected Build and one exact dataset path"]
fn installed_item_condition_cold_query() -> TestResult {
    component_family_parity::verify_cold(FAMILIES, "P4KX_ITEM_CONDITION_SDK_COLD_DATASET")
}
