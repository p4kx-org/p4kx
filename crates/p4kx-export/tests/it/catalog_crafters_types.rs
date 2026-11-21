// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! A crafter whose fields are absent, as in the data before the modifiers, or stored with another
//! type than the export expects: absent reads as empty, wrong-typed as empty plus an issue.

use std::error::Error;
use std::time::{Duration, UNIX_EPOCH};

use p4kx_export::{CatalogOptions, build_catalog};
use p4kx_extraction::Archive;
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestInstance, TestRecord, TestStruct, TestType,
    TestValue,
};
use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn Error>>;

const CRAFTER: u128 = 0x7200_0000_0000_4000_8000_0000_0000_0001;

/// How the crafter's structs are declared; every variant but `Absent` stores one field with the
/// wrong type.
#[derive(Clone, Copy)]
enum Shape {
    Absent,
    ModifiersNoArray,
    ProcessTypesNoArray,
    QueueNoStruct,
    QueuesNoArray,
    BaseTimeText,
    AttachDefNoStruct,
    AttachTypeNumber,
    AttachSizeText,
}

fn class(name: &str) -> TestType {
    TestType::Class(name.to_owned())
}

fn queue_struct(shape: Shape) -> TestStruct {
    match shape {
        Shape::ProcessTypesNoArray => {
            TestStruct::new("CrafterQueue").field("processTypeAvailability", TestType::Int32)
        }
        _ => TestStruct::new("CrafterQueue").array("processTypeAvailability", TestType::Bool),
    }
}

fn component_struct(shape: Shape) -> TestStruct {
    let mut component =
        TestStruct::new("CrafterComponentParams").parent("DataForgeComponentParams");
    component = match shape {
        Shape::QueueNoStruct => component.array("queues", TestType::Int32),
        Shape::QueuesNoArray => component.field("queues", TestType::Int32),
        _ => component.array("queues", class("CrafterQueue")),
    };
    component = match shape {
        Shape::Absent => component,
        Shape::ModifiersNoArray => component.field("modifiers", TestType::Int32),
        _ => component.array("modifiers", class("CraftingModifierParams")),
    };
    component = match shape {
        Shape::Absent => component,
        Shape::BaseTimeText => component.field("baseCraftingTime", TestType::String),
        _ => component.field("baseCraftingTime", TestType::Single),
    };
    component
}

fn structures(shape: Shape) -> DataForgeBuilder {
    let attachable = match shape {
        Shape::AttachDefNoStruct => {
            TestStruct::new("SAttachableComponentParams").field("AttachDef", TestType::Int32)
        }
        _ => TestStruct::new("SAttachableComponentParams")
            .field("AttachDef", class("SItemDefinition")),
    }
    .parent("DataForgeComponentParams");
    let definition = TestStruct::new("SItemDefinition")
        .field(
            "Type",
            if matches!(shape, Shape::AttachTypeNumber) {
                TestType::Int32
            } else {
                TestType::String
            },
        )
        .field(
            "Size",
            if matches!(shape, Shape::AttachSizeText) {
                TestType::String
            } else {
                TestType::Int32
            },
        );
    [
        TestStruct::new("EntityClassDefinition").array(
            "Components",
            TestType::StrongPointer("DataForgeComponentParams".to_owned()),
        ),
        TestStruct::new("DataForgeComponentParams"),
        TestStruct::new("CraftingModifierParams"),
        queue_struct(shape),
        component_struct(shape),
        attachable,
        definition,
    ]
    .into_iter()
    .fold(DataForgeBuilder::new(), DataForgeBuilder::structure)
}

fn queue(shape: Shape) -> TestValue {
    let instance = TestInstance::new("CrafterQueue");
    TestValue::Struct(match shape {
        Shape::ProcessTypesNoArray => {
            instance.value("processTypeAvailability", TestValue::Int32(3))
        }
        _ => instance,
    })
}

fn crafter_component(shape: Shape) -> TestValue {
    let mut component = TestInstance::new("CrafterComponentParams");
    component = match shape {
        Shape::QueueNoStruct => {
            component.value("queues", TestValue::Array(vec![TestValue::Int32(1)]))
        }
        Shape::QueuesNoArray => component.value("queues", TestValue::Int32(1)),
        _ => component.value("queues", TestValue::Array(vec![queue(shape)])),
    };
    component = match shape {
        Shape::ModifiersNoArray => component.value("modifiers", TestValue::Int32(2)),
        Shape::Absent => component,
        _ => component.value("modifiers", TestValue::Array(Vec::new())),
    };
    TestValue::Struct(match shape {
        Shape::BaseTimeText => component.value("baseCraftingTime", TestValue::text("slow")),
        _ => component,
    })
}

fn attachable(shape: Shape) -> TestValue {
    let definition = match shape {
        Shape::AttachTypeNumber => TestInstance::new("SItemDefinition")
            .value("Type", TestValue::Int32(5))
            .value("Size", TestValue::Int32(1)),
        Shape::AttachSizeText => TestInstance::new("SItemDefinition")
            .value("Type", TestValue::text("Crafter"))
            .value("Size", TestValue::text("large")),
        _ => TestInstance::new("SItemDefinition")
            .value("Type", TestValue::text("Crafter"))
            .value("Size", TestValue::Int32(1)),
    };
    let attach = match shape {
        Shape::AttachDefNoStruct => TestValue::Int32(7),
        _ => TestValue::Struct(definition),
    };
    TestValue::Struct(TestInstance::new("SAttachableComponentParams").value("AttachDef", attach))
}

/// Builds the catalog of an Archive with one crafter of the given shape.
fn crafter_catalog(shape: Shape) -> Result<Value, Box<dyn Error>> {
    let record = TestRecord::new(
        CRAFTER,
        "EntityClassDefinition",
        "EntityClassDefinition.typed",
        "libs/foundry/records/entities/basebuilding/interactables/typed.xml",
    )
    .value(
        "Components",
        TestValue::Array(vec![crafter_component(shape), attachable(shape)]),
    );
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(
            TestEntry::zstd("Data\\Game2.dcb", structures(shape).record(record).build())
                .encrypted(),
        )
        .write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?;
    let options = CatalogOptions::new(UNIX_EPOCH + Duration::from_secs(1_792_000_000));
    Ok(serde_json::from_slice(
        build_catalog(&archive, &options)?.json(),
    )?)
}

fn crafter(shape: Shape) -> Result<Value, Box<dyn Error>> {
    let catalog = crafter_catalog(shape)?;
    catalog
        .get("crafters")
        .and_then(Value::as_array)
        .and_then(|crafters| crafters.first())
        .cloned()
        .ok_or_else(|| "the crafter".into())
}

#[test]
fn absent_fields_read_as_empty_without_an_issue() -> TestResult {
    let entry = crafter(Shape::Absent)?;
    assert_eq!(entry["baseCraftingTime"], Value::Null);
    assert_eq!(entry["modifiers"], json!([]));
    assert_eq!(entry["issues"], json!([]));
    assert_eq!(entry["type"], "Crafter");
    assert_eq!(entry["size"], 1);
    assert_eq!(entry["subType"], Value::Null, "the struct lacks SubType");
    Ok(())
}

#[test]
fn a_modifiers_value_that_is_no_array_is_reported() -> TestResult {
    let entry = crafter(Shape::ModifiersNoArray)?;
    assert_eq!(entry["modifiers"], json!([]));
    assert_eq!(entry["issues"], json!(["unreadable_modifiers"]));
    Ok(())
}

#[test]
fn a_process_type_value_that_is_no_array_is_reported() -> TestResult {
    let entry = crafter(Shape::ProcessTypesNoArray)?;
    assert_eq!(entry["queues"][0]["processTypeAvailability"], json!([]));
    assert_eq!(entry["issues"], json!(["unreadable_process_types"]));
    Ok(())
}

#[test]
fn a_queue_that_is_no_struct_is_kept_empty_and_reported() -> TestResult {
    let entry = crafter(Shape::QueueNoStruct)?;
    assert_eq!(
        entry["queues"],
        json!([{
            "name": null,
            "maxJobsInProgress": null,
            "maxJobsWaiting": null,
            "blueprintCategoryAvailability":
                {"kind": "none", "structName": null, "blueprintCategoryGuids": []},
            "processTypeAvailability": [],
        }])
    );
    assert_eq!(entry["issues"], json!(["unreadable_queue"]));
    Ok(())
}

#[test]
fn a_queues_value_that_is_no_array_is_reported() -> TestResult {
    let entry = crafter(Shape::QueuesNoArray)?;
    assert_eq!(entry["queues"], json!([]));
    assert_eq!(entry["issues"], json!(["unreadable_queues"]));
    Ok(())
}

#[test]
fn a_base_crafting_time_that_is_no_number_is_reported() -> TestResult {
    let entry = crafter(Shape::BaseTimeText)?;
    assert_eq!(entry["baseCraftingTime"], Value::Null);
    assert_eq!(entry["issues"], json!(["unreadable_base_crafting_time"]));
    Ok(())
}

#[test]
fn an_attach_def_with_a_wrong_type_is_reported() -> TestResult {
    let entry = crafter(Shape::AttachDefNoStruct)?;
    assert_eq!(
        (&entry["type"], &entry["size"], &entry["issues"]),
        (
            &Value::Null,
            &Value::Null,
            &json!(["unreadable_attach_def"])
        )
    );
    let entry = crafter(Shape::AttachTypeNumber)?;
    assert_eq!(entry["type"], Value::Null);
    assert_eq!(entry["size"], 1);
    assert_eq!(entry["issues"], json!(["unreadable_attach_def"]));
    let entry = crafter(Shape::AttachSizeText)?;
    assert_eq!(entry["type"], "Crafter");
    assert_eq!(entry["size"], Value::Null);
    assert_eq!(entry["issues"], json!(["unreadable_attach_def"]));
    Ok(())
}

#[test]
fn readable_alternate_attach_restrictions_retain_their_original_storage() -> TestResult {
    for (shape, key, original) in [
        (
            Shape::AttachTypeNumber,
            "itemType",
            json!({"type":"number", "value":{"type":"int32", "value":5}}),
        ),
        (
            Shape::AttachSizeText,
            "size",
            json!({"type":"string", "value":{"type":"gameText", "sourceText":"large"}}),
        ),
    ] {
        let catalog = crafter_catalog(shape)?;
        let cell = catalog
            .pointer(&format!(
                "/configurations/itemAttachmentConstraints/rows/0/definition/value/{key}"
            ))
            .ok_or("authored alternate restriction")?;
        assert_eq!(
            cell["state"], "present",
            "the original storage was readable"
        );
        assert_eq!(
            cell["containsEncryptedSource"], true,
            "the original DCB was encrypted"
        );
        assert_eq!(
            cell.pointer("/value/values/0/value/value"),
            Some(&original),
            "the original restriction keeps its storage and value"
        );
    }
    Ok(())
}
