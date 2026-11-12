// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};

pub(super) const CLASS_A: u128 = 0x1086_0001;
pub(super) const CLASS_B: u128 = 0x1086_0002;
pub(super) const FUNCTION: u128 = 0x1086_0003;
pub(super) const SIZE: u128 = 0x1086_0004;
pub(super) const CLASSES: u128 = 0x1086_0005;
pub(super) const FUNCTIONS: u128 = 0x1086_0006;
pub(super) const SIZES: u128 = 0x1086_0007;
pub(super) const MISSING: u128 = 0x1086_00ff;

const CLASS: &str = "MissionLocationClass";
const FUNCTION_TYPE: &str = "MissionLocationFunction";
const SIZE_TYPE: &str = "MissionLocationSize";
const CLASSES_TYPE: &str = "MissionLocationClasses";
const FUNCTIONS_TYPE: &str = "MissionLocationFunctions";
const SIZES_TYPE: &str = "MissionLocationSizes";
const SCOPE: &str = "ContractScope";
pub(super) const LOCAL: &str = "FixtureLocal";
pub(super) const FUTURE: &str = "FixtureFutureScope";

pub(super) fn database() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .enumeration(SCOPE, [LOCAL, FUTURE])
        .structure(
            TestStruct::new(CLASS).array("scopesVisibleTo", TestType::Enum(SCOPE.to_owned())),
        )
        .structure(TestStruct::new(FUNCTION_TYPE))
        .structure(TestStruct::new(SIZE_TYPE))
        .structure(registry(CLASSES_TYPE, "locationClasses", CLASS))
        .structure(registry(FUNCTIONS_TYPE, "locationFunctions", FUNCTION_TYPE))
        .structure(registry(SIZES_TYPE, "locationSizes", SIZE_TYPE))
        .record(record(CLASS_B, CLASS, "classes/second").value(
            "scopesVisibleTo",
            TestValue::Array(vec![TestValue::text(FUTURE)]),
        ))
        .record(record(CLASS_A, CLASS, "classes/first").value(
            "scopesVisibleTo",
            TestValue::Array([LOCAL, LOCAL, FUTURE].map(TestValue::text).to_vec()),
        ))
        .record(record(FUNCTION, FUNCTION_TYPE, "functions/empty"))
        .record(record(SIZE, SIZE_TYPE, "sizes/empty"))
        .record(record(CLASSES, CLASSES_TYPE, "registries/classes").value(
            "locationClasses",
            TestValue::Array(vec![
                TestValue::Reference(CLASS_B),
                TestValue::Reference(MISSING),
                TestValue::Reference(CLASS_A),
                TestValue::Reference(CLASS_A),
                TestValue::Null,
            ]),
        ))
        .record(
            record(FUNCTIONS, FUNCTIONS_TYPE, "registries/functions")
                .value("locationFunctions", TestValue::Array(Vec::new())),
        )
        .record(record(SIZES, SIZES_TYPE, "registries/sizes").value(
            "locationSizes",
            TestValue::Array(vec![TestValue::Reference(SIZE)]),
        ))
}

fn registry(source_type: &str, field: &str, member_type: &str) -> TestStruct {
    TestStruct::new(source_type).array(field, TestType::ReferenceTo(member_type.to_owned()))
}

fn record(guid: u128, source_type: &str, path: &str) -> TestRecord {
    TestRecord::new(
        guid,
        source_type,
        format!("{source_type}.Fixture"),
        format!("libs/foundry/records/fixture_location/{path}.xml"),
    )
}
