// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! A crafter whose fields are absent, as in the data before the modifiers, or stored with another
//! type than the export expects: absent reads as empty, wrong-typed as empty plus an issue.

use std::error::Error;
use std::time::{Duration, UNIX_EPOCH};

use p4kx_export::{CatalogOptions, CatalogPublicationMode, build_catalog};
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
    let mut options = CatalogOptions::new(UNIX_EPOCH + Duration::from_secs(1_792_000_000));
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    Ok(serde_json::from_slice(
        build_catalog(&archive, &options)?.json(),
    )?)
}

fn crafter(shape: Shape) -> Result<Value, Box<dyn Error>> {
    let catalog = crafter_catalog(shape)?;
    catalog
        .pointer("/definitions/crafters/rows/0")
        .cloned()
        .ok_or_else(|| "the crafter".into())
}

#[test]
fn absent_fields_and_original_attachment_values_are_distinct() -> TestResult {
    let entry = crafter(Shape::Absent)?;
    let component = &entry["component"]["value"];
    let attach = &entry["attachment"]["value"]["attachDef"]["value"];
    assert_eq!(
        component["baseCraftingTime"]["state"], "absent",
        "the field is undeclared"
    );
    assert_eq!(
        component["modifiers"]["state"], "absent",
        "the field is undeclared"
    );
    assert_eq!(
        attach["itemType"]["value"], "Crafter",
        "the original attach type"
    );
    assert_eq!(
        attach["size"]["value"],
        json!({"type":"int32","value":1}),
        "the original attach size"
    );
    assert_eq!(
        attach["itemSubtype"]["state"], "absent",
        "the definition lacks SubType"
    );
    Ok(())
}

#[test]
fn invalid_component_fields_keep_their_original_members() -> TestResult {
    for (shape, owner_path, key, authored) in [
        (
            Shape::ModifiersNoArray,
            "/component/value",
            "modifiers",
            "modifiers",
        ),
        (
            Shape::ProcessTypesNoArray,
            "/component/value/queues/value/0/value",
            "processTypeAvailability",
            "processTypeAvailability",
        ),
        (Shape::QueueNoStruct, "/component/value", "queues", "queues"),
        (Shape::QueuesNoArray, "/component/value", "queues", "queues"),
        (
            Shape::BaseTimeText,
            "/component/value",
            "baseCraftingTime",
            "baseCraftingTime",
        ),
        (
            Shape::AttachTypeNumber,
            "/attachment/value/attachDef/value",
            "itemType",
            "Type",
        ),
        (
            Shape::AttachSizeText,
            "/attachment/value/attachDef/value",
            "size",
            "Size",
        ),
    ] {
        let entry = crafter(shape)?;
        let owner = entry.pointer(owner_path).ok_or("the owning definition")?;
        assert_eq!(
            owner[key]["state"], "invalid",
            "unexpected storage for {key}"
        );
        assert_eq!(
            owner[key]["containsEncryptedSource"], true,
            "the invalid source is encrypted"
        );
        assert!(
            owner
                .pointer("/additionalFields/roots")
                .and_then(Value::as_array)
                .is_some_and(|roots| {
                    roots.iter().any(|root| root["name"]["value"] == authored)
                }),
            "the original {authored} survives in the owner's field arena"
        );
    }
    Ok(())
}

#[test]
fn a_malformed_attachment_definition_retains_its_source_state() -> TestResult {
    let entry = crafter(Shape::AttachDefNoStruct)?;
    let attach = &entry["attachment"]["value"]["attachDef"];
    assert_eq!(
        attach["state"], "invalid",
        "wrong AttachDef storage is retained as invalid"
    );
    assert_eq!(
        attach["containsEncryptedSource"], true,
        "the invalid AttachDef has original evidence"
    );
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
