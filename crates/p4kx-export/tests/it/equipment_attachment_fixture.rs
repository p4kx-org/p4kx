// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

pub(super) const OWNER: u128 = 0x1166_a001;
const TARGET: u128 = 0x1166_a002;
const ENUM: &str = "EItemPortAttachRotationLimitAxis";

pub(super) fn original_restrictions() -> DataForgeBuilder {
    schema()
        .record(owner("FixtureRepeatedAttachment", "repeated_attachment", 2))
        .record(owner("FixtureAttachmentAlias", "attachment_alias", 1))
        .record(TestRecord::new(
            TARGET,
            "EntityClassDefinition",
            "EntityClassDefinition.FixtureTarget",
            "libs/foundry/records/entities/scitem/fixture/attachment_target.xml",
        ))
}

fn schema() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .enumeration(ENUM, ["FixtureAxis"])
        .structure(
            TestStruct::new("EntityClassDefinition")
                .array("Components", pointer("DataForgeComponentParams")),
        )
        .structure(TestStruct::new("DataForgeComponentParams"))
        .structure(
            TestStruct::new("SAttachableComponentParams")
                .parent("DataForgeComponentParams")
                .field("AttachDef", class("SItemDefinition")),
        )
        .structure(
            TestStruct::new("SItemDefinition")
                .field("Type", class("FixtureRestriction"))
                .field("SubType", TestType::String)
                .field("Size", TestType::Double)
                .field("Tags", TestType::String)
                .field("RequiredTags", TestType::String)
                .field("attachAxesToAlign", TestType::Enum(ENUM.to_owned()))
                .field("ignoredAttachAxis", TestType::Enum(ENUM.to_owned()))
                .field("attachCheckCenterOfMass", TestType::Bool),
        )
        .structure(
            TestStruct::new("FixtureRestriction")
                .field("exactInteger", TestType::Int64)
                .field("rawSingle", TestType::Single)
                .field("rawDouble", TestType::Double)
                .field(
                    "originalTarget",
                    TestType::ReferenceTo("EntityClassDefinition".to_owned()),
                ),
        )
}

fn owner(name: &str, path: &str, count: usize) -> TestRecord {
    let components = (0..count).map(|_| attachment()).collect();
    TestRecord::new(
        OWNER,
        "EntityClassDefinition",
        format!("EntityClassDefinition.{name}"),
        format!("libs/foundry/records/entities/scitem/fixture/{path}.xml"),
    )
    .value("Components", TestValue::Array(components))
}

fn attachment() -> TestValue {
    node(
        TestInstance::new("SAttachableComponentParams").value(
            "AttachDef",
            node(
                TestInstance::new("SItemDefinition")
                    .value(
                        "Type",
                        node(
                            TestInstance::new("FixtureRestriction")
                                .value("exactInteger", TestValue::Int64(9_007_199_254_740_993))
                                .value("rawSingle", TestValue::Single(f32::from_bits(0x7fc0_1166)))
                                .value("rawDouble", TestValue::Double(-0.0))
                                .value("originalTarget", TestValue::Reference(TARGET)),
                        ),
                    )
                    .value("SubType", TestValue::text("Power"))
                    .value(
                        "Size",
                        TestValue::Double(f64::from_bits(0x7ff8_0000_0000_1166)),
                    )
                    .value("Tags", TestValue::text("  repeated  tags "))
                    .value("RequiredTags", TestValue::text("  attachment_required "))
                    .value("attachAxesToAlign", TestValue::text("FixtureAxis"))
                    .value("ignoredAttachAxis", TestValue::text("FixtureFutureAxis"))
                    .value("attachCheckCenterOfMass", TestValue::Bool(false)),
            ),
        ),
    )
}

fn class(name: &str) -> TestType {
    TestType::Class(name.to_owned())
}
fn pointer(name: &str) -> TestType {
    TestType::StrongPointer(name.to_owned())
}
const fn node(value: TestInstance) -> TestValue {
    TestValue::Struct(value)
}
