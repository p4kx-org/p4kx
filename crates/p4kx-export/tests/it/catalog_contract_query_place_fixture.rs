// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

pub(super) const OWNER: u128 = 0x7100_0000_0000_4000_8000_0000_0000_0001;
const CONTRACT: u128 = 0x7200_0000_0000_4000_8000_0000_0000_0001;
pub(super) const LOCATION: u128 = 0x7300_0000_0000_4000_8000_0000_0000_0001;
pub(super) const OTHER_LOCATION: u128 = 0x7300_0000_0000_4000_8000_0000_0002;
pub(super) const LOCALITY: u128 = 0x7400_0000_0000_4000_8000_0000_0001;
const LOCALITY_TYPE: &str = "MissionLocality";
const OTHER_TYPE: &str = "SyntheticLocalityDecoy";
pub(super) const ACCEPTED_KEY: &str = "@canonical_place";
pub(super) const REJECTED_KEY: &str = "@excluded_place";

#[derive(Clone, Copy, Debug)]
pub(super) enum Case {
    OutsideOnly,
    OutsideShadow,
    LeftoverShadow,
}

pub(super) fn database(case: Case) -> Vec<u8> {
    let source = source().record(generator(
        TestInstance::new("ContractPrerequisite_Location")
            .value("locationAvailable", TestValue::Reference(LOCATION)),
    ));
    let source = match case {
        Case::OutsideOnly => source.record(object(
            LOCATION,
            "elsewhere/outside",
            "Outside",
            REJECTED_KEY,
        )),
        Case::OutsideShadow => source
            .record(object(LOCATION, "pu/accepted", "Accepted", ACCEPTED_KEY))
            .record(object(
                LOCATION,
                "elsewhere/outside",
                "Outside",
                REJECTED_KEY,
            )),
        Case::LeftoverShadow => source
            .record(object(LOCATION, "pu/accepted", "Accepted", ACCEPTED_KEY))
            .record(object(LOCATION, "pu/dev_place", "dev_place", REJECTED_KEY)),
    };
    source.build()
}

#[derive(Clone, Copy)]
pub(super) enum LocalityCase {
    SameFamily,
    ForeignFirst,
}

pub(super) fn locality_database(case: LocalityCase) -> Vec<u8> {
    let shape = |name| TestStruct::new(name).array("availableLocations", TestType::Reference);
    let first = match case {
        LocalityCase::SameFamily => LOCALITY_TYPE,
        LocalityCase::ForeignFirst => OTHER_TYPE,
    };
    source()
        .structure(shape(LOCALITY_TYPE))
        .structure(shape(OTHER_TYPE))
        .structure(
            TestStruct::new("ContractPrerequisite_Locality")
                .parent("ContractPrerequisite")
                .field("localityAvailable", TestType::Reference),
        )
        .record(generator(
            TestInstance::new("ContractPrerequisite_Locality")
                .value("localityAvailable", TestValue::Reference(LOCALITY)),
        ))
        .record(locality(first, "first", LOCATION))
        .record(locality(LOCALITY_TYPE, "later", OTHER_LOCATION))
        .record(object(LOCATION, "pu/accepted", "Accepted", ACCEPTED_KEY))
        .record(object(OTHER_LOCATION, "pu/other", "Other", REJECTED_KEY))
        .build()
}

fn locality(source_type: &str, name: &str, location: u128) -> TestRecord {
    TestRecord::new(
        LOCALITY,
        source_type,
        format!("{source_type}.{name}"),
        format!("libs/foundry/records/missiondata/pu_missionlocality/{name}.xml"),
    )
    .value(
        "availableLocations",
        TestValue::Array(vec![TestValue::Reference(location)]),
    )
}

fn source() -> DataForgeBuilder {
    schema().into_iter().fold(
        DataForgeBuilder::new().version(8),
        DataForgeBuilder::structure,
    )
}

fn schema() -> [TestStruct; 6] {
    [
        TestStruct::new("ContractGenerator").array("generators", pointer("Handler")),
        TestStruct::new("Handler").array("contracts", pointer("Contract")),
        TestStruct::new("Contract")
            .field("id", TestType::Guid)
            .field("debugName", TestType::String)
            .array("additionalPrerequisites", pointer("ContractPrerequisite")),
        TestStruct::new("ContractPrerequisite"),
        TestStruct::new("ContractPrerequisite_Location")
            .parent("ContractPrerequisite")
            .field("locationAvailable", TestType::Reference),
        TestStruct::new("StarMapObject").field("name", TestType::Locale),
    ]
}

fn pointer(name: &str) -> TestType {
    TestType::StrongPointer(name.to_owned())
}

fn generator(prerequisite: TestInstance) -> TestRecord {
    let contract = TestInstance::new("Contract")
        .value("id", TestValue::Guid(CONTRACT))
        .value("debugName", TestValue::text("Delivery"))
        .value(
            "additionalPrerequisites",
            TestValue::Array(vec![TestValue::Struct(prerequisite)]),
        );
    TestRecord::new(
        OWNER,
        "ContractGenerator",
        "ContractGenerator.Carrier",
        "libs/foundry/records/contracts/contractgenerator/carrier.xml",
    )
    .value(
        "generators",
        TestValue::Array(vec![TestValue::Struct(TestInstance::new("Handler").value(
            "contracts",
            TestValue::Array(vec![TestValue::Struct(contract)]),
        ))]),
    )
}

fn object(guid: u128, path: &str, name: &str, key: &str) -> TestRecord {
    TestRecord::new(
        guid,
        "StarMapObject",
        format!("StarMapObject.{name}"),
        format!("libs/foundry/records/starmap/{path}.xml"),
    )
    .value("name", TestValue::text(key))
}
