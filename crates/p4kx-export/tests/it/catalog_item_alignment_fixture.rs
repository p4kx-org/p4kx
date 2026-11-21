// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

const ENUM: &str = "EItemPortAttachRotationLimitAxis";
pub(crate) const OWNER: u128 = 0x1087;
pub(super) const CHILD: u128 = 0x1088;
pub(super) const PORT_KEY: &str = "@fixture_port_display";

#[derive(Clone, Copy)]
pub(super) enum Layout {
    Complete,
    Localized,
    Absent,
    Unreadable,
    UnrelatedInvalid,
}

pub(super) fn database(layout: Layout) -> DataForgeBuilder {
    let alignment = !matches!(layout, Layout::Absent);
    let mut port = TestStruct::new("SItemPortDef")
        .field("Name", TestType::String)
        .field("MinSize", TestType::UInt32)
        .field("MaxSize", TestType::UInt32)
        .field("defaultItem", class("SItemPortDefaultItemDef"));
    if matches!(layout, Layout::Localized) {
        port = port.field("DisplayName", TestType::Locale);
    }
    let mut definition = TestStruct::new("SItemDefinition")
        .field("Type", TestType::String)
        .field("SubType", TestType::String)
        .field("Size", TestType::UInt32);
    if alignment {
        port = port.field("attachAxesToAlign", TestType::Enum(ENUM.to_owned()));
        definition = definition
            .field("attachAxesToAlign", TestType::Enum(ENUM.to_owned()))
            .field("ignoredAttachAxis", TestType::Enum(ENUM.to_owned()))
            .field("attachCheckCenterOfMass", TestType::Bool);
    }
    DataForgeBuilder::new()
        .version(8)
        .enumeration(
            ENUM,
            ["FixturePortAxis", "FixtureItemAxis", "FixtureStoredDefault"],
        )
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
        .structure(definition)
        .structure(
            TestStruct::new("SItemPortContainerComponentParams")
                .parent("DataForgeComponentParams")
                .array("Ports", class("SItemPortDef")),
        )
        .structure(port)
        .structure(TestStruct::new("SItemPortDefaultItemDef").field(
            "entityClass",
            TestType::ReferenceTo("EntityClassDefinition".to_owned()),
        ))
        .record(entity(OWNER, "FixtureAlignOwner", "align_owner", layout))
        .record(entity(CHILD, "FixtureAlignChild", "align_child", layout))
}

fn entity(guid: u128, name: &str, filename: &str, layout: Layout) -> TestRecord {
    let alignment = !matches!(layout, Layout::Absent);
    let mut definition = TestInstance::new("SItemDefinition")
        .value("Type", TestValue::text("PowerPlant"))
        .value("SubType", TestValue::text("Power"))
        .value("Size", TestValue::UInt32(1));
    if alignment {
        let ignored = if matches!(layout, Layout::UnrelatedInvalid) {
            TestValue::Raw((u32::MAX - 1).to_le_bytes().to_vec())
        } else {
            TestValue::text("FixtureStoredDefault")
        };
        definition = definition
            .value("attachAxesToAlign", TestValue::text("FixtureItemAxis"))
            .value("ignoredAttachAxis", ignored)
            .value("attachCheckCenterOfMass", TestValue::Bool(false));
    }
    let ports = if guid == OWNER {
        vec![
            port("mount", Some(CHILD), "FixturePortAxis", layout),
            port("future", None, "FixtureFutureAxis", layout),
        ]
    } else {
        vec![port("nested", None, "FixtureStoredDefault", layout)]
    };
    TestRecord::new(
        guid,
        "EntityClassDefinition",
        format!("EntityClassDefinition.{name}"),
        format!("libs/foundry/records/entities/scitem/fixture/{filename}.xml"),
    )
    .value(
        "Components",
        TestValue::Array(vec![
            node(
                TestInstance::new("SAttachableComponentParams")
                    .value("AttachDef", node(definition)),
            ),
            node(
                TestInstance::new("SItemPortContainerComponentParams")
                    .value("Ports", TestValue::Array(ports)),
            ),
        ]),
    )
}

fn port(name: &str, target: Option<u128>, label: &str, layout: Layout) -> TestValue {
    let mut port = TestInstance::new("SItemPortDef")
        .value("Name", TestValue::text(name))
        .value("MinSize", TestValue::UInt32(1))
        .value("MaxSize", TestValue::UInt32(2));
    if matches!(layout, Layout::Localized) {
        let display_name = if name == "mount" {
            PORT_KEY
        } else {
            "@LOC_EMPTY"
        };
        port = port.value("DisplayName", TestValue::text(display_name));
    }
    if let Some(guid) = target {
        port = port.value(
            "defaultItem",
            node(
                TestInstance::new("SItemPortDefaultItemDef")
                    .value("entityClass", TestValue::Reference(guid)),
            ),
        );
    }
    if !matches!(layout, Layout::Absent) {
        let value = if matches!(layout, Layout::Unreadable) && name == "future" {
            TestValue::Raw((u32::MAX - 1).to_le_bytes().to_vec())
        } else {
            TestValue::text(label)
        };
        port = port.value("attachAxesToAlign", value);
    }
    node(port)
}

fn pointer(name: &str) -> TestType {
    TestType::StrongPointer(name.to_owned())
}

fn class(name: &str) -> TestType {
    TestType::Class(name.to_owned())
}

const fn node(instance: TestInstance) -> TestValue {
    TestValue::Struct(instance)
}
