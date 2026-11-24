// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::{
    CatalogRecordSelection, DatasetAvailability, ItemPortSnapshot, ParsedCatalogQueries,
};

use super::component_family_parity::{
    self, ComponentRows, PhysicalComponentFamily, QueryWitness, physical_slot,
};
use crate::real_archive_catalog_table_parity::TestResult;

#[derive(Clone, Copy)]
enum Family {
    ItemPorts,
    ItemBounds,
    AttachmentConstraints,
    InventoryConfigurations,
}

impl PhysicalComponentFamily for Family {
    const DOMAIN: &'static str = "Inventory and attachment components";

    fn path(self) -> &'static str {
        match self {
            Self::ItemPorts => "/configurations/itemPorts",
            Self::ItemBounds => "/configurations/itemBounds",
            Self::AttachmentConstraints => "/configurations/itemAttachmentConstraints",
            Self::InventoryConfigurations => "/configurations/inventoryConfigurations",
        }
    }

    fn query(
        self,
        parsed: &ParsedCatalogQueries<'_>,
        witness: QueryWitness<'_>,
    ) -> TestResult<DatasetAvailability> {
        let selection = CatalogRecordSelection::All;
        match self {
            Self::ItemPorts => parsed.with_item_ports(selection, None, |snapshot| {
                inspect_ports(witness, &snapshot)
            })?,
            Self::ItemBounds => parsed.with_item_bounds(selection, None, |snapshot| {
                witness.inspect_rows(&ComponentRows::equipment(&snapshot), |row| {
                    physical_slot(row.owner_record_index(), row.component_ordinal())
                })
            })?,
            Self::AttachmentConstraints => {
                parsed.with_attachment_constraints(selection, None, |snapshot| {
                    witness.inspect_rows(&ComponentRows::equipment(&snapshot), |row| {
                        physical_slot(row.owner_record_index(), row.component_ordinal())
                    })
                })?
            }
            Self::InventoryConfigurations => {
                parsed.with_inventory_configurations(selection, None, |snapshot| {
                    witness.inspect(&snapshot, |row| row.owner())
                })?
            }
        }
    }
}

fn inspect_ports(
    witness: QueryWitness<'_>,
    snapshot: &ItemPortSnapshot<'_>,
) -> TestResult<DatasetAvailability> {
    let rows = ComponentRows {
        build: snapshot.build.as_ref(),
        availability: snapshot.availability,
        rows: &snapshot.rows,
        coverage: &snapshot.coverage,
        inventory: &snapshot.source_inventory,
    };
    witness.inspect_rows(&rows, |row| {
        physical_slot(row.owner_record_index(), row.component_ordinal())
    })
}

const FAMILIES: [Family; 4] = [
    Family::ItemPorts,
    Family::ItemBounds,
    Family::AttachmentConstraints,
    Family::InventoryConfigurations,
];

#[test]
#[ignore = "same Build: configure the installed Archive and exact completed Normal producer"]
fn installed_inventory_component_tables_match_one_complete_catalog() -> TestResult {
    component_family_parity::verify_retained(
        FAMILIES,
        "P4KX_INVENTORY_COMPONENT_CATALOG_PRODUCER_REVISION",
    )
}

#[test]
#[ignore = "installed Archive: configure the expected Build and one exact dataset path"]
fn installed_inventory_component_cold_query() -> TestResult {
    component_family_parity::verify_cold(FAMILIES, "P4KX_INVENTORY_COMPONENT_SDK_COLD_DATASET")
}
