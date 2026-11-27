// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

pub(super) const OWNER: u128 = 0x1125_0404_0001;
pub(super) const TARGET: u128 = 0x1125_0404_0002;
pub(super) const MISSING: u128 = 0x1125_0404_0003;
const ROOT: &str = "MissionLocationTemplate";
const DATA: &str = "MissionLocationData";
const HOLDER: &str = "MissionStringVariants";
const NAME: &str = "MissionStringVariant";
const CLUSTER: &str = "FixtureCluster";
const OTHER: &str = "FixtureOther";

#[derive(Clone, Copy, Debug)]
pub(super) enum Case {
    Sparse,
    Present,
    Dangling,
    Unexpected,
    InvalidFlag,
    NameSlots,
    EmptyNames,
    NullNames,
    NullData,
}

pub(super) const ALL: [Case; 9] = [
    Case::Sparse,
    Case::Present,
    Case::Dangling,
    Case::Unexpected,
    Case::InvalidFlag,
    Case::NameSlots,
    Case::EmptyNames,
    Case::NullNames,
    Case::NullData,
];

pub(super) fn database(case: Case) -> DataForgeBuilder {
    let root_type = if matches!(case, Case::NullData) {
        TestType::StrongPointer(DATA.to_owned())
    } else {
        TestType::Class(DATA.to_owned())
    };
    let mut source = DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new(ROOT).field("locationData", root_type))
        .structure(location_fields(case))
        .structure(TestStruct::new(CLUSTER))
        .structure(TestStruct::new(OTHER))
        .structure(
            TestStruct::new(HOLDER).array("variants", TestType::StrongPointer(NAME.to_owned())),
        )
        .structure(
            TestStruct::new(NAME)
                .field("tag", TestType::Reference)
                .field("string", TestType::Locale),
        )
        .record(owner("Fixture", case, TestValue::Bool(false)));
    let target_type = if matches!(case, Case::Unexpected) {
        OTHER
    } else {
        CLUSTER
    };
    source = source.record(TestRecord::new(
        TARGET,
        target_type,
        format!("{target_type}.Target"),
        "libs/foundry/records/fixture/cluster.xml",
    ));
    source
}

pub(super) fn duplicated() -> DataForgeBuilder {
    database(Case::Present).record(owner("Repeated", Case::Present, TestValue::Bool(true)))
}

fn location_fields(case: Case) -> TestStruct {
    let source = TestStruct::new(DATA);
    if matches!(case, Case::Sparse) {
        return source;
    }
    let boolean = if matches!(case, Case::InvalidFlag) {
        TestType::String
    } else {
        TestType::Bool
    };
    source
        .field("disabled", boolean)
        .field("isSecurityNetworkHost", TestType::Bool)
        .field(
            "entityClusterMember",
            TestType::ReferenceTo(CLUSTER.to_owned()),
        )
        .field("stringVariants", TestType::StrongPointer(HOLDER.to_owned()))
}

fn owner(name: &str, case: Case, disabled: TestValue) -> TestRecord {
    let value = if matches!(case, Case::NullData) {
        TestValue::Null
    } else {
        TestValue::Struct(location_data(case, disabled))
    };
    TestRecord::new(
        OWNER,
        ROOT,
        format!("{ROOT}.{name}"),
        format!("libs/foundry/records/missiondata/pu_locations/fixture/{name}.xml"),
    )
    .value("locationData", value)
}

fn location_data(case: Case, disabled: TestValue) -> TestInstance {
    let source = TestInstance::new(DATA);
    if matches!(case, Case::Sparse) {
        return source;
    }
    let disabled = if matches!(case, Case::InvalidFlag) {
        TestValue::text("false")
    } else {
        disabled
    };
    let target = if matches!(case, Case::Dangling) {
        MISSING
    } else {
        TARGET
    };
    source
        .value("disabled", disabled)
        .value("isSecurityNetworkHost", TestValue::Bool(false))
        .value("entityClusterMember", TestValue::Reference(target))
        .value("stringVariants", names(case))
}

fn names(case: Case) -> TestValue {
    if matches!(case, Case::NullNames) {
        return TestValue::Null;
    }
    let name = || {
        TestValue::Struct(
            TestInstance::new(NAME)
                .value("tag", TestValue::Null)
                .value("string", TestValue::text("@location_fixture")),
        )
    };
    let rows = match case {
        Case::EmptyNames => Vec::new(),
        Case::NameSlots => {
            let mut invalid = (u32::MAX - 1).to_le_bytes().to_vec();
            invalid.extend(0_u32.to_le_bytes());
            vec![name(), TestValue::Null, name(), TestValue::Raw(invalid)]
        }
        _ => vec![name(), name()],
    };
    TestValue::Struct(TestInstance::new(HOLDER).value("variants", TestValue::Array(rows)))
}
