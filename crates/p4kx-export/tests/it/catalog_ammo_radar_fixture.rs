// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

const TAG: u128 = 0x8210;
const MISSING: u128 = 0x8211;
const OTHER: u128 = 0x8212;
pub(super) const NAME_KEY: &str = "@fixture_scan_name";
pub(super) const TEXT_KEY: &str = "@fixture_scan_value";

pub(super) fn database(detection_target: &str) -> DataForgeBuilder {
    let core = DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new("AmmoParams")
                .field("radarObjectParams", pointer("SSCSignatureSystemParams")),
        )
        .structure(
            TestStruct::new("SSCSignatureSystemParams")
                .field("bindingURLPrefix", TestType::String)
                .array(
                    "detectionTags",
                    TestType::ReferenceTo(detection_target.to_owned()),
                )
                .field("radarProperties", pointer("SSCRadarContactProperites"))
                .field("embeddedScanInfo", pointer("ScanCustomDataInfo")),
        )
        .structure(TestStruct::new("Tag"))
        .structure(TestStruct::new("FixtureWrongTarget"));
    let database = scan_declarations(signature_declarations(core));
    database
        .record(
            TestRecord::new(
                0x8213,
                "AmmoParams",
                "AmmoParams.RadarFixture",
                "fixture/ammo_radar.xml",
            )
            .value("radarObjectParams", radar_value()),
        )
        .record(TestRecord::new(
            TAG,
            "Tag",
            "Tag.Fixture",
            "fixture/radar_tag.xml",
        ))
        .record(TestRecord::new(
            OTHER,
            "FixtureWrongTarget",
            "Other.Fixture",
            "fixture/radar_wrong.xml",
        ))
}

fn signature_declarations(source: DataForgeBuilder) -> DataForgeBuilder {
    source
        .structure(
            TestStruct::new("SSCRadarContactProperites")
                .field("baseSignatureParams", pointer("SSCSignatureParamsBase"))
                .field(
                    "crossSectionParams",
                    pointer("SSCSignatureSystemCrossSectionParams"),
                ),
        )
        .structure(TestStruct::new("SSCSignatureParamsBase"))
        .structure(
            TestStruct::new("SSCSignatureSystemBaseSignatureParams")
                .parent("SSCSignatureParamsBase")
                .array("signatures", TestType::Single),
        )
        .structure(TestStruct::new("SSCSignatureSystemCrossSectionParams"))
        .structure(
            TestStruct::new("SSCSignatureSystemManualCrossSectionParams")
                .parent("SSCSignatureSystemCrossSectionParams")
                .field("crossSection", class("Vec3")),
        )
        .structure(TestStruct::new("Vec3").field("x", TestType::Single))
}

fn scan_declarations(source: DataForgeBuilder) -> DataForgeBuilder {
    source
        .structure(
            TestStruct::new("ScanCustomDataInfo")
                .array("scanProcedures", class("CustomScanProcedureParams")),
        )
        .structure(
            TestStruct::new("ScanProcedureParams").field("allowedInAIAutoScan", TestType::Bool),
        )
        .structure(
            TestStruct::new("CustomScanProcedureParams")
                .parent("ScanProcedureParams")
                .array("customDatas", pointer("ScanCustomData")),
        )
        .structure(TestStruct::new("ScanCustomData").field("name", TestType::Locale))
        .structure(
            TestStruct::new("ScanCustomDataDirect")
                .parent("ScanCustomData")
                .field("value", pointer("ScanCustomValue")),
        )
        .structure(TestStruct::new("ScanCustomValue"))
        .structure(
            TestStruct::new("ScanCustomValueArray")
                .parent("ScanCustomValue")
                .array("value", pointer("ScanCustomValue")),
        )
        .structure(
            TestStruct::new("ScanCustomValueFloat")
                .parent("ScanCustomValue")
                .field("value", TestType::Single),
        )
        .structure(
            TestStruct::new("ScanCustomValueLocString")
                .parent("ScanCustomValue")
                .field("value", TestType::Locale),
        )
}

fn radar_value() -> TestValue {
    let base = TestInstance::new("SSCSignatureSystemBaseSignatureParams").value(
        "signatures",
        TestValue::Array(vec![
            TestValue::Single(-0.0),
            TestValue::Single(f32::from_bits(0x7fc0_1234)),
        ]),
    );
    let cross = TestInstance::new("SSCSignatureSystemManualCrossSectionParams").value(
        "crossSection",
        node(TestInstance::new("Vec3").value("x", TestValue::Single(1.0))),
    );
    let contact = TestInstance::new("SSCRadarContactProperites")
        .value("baseSignatureParams", node(base))
        .value("crossSectionParams", node(cross));
    node(
        TestInstance::new("SSCSignatureSystemParams")
            .value("bindingURLPrefix", TestValue::text("fixture:"))
            .value(
                "detectionTags",
                TestValue::Array(vec![
                    TestValue::Reference(TAG),
                    TestValue::Null,
                    TestValue::Reference(MISSING),
                    TestValue::Reference(OTHER),
                    TestValue::Reference(TAG),
                ]),
            )
            .value("radarProperties", node(contact))
            .value("embeddedScanInfo", scan_info()),
    )
}

fn scan_info() -> TestValue {
    let value = TestInstance::new("ScanCustomValueArray").value(
        "value",
        TestValue::Array(vec![
            node(TestInstance::new("ScanCustomValueFloat").value("value", TestValue::Single(1.0))),
            node(
                TestInstance::new("ScanCustomValueLocString")
                    .value("value", TestValue::text(TEXT_KEY)),
            ),
        ]),
    );
    let datum = TestInstance::new("ScanCustomDataDirect")
        .value("name", TestValue::text(NAME_KEY))
        .value("value", node(value));
    let procedure = TestInstance::new("CustomScanProcedureParams")
        .value("allowedInAIAutoScan", TestValue::Bool(false))
        .value("customDatas", TestValue::Array(vec![node(datum)]));
    node(
        TestInstance::new("ScanCustomDataInfo")
            .value("scanProcedures", TestValue::Array(vec![node(procedure)])),
    )
}

fn pointer(name: &str) -> TestType {
    TestType::StrongPointer(name.to_owned())
}

fn class(name: &str) -> TestType {
    TestType::Class(name.to_owned())
}

const fn node(value: TestInstance) -> TestValue {
    TestValue::Struct(value)
}
