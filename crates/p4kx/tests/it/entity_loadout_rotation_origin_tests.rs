// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForge, RecordTree, Value as DataForgeValue};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntity, TestEntry, TestInstance, TestStruct, TestType,
    TestValue, entity_schema,
};
use serde_json::Value;

use super::{TestResult, dataforge_ordinal, origin, verify_emitted_fields};

#[test]
fn json_rotation_constraint_keeps_authored_value_state_and_origin() -> TestResult {
    let directory = tempfile::tempdir()?;
    let archive = directory.path().join("Data.p4k");
    let database = rotation_constraint_fixture();
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("Data\\Game2.dcb", database.clone()).encrypted())
        .write_to(&archive)?;
    let result = p4kx_testkit::p4kx()?
        .args(["--json", "entity", "loadout"])
        .arg(&archive)
        .arg("RotationFixture")
        .output()?;
    assert!(
        result.status.success(),
        "synthetic rotation constraints load: {result:?}"
    );
    let json: Value = serde_json::from_slice(&result.stdout)?;
    let forge = DataForge::open(&database)?;
    let tree = forge.record(0).ok_or("rotation fixture Record")?.tree()?;
    verify_rotation_constraints(&json, &tree)?;
    verify_emitted_fields(&json)
}

fn verify_rotation_constraints(json: &Value, tree: &RecordTree<'_>) -> TestResult {
    let ports = json
        .pointer("/entity/ports")
        .and_then(Value::as_array)
        .ok_or("fixture ports")?;
    for (port_name, state, value) in [
        ("enabled", "present", Some(true)),
        ("disabled", "present", Some(false)),
        ("null", "null", None),
        ("invalid", "invalid", None),
        ("unreadable", "unreadable", None),
        ("absent", "absent", None),
    ] {
        let index = ports
            .iter()
            .position(|port| port.get("name").and_then(Value::as_str) == Some(port_name))
            .ok_or("an original rotation fixture port")?;
        let field = format!("/entity/ports/{index}/definition/rotation_constraint");
        verify_rotation_constraint(json, tree, &field, port_name, state, value)?;
    }
    Ok(())
}

fn verify_rotation_constraint(
    json: &Value,
    tree: &RecordTree<'_>,
    field: &str,
    port_name: &str,
    state: &str,
    value: Option<bool>,
) -> TestResult {
    if state == "absent" {
        assert!(
            json.pointer(field).is_none(),
            "{port_name}: no constraint declaration"
        );
        return Ok(());
    }
    let constraint = json.pointer(field).ok_or("emitted rotation constraint")?;
    assert_eq!(
        constraint
            .get("animation_semantics")
            .and_then(Value::as_str),
        Some("unresolved"),
        "{port_name}"
    );
    let flag = constraint
        .get("no_rotation")
        .ok_or("emitted rotation flag")?;
    assert_eq!(
        flag.get("state").and_then(Value::as_str),
        Some(state),
        "{port_name}"
    );
    assert_eq!(
        flag.get("value"),
        Some(&value.map_or(Value::Null, Value::Bool)),
        "{port_name}"
    );
    let pointer = format!("{field}/no_rotation");
    let receipt = origin(json, &pointer)?;
    assert_eq!(
        receipt
            .get("containsEncryptedSource")
            .and_then(Value::as_bool),
        Some(true),
        "{port_name}"
    );
    assert_eq!(
        receipt.get("sourceSetComplete").and_then(Value::as_bool),
        Some(state != "unreadable"),
        "{port_name}"
    );
    if let Some(expected) = value {
        let ordinal = dataforge_ordinal(json, &pointer, "payload")?;
        let mut original = None;
        tree.root().visit_owned_values(|visit| {
            if visit.ordinal() == ordinal
                && let DataForgeValue::Bool(value) = visit.value()
            {
                original = Some(*value);
            }
        });
        assert_eq!(original, Some(expected), "{port_name}: original Boolean");
    }
    Ok(())
}

fn rotation_constraint_fixture() -> Vec<u8> {
    const BASE: &str = "SItemPortDefAttachmentImplementationBase";
    const ENTITY: &str = "SItemPortDefAttachmentImplementationEntity";
    const NULL: &str = "SItemPortDefAttachmentImplementationNullFixture";
    const TEXT: &str = "SItemPortDefAttachmentImplementationTextFixture";
    const FLAGS: &str = "SItemPortConstraintParams";
    const TEXT_FLAGS: &str = "FixtureTextConstraint";
    let authored = |kind, constraint_type, value| {
        TestInstance::new(kind).value(
            "constraintParams",
            TestValue::Struct(TestInstance::new(constraint_type).value("noRotation", value)),
        )
    };
    let ports = [
        (
            "enabled",
            Some(authored(ENTITY, FLAGS, TestValue::Bool(true))),
        ),
        (
            "disabled",
            Some(authored(ENTITY, FLAGS, TestValue::Bool(false))),
        ),
        ("null", Some(TestInstance::new(NULL))),
        (
            "invalid",
            Some(authored(TEXT, TEXT_FLAGS, TestValue::text("incompatible"))),
        ),
        (
            "unreadable",
            Some(authored(ENTITY, FLAGS, TestValue::UInt8(2))),
        ),
        ("absent", None),
    ]
    .into_iter()
    .map(|(name, attachment)| {
        let mut port = TestInstance::new("SItemPortDef").value("Name", TestValue::text(name));
        if let Some(attachment) = attachment {
            port = port.value("AttachmentImplementation", TestValue::Struct(attachment));
        }
        TestValue::Struct(port)
    })
    .collect();
    let component = TestInstance::new("SItemPortContainerComponentParams")
        .value("Ports", TestValue::Array(ports));
    let record = TestEntity::new(1, "RotationFixture", "entities/rotation_fixture.xml")
        .component(TestValue::Struct(component))
        .record();
    entity_schema(DataForgeBuilder::new())
        .structure(TestStruct::new(NULL).parent(BASE).field(
            "constraintParams",
            TestType::StrongPointer(FLAGS.to_owned()),
        ))
        .structure(
            TestStruct::new(TEXT)
                .parent(BASE)
                .field("constraintParams", TestType::Class(TEXT_FLAGS.to_owned())),
        )
        .structure(TestStruct::new(TEXT_FLAGS).field("noRotation", TestType::String))
        .record(record)
        .build()
}
