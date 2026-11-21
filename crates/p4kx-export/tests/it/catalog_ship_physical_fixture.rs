// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::Archive;
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestInstance, TestRecord, TestStruct, TestType,
    TestValue,
};

use crate::commodity_v3_fixture::TestResult;

pub(super) const SHARED_GUID: u128 = 0x9600_0000_0000_4000_8000_0000_0000_0001;

pub(super) struct Case {
    pub(super) class: &'static str,
    pub(super) crew: i32,
    pub(super) definition: &'static str,
    pub(super) restore: bool,
    pub(super) insured_class: &'static str,
    pub(super) wait: f32,
    pub(super) mandatory: f32,
    pub(super) fee: u32,
}

pub(super) const CASES: [Case; 2] = [
    Case {
        class: "PhysicalShipFirst",
        crew: 2,
        definition: "",
        restore: true,
        insured_class: "FirstShip",
        wait: 5.83,
        mandatory: 1.94,
        fee: 2335,
    },
    Case {
        class: "PhysicalShipSecond",
        crew: -7,
        definition: "  ",
        restore: false,
        insured_class: "",
        wait: -0.0,
        mandatory: 0.0,
        fee: u32::MAX,
    },
];

pub(super) fn open(path: &Path) -> TestResult<Archive> {
    let mut source = declarations();
    for case in &CASES {
        source = source.record(record(case));
    }
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("Data/Game2.dcb", source.build()).encrypted())
        .write_to(path)?;
    Ok(Archive::open_with_key(
        path,
        Some(p4kx_testkit::test_key()),
    )?)
}

fn declarations() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("Component"))
        .structure(
            TestStruct::new("VehicleComponentParams")
                .parent("Component")
                .field("crewSize", TestType::Int32)
                .field("vehicleDefinition", TestType::String),
        )
        .structure(TestStruct::new("StaticEntityClassData"))
        .structure(
            TestStruct::new("ShipInsuranceParams")
                .field("shipEntityClassName", TestType::String)
                .field("baseWaitTimeMinutes", TestType::Single)
                .field("mandatoryWaitTimeMinutes", TestType::Single)
                .field("baseExpeditingFee", TestType::UInt32),
        )
        .structure(
            TestStruct::new("SEntityInsuranceProperties")
                .parent("StaticEntityClassData")
                .field("restoreOnClaim", TestType::Bool)
                .field(
                    "shipInsuranceParams",
                    TestType::Class("ShipInsuranceParams".to_owned()),
                ),
        )
        .structure(
            TestStruct::new("EntityClassDefinition")
                .array(
                    "Components",
                    TestType::StrongPointer("Component".to_owned()),
                )
                .array(
                    "StaticEntityClassData",
                    TestType::StrongPointer("StaticEntityClassData".to_owned()),
                ),
        )
}

fn record(case: &Case) -> TestRecord {
    let vehicle = TestInstance::new("VehicleComponentParams")
        .value("crewSize", TestValue::Int32(case.crew))
        .value("vehicleDefinition", TestValue::text(case.definition));
    let params = TestInstance::new("ShipInsuranceParams")
        .value("shipEntityClassName", TestValue::text(case.insured_class))
        .value("baseWaitTimeMinutes", TestValue::Single(case.wait))
        .value(
            "mandatoryWaitTimeMinutes",
            TestValue::Single(case.mandatory),
        )
        .value("baseExpeditingFee", TestValue::UInt32(case.fee));
    let insurance = TestInstance::new("SEntityInsuranceProperties")
        .value("restoreOnClaim", TestValue::Bool(case.restore))
        .value("shipInsuranceParams", TestValue::Struct(params));
    TestRecord::new(
        SHARED_GUID,
        "EntityClassDefinition",
        format!("EntityClassDefinition.{}", case.class),
        "libs/foundry/records/entities/spaceships/physical_owner.xml",
    )
    .value(
        "Components",
        TestValue::Array(vec![TestValue::Struct(vehicle)]),
    )
    .value(
        "StaticEntityClassData",
        TestValue::Array(vec![TestValue::Struct(insurance)]),
    )
}
