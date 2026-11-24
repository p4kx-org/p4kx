// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::CatalogRecordSelection;
use p4kx_export::catalog_queries::equipment::vehicle_armor;
use p4kx_extraction::Archive;
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntity, TestEntry, TestInstance, TestStruct, TestType,
    TestValue, entity_schema,
};

use super::super::VEHICLE_ARMOR;
use super::{Route, TestResult};

pub(super) fn read() -> TestResult<Route> {
    const COMPONENT: &str = "SCItemVehicleArmorParams";
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("armor_fixture.p4k");
    let database = entity_schema(DataForgeBuilder::new().version(8))
        .structure(
            TestStruct::new(COMPONENT)
                .parent("DataForgeComponentParams")
                .field("signalInfrared", TestType::Single),
        )
        .record(
            TestEntity::new(
                1,
                "FixtureArmor",
                "libs/foundry/records/entities/scitem/ships/armor/fixture.xml",
            )
            .attach_type("Misc")
            .component(TestValue::Struct(
                TestInstance::new(COMPONENT).value("signalInfrared", TestValue::Single(1.0)),
            ))
            .record(),
        )
        .build();
    ArchiveBuilder::new()
        .entry(TestEntry::stored(r"Data\Game2.dcb", database))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    vehicle_armor::with_vehicle_armor(&archive, CatalogRecordSelection::All, None, |snapshot| {
        Route::queried_rows(
            VEHICLE_ARMOR,
            snapshot.rows(),
            "selected installed original-field witness available; complete retained Normal parity and cold SDK measurement remain open",
        )
    })?
}
