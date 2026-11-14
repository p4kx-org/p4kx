// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogError, CatalogPublicationMode, build_catalog};
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::health_origin_assertions as proof;

#[path = "catalog_item_alignment_links.rs"]
mod links;

const ENUM: &str = "EItemPortAttachRotationLimitAxis";
const OWNER: u128 = 0x1087;
const CHILD: u128 = 0x1088;
const PORTS: &str = "/configurations/itemPorts/rows";
const ITEMS: &str = "/configurations/itemAttachmentConstraints/rows";

#[derive(Clone, Copy)]
enum Layout {
    Complete,
    Absent,
    Unreadable,
}

#[test]
fn physical_port_and_item_declarations_keep_their_independent_values() -> TestResult {
    let database = database(Layout::Complete);
    let validator = jsonschema::validator_for(&p4kx_export::catalog_schema())?;
    for (origin, encrypted) in [(DcbOrigin::Plain, false), (DcbOrigin::Encrypted, true)] {
        let (_folder, archive) = common::open(&database, origin)?;
        let catalog: Value =
            serde_json::from_slice(build_catalog(&archive, &common::options())?.json())?;
        validator
            .validate(&catalog)
            .map_err(|error| format!("catalog v3 alignment schema: {error}"))?;
        check_port_declarations(&catalog, encrypted)?;
        check_item_declarations(&catalog, encrypted)?;
        proof::closure(&catalog)?;
    }
    Ok(())
}

fn check_port_declarations(catalog: &Value, encrypted: bool) -> TestResult {
    let [ports, child_ports] = proof::rows(catalog, PORTS)? else {
        return Err("one physical port container from each selected Entity".into());
    };
    proof::present(
        ports,
        "ownerGuid",
        &json!(common::guid(OWNER)),
        Some(encrypted),
    )?;
    proof::present(ports, "componentOrdinal", &json!(1), Some(encrypted))?;
    let [first, second] = proof::rows(ports, "/ports/value")? else {
        return Err("both physical ports in their authored order".into());
    };
    let first = proof::at(first, "/value")?;
    let second = proof::at(second, "/value")?;
    check_axis(
        first,
        "attachAxesToAlign",
        "FixturePortAxis",
        "declared",
        encrypted,
    )?;
    check_axis(
        second,
        "attachAxesToAlign",
        "FixtureFutureAxis",
        "undeclared",
        encrypted,
    )?;
    proof::present(first, "portOrdinal", &json!(0), Some(encrypted))?;
    proof::present(second, "portOrdinal", &json!(1), Some(encrypted))?;
    proof::present(
        child_ports,
        "ownerGuid",
        &json!(common::guid(CHILD)),
        Some(encrypted),
    )?;

    Ok(())
}

fn check_item_declarations(catalog: &Value, encrypted: bool) -> TestResult {
    let [_, child] = proof::rows(catalog, ITEMS)? else {
        return Err("the owner and child each have a physical attachable component".into());
    };
    let child = proof::at(child, "/definition/value")?;
    check_axis(
        child,
        "attachAxesToAlign",
        "FixtureItemAxis",
        "declared",
        encrypted,
    )?;
    check_axis(
        child,
        "ignoredAttachAxis",
        "FixtureStoredDefault",
        "declared",
        encrypted,
    )?;
    proof::present(
        child,
        "attachCheckCenterOfMass",
        &json!(false),
        Some(encrypted),
    )?;
    Ok(())
}

#[test]
fn undeclared_alignment_members_do_not_become_defaults() -> TestResult {
    let (_built, catalog) = common::build(&database(Layout::Absent))?;
    let item = proof::at(&catalog, &format!("{ITEMS}/0/definition/value"))?;
    let axis = proof::at(item, "/attachAxesToAlign")?;
    proof::cell(axis, "label", "absent")?;
    assert_eq!(proof::at(axis, "/optionStatus")?, &Value::Null);
    proof::cell(item, "attachCheckCenterOfMass", "absent")?;
    let port = proof::at(
        &catalog,
        &format!("{PORTS}/0/ports/value/0/value/attachAxesToAlign"),
    )?;
    proof::cell(port, "label", "absent")?;
    assert_eq!(proof::at(port, "/optionStatus")?, &Value::Null);
    Ok(())
}

#[test]
fn unreadable_original_axis_keeps_its_physical_port_and_failure_state() -> TestResult {
    let (_folder, archive) = common::open(&database(Layout::Unreadable), DcbOrigin::Encrypted)?;
    let mut options = common::options();
    assert!(
        matches!(
            build_catalog(&archive, &options),
            Err(CatalogError::IncompleteExport(_))
        ),
        "the invalid original label prevents a complete export"
    );
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    let catalog: Value = serde_json::from_slice(build_catalog(&archive, &options)?.json())?;
    let future = proof::at(&catalog, &format!("{PORTS}/0/ports/value/1/value"))?;
    proof::present(future, "portOrdinal", &json!(1), Some(true))?;
    let axis = proof::at(future, "/attachAxesToAlign")?;
    let label = proof::cell(axis, "label", "unreadable")?;
    assert_eq!(proof::at(label, "/sourceSetComplete")?, &json!(false));
    assert_eq!(proof::at(axis, "/optionStatus")?, &Value::Null);
    proof::closure(&catalog)
}

fn check_axis(
    owner: &Value,
    field: &str,
    label: &str,
    status: &str,
    encrypted: bool,
) -> TestResult {
    let axis = owner.get(field).ok_or("alignment axis")?;
    proof::present(axis, "label", &json!(label), Some(encrypted))?;
    proof::present(axis, "optionStatus", &json!(status), Some(encrypted))?;
    Ok(())
}

fn database(layout: Layout) -> DataForgeBuilder {
    let alignment = !matches!(layout, Layout::Absent);
    let mut port = TestStruct::new("SItemPortDef")
        .field("Name", TestType::String)
        .field("MinSize", TestType::UInt32)
        .field("MaxSize", TestType::UInt32)
        .field("defaultItem", class("SItemPortDefaultItemDef"));
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
        definition = definition
            .value("attachAxesToAlign", TestValue::text("FixtureItemAxis"))
            .value("ignoredAttachAxis", TestValue::text("FixtureStoredDefault"))
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
