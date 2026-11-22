// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, Language};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestInstance, TestRecord, TestStruct, TestType,
    TestValue,
};

use super::{TEMPLATE_GUID, TestResult};

const TEMPLATE_TYPE: &str = "ContractTemplate";
const BROKER_TYPE: &str = "MissionBrokerEntry";
const FLOW: &str = "MissionFlow";
const TRIGGER: &str = "FlowTrigger";
const CONDITION: &str = "MissionFlowCondition";
const PROPERTY_IS_SET: &str = "MissionFlowCondition_PropertyIsSet";
const TARGET: &str = "FlowTarget";
const TAG_ROOT: u128 = 0x10;
const TAG_LEAF: u128 = 0x20;
const TAG_DB: u128 = 0x30;

pub(super) fn open(null_template_guid: u128) -> TestResult<(tempfile::TempDir, Archive)> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("mission-flow-sdk.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", database(null_template_guid)).encrypted())
        .entry(TestEntry::stored(
            Language::English.global_ini_path(),
            b"fixture_broker_title=Fixture Broker title\nfixture_template_title=Fixture Template title\n",
        ))
        .write_to(&path)?;
    Ok((
        folder,
        Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?,
    ))
}

fn database(null_template_guid: u128) -> Vec<u8> {
    dataforge_layout()
        .record(
            TestRecord::new(
                1,
                BROKER_TYPE,
                "MissionBrokerEntry.Flow",
                "libs/foundry/records/missionbroker/sdk_flow.xml",
            )
            .value("missionFlow", authored("broker-target"))
            .value("title", TestValue::text("@fixture_broker_title")),
        )
        .record(
            template(TEMPLATE_GUID, "Flow", authored("template-target")).value(
                "contractStartCommsNotification",
                TestValue::Struct(
                    TestInstance::new("FixtureTemplateComms")
                        .value("communicationTags", TestValue::Reference(TAG_LEAF))
                        .value("fixtureTitle", TestValue::text("@fixture_template_title")),
                ),
            ),
        )
        .record(template(null_template_guid, "NullFlow", TestValue::Null))
        .record(tag(TAG_ROOT, "Root", vec![TestValue::Reference(TAG_LEAF)]))
        .record(tag(TAG_LEAF, "Leaf", Vec::new()))
        .record(
            TestRecord::new(
                TAG_DB,
                "TagDatabase",
                "TagDatabase.Root",
                "fixture/roots.xml",
            )
            .value(
                "tags",
                TestValue::Array(vec![TestValue::Reference(TAG_ROOT)]),
            ),
        )
        .build()
}

fn dataforge_layout() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new(TEMPLATE_TYPE)
                .field("missionFlow", strong(FLOW))
                .field(
                    "contractStartCommsNotification",
                    strong("FixtureTemplateComms"),
                ),
        )
        .structure(
            TestStruct::new("FixtureTemplateComms")
                .field("communicationTags", TestType::ReferenceTo("Tag".to_owned()))
                .field("fixtureTitle", TestType::Locale),
        )
        .structure(
            TestStruct::new("Tag")
                .field("tagName", TestType::String)
                .array("children", TestType::ReferenceTo("Tag".to_owned())),
        )
        .structure(
            TestStruct::new("TagDatabase").array("tags", TestType::ReferenceTo("Tag".to_owned())),
        )
        .structure(
            TestStruct::new(BROKER_TYPE)
                .field("missionFlow", strong(FLOW))
                .field("title", TestType::Locale),
        )
        .structure(
            TestStruct::new(FLOW)
                .field("target", strong(TARGET))
                .array("triggers", TestType::Class(TRIGGER.to_owned())),
        )
        .structure(TestStruct::new(TRIGGER).field("condition", strong(CONDITION)))
        .structure(TestStruct::new(CONDITION))
        .structure(
            TestStruct::new(PROPERTY_IS_SET)
                .parent(CONDITION)
                .field("property", TestType::WeakPointer(TARGET.to_owned())),
        )
        .structure(TestStruct::new(TARGET).field("missionVariableName", TestType::String))
}

fn tag(guid: u128, name: &str, children: Vec<TestValue>) -> TestRecord {
    TestRecord::new(guid, "Tag", format!("Tag.{name}"), "fixture/tags.xml")
        .value("tagName", TestValue::text(name))
        .value("children", TestValue::Array(children))
}

fn template(guid: u128, name: &str, flow: TestValue) -> TestRecord {
    TestRecord::new(
        guid,
        TEMPLATE_TYPE,
        format!("ContractTemplate.{name}"),
        format!("libs/foundry/records/contracts/contracttemplates/{name}.xml"),
    )
    .value("missionFlow", flow)
}

fn authored(target: &str) -> TestValue {
    let property = TestInstance::new(TARGET)
        .named(target)
        .value("missionVariableName", TestValue::text(target));
    let condition =
        TestInstance::new(PROPERTY_IS_SET).value("property", TestValue::pointer(target));
    let trigger = TestInstance::new(TRIGGER).value("condition", TestValue::Struct(condition));
    TestValue::Struct(
        TestInstance::new(FLOW)
            .value("target", TestValue::Struct(property))
            .value(
                "triggers",
                TestValue::Array(vec![TestValue::Struct(trigger)]),
            ),
    )
}

fn strong(target: &str) -> TestType {
    TestType::StrongPointer(target.to_owned())
}
