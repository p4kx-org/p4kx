// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Language;
use p4kx_testkit::{
    DataForgeBuilder, GlobalIniBuilder, TestEntry, TestInstance, TestRecord, TestStruct, TestType,
    TestValue,
};

const ENTITY: &str = "EntityClassDefinition";
const COMPONENT: &str = "DataForgeComponentParams";
const PORTS: &str = "SItemPortContainerComponentParams";
const PORT: &str = "SItemPortDef";
const PORT_TYPE: &str = "SItemPortDefTypes";
const ATTACHABLE: &str = "SAttachableComponentParams";
const ATTACHMENT: &str = "SItemDefinition";
const SHARED_GUID: u128 = 0x1177_9170;
pub(super) const DISPLAY_KEY: &str = "fixture_mount_label";

pub(super) fn database(attachments: bool) -> Vec<u8> {
    let schema = port_schema();
    if !attachments {
        return schema
            .record(entity(SHARED_GUID, "spaceships", "empty", Vec::new()))
            .build();
    }
    schema
        .structure(
            TestStruct::new(ATTACHABLE)
                .parent(COMPONENT)
                .field("AttachDef", TestType::Class(ATTACHMENT.into())),
        )
        .structure(
            TestStruct::new(ATTACHMENT)
                .field("Type", TestType::String)
                .field("SubType", TestType::String)
                .field("Size", TestType::UInt32)
                .field("Tags", TestType::String)
                .field("RequiredTags", TestType::String),
        )
        .record(entity(SHARED_GUID, "spaceships", "ship", vec![ports()]))
        .record(entity(
            SHARED_GUID,
            "scitem/fixture",
            "unmounted",
            vec![attachment(), ports()],
        ))
        .build()
}

fn port_schema() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new(ENTITY).array("Components", TestType::StrongPointer(COMPONENT.into())),
        )
        .structure(TestStruct::new(COMPONENT))
        .structure(
            TestStruct::new(PORTS)
                .parent(COMPONENT)
                .array("Ports", TestType::Class(PORT.into())),
        )
        .structure(
            TestStruct::new(PORT)
                .field("Name", TestType::String)
                .field("DisplayName", TestType::Locale)
                .field("MinSize", TestType::UInt32)
                .field("Flags", TestType::String)
                .field("RequiredPortTags", TestType::String)
                .array("Types", TestType::Class(PORT_TYPE.into())),
        )
        .structure(
            TestStruct::new(PORT_TYPE)
                .field("Type", TestType::String)
                .array("SubTypes", TestType::String),
        )
}

fn entity(guid: u128, folder: &str, name: &str, components: Vec<TestValue>) -> TestRecord {
    TestRecord::new(
        guid,
        ENTITY,
        format!("{ENTITY}.{name}"),
        format!("libs/foundry/records/entities/{folder}/{name}.xml"),
    )
    .value("Components", TestValue::Array(components))
}

fn ports() -> TestValue {
    let types = || {
        TestValue::Struct(
            TestInstance::new(PORT_TYPE)
                .value("Type", TestValue::text("MiningModifier"))
                .value(
                    "SubTypes",
                    TestValue::Array(vec![TestValue::text("Active"), TestValue::text("Active")]),
                ),
        )
    };
    let port = || {
        TestValue::Struct(
            TestInstance::new(PORT)
                .value("Name", TestValue::text("same_mount"))
                .value("DisplayName", TestValue::text(format!("@{DISPLAY_KEY}")))
                .value("MinSize", TestValue::UInt32(1))
                .value("Flags", TestValue::text("Uneditable"))
                .value("RequiredPortTags", TestValue::text("compatible"))
                .value("Types", TestValue::Array(vec![types()])),
        )
    };
    TestValue::Struct(
        TestInstance::new(PORTS).value("Ports", TestValue::Array(vec![port(), port()])),
    )
}

fn attachment() -> TestValue {
    let definition = TestInstance::new(ATTACHMENT)
        .value("Type", TestValue::text("MiningModifier"))
        .value("SubType", TestValue::text("Active"))
        .value("Size", TestValue::UInt32(2))
        .value("Tags", TestValue::text("compatible"))
        .value("RequiredTags", TestValue::text("required"));
    TestValue::Struct(
        TestInstance::new(ATTACHABLE).value("AttachDef", TestValue::Struct(definition)),
    )
}

pub(super) fn localization(language: Language, label: &str) -> TestEntry {
    TestEntry::stored(
        language.global_ini_path(),
        GlobalIniBuilder::new()
            .text(DISPLAY_KEY, label)
            .text("unused_mount", "Unused")
            .build(),
    )
}
