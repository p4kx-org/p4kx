// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForge, DataForgeEntry, DataType, RecordTree, Value as Raw};
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

use crate::commodity_v3_fixture::TestResult;

const OWNER: u128 = 0x9660;
const ENTITY: &str = "EntityClassDefinition";
const COMPONENT: &str = "DataForgeComponentParams";
const ATTACHMENT: &str = "SAttachableComponentParams";
const DEFINITION: &str = "SItemDefinition";
const METADATA: &str = "SEntityComponentObjectMetadataParams";
const FOREIGN: &str = "FixtureUnrelatedComponent";
const BOUNDS: &str = "inventoryOccupancyLocalBoundsMin";
const COMPONENT_INDEX: u32 = 1;
const ATTACHMENT_INDEX: u32 = 2;
const DEFINITION_INDEX: u32 = 3;
const BAD_INSTANCE: u32 = u32::MAX - 1;

#[derive(Clone, Copy, Debug)]
pub(super) enum Fault {
    Component,
    AttachDef,
    Null,
    Foreign,
}

pub(super) fn database(fault: Fault) -> DataForgeBuilder {
    let structs = [
        TestStruct::new(ENTITY).array("Components", pointer(COMPONENT)),
        TestStruct::new(COMPONENT),
        TestStruct::new(ATTACHMENT)
            .parent(COMPONENT)
            .field("AttachDef", pointer(DEFINITION)),
        TestStruct::new(DEFINITION)
            .field("Type", TestType::String)
            .field("Size", TestType::Int32)
            .field(BOUNDS, TestType::Class("Vec3".to_owned())),
        TestStruct::new(METADATA)
            .parent(COMPONENT)
            .field("hasLocalBounds", TestType::Bool),
        TestStruct::new(FOREIGN)
            .parent(COMPONENT)
            .field("marker", TestType::String),
        TestStruct::new("Vec3")
            .field("x", TestType::Single)
            .field("y", TestType::Single)
            .field("z", TestType::Single),
    ];
    let components = TestValue::Array(vec![
        attachment(definition()),
        node(TestInstance::new(METADATA).value("hasLocalBounds", TestValue::Bool(true))),
        fault_value(fault),
    ]);
    structs
        .into_iter()
        .fold(
            DataForgeBuilder::new().version(8),
            DataForgeBuilder::structure,
        )
        .record(
            TestRecord::new(
                OWNER,
                ENTITY,
                "EntityClassDefinition.ScopeOwner",
                "libs/foundry/records/entities/scitem/fixture/scope_owner.xml",
            )
            .value("Components", components),
        )
}

fn definition() -> TestValue {
    node(
        TestInstance::new(DEFINITION)
            .value("Type", TestValue::text("PowerPlant"))
            .value("Size", TestValue::Int32(1))
            .value(
                BOUNDS,
                node(
                    TestInstance::new("Vec3")
                        .value("x", TestValue::Single(1.0))
                        .value("y", TestValue::Single(2.0))
                        .value("z", TestValue::Single(3.0)),
                ),
            ),
    )
}

fn fault_value(fault: Fault) -> TestValue {
    match fault {
        Fault::Component => bad_pointer(ATTACHMENT_INDEX),
        Fault::AttachDef => attachment(bad_pointer(DEFINITION_INDEX)),
        Fault::Null => TestValue::Null,
        Fault::Foreign => node(
            TestInstance::new(FOREIGN).value("marker", TestValue::text("readable foreign source")),
        ),
    }
}

fn attachment(value: TestValue) -> TestValue {
    node(TestInstance::new(ATTACHMENT).value("AttachDef", value))
}

fn bad_pointer(structure: u32) -> TestValue {
    let mut bytes = structure.to_le_bytes().to_vec();
    bytes.extend_from_slice(&BAD_INSTANCE.to_le_bytes());
    TestValue::Raw(bytes)
}

pub(super) fn prove_original(entry: &DataForgeEntry, fault: Fault) -> TestResult {
    let forge = entry.open()?;
    prove_schema(&forge)?;
    let record = forge.record(0).ok_or("the original owner")?;
    let tree = record.tree()?;
    prove_collection(&tree, fault)
}

fn prove_schema(forge: &DataForge<'_>) -> TestResult {
    for (index, name) in [
        (COMPONENT_INDEX, COMPONENT),
        (ATTACHMENT_INDEX, ATTACHMENT),
        (DEFINITION_INDEX, DEFINITION),
    ] {
        assert_eq!(
            forge
                .structs()
                .nth(usize::try_from(index)?)
                .ok_or("original struct slot")?
                .name()?,
            name,
            "the raw pointer uses a valid declared struct index"
        );
    }
    Ok(())
}

fn prove_collection(tree: &RecordTree<'_>, fault: Fault) -> TestResult {
    let field = tree
        .root()
        .fields()
        .iter()
        .find(|field| field.name() == "Components")
        .ok_or("original Components declaration")?;
    assert_eq!(
        field.storage().data_type(),
        DataType::StrongPointer,
        "the original collection holds component pointers"
    );
    assert_eq!(
        u32::from(field.storage().target()),
        COMPONENT_INDEX,
        "the nominal family can contain the requested components"
    );
    let Some(Raw::Array(components)) = tree.field("Components") else {
        return Err("readable original Components array".into());
    };
    assert_eq!(
        components.len(),
        3,
        "two readable prefix components and the exact final source slot"
    );
    let [prefix, _, final_slot] = components.as_ref() else {
        return Err("exact three original Components slots".into());
    };
    let Raw::Struct(prefix) = prefix else {
        return Err("readable original attachment prefix".into());
    };
    let Some(Raw::Struct(definition)) = prefix.field("AttachDef") else {
        return Err("readable original occupancy definition".into());
    };
    assert!(
        definition.field(BOUNDS).is_some(),
        "the declared SItemDefinition family contains occupancy bounds"
    );
    prove_fault(final_slot, fault)
}

fn prove_fault(value: &Raw<'_>, fault: Fault) -> TestResult {
    match fault {
        Fault::Component => assert!(
            matches!(value, Raw::Error(_)),
            "the actual original component slot is a Value::Error"
        ),
        Fault::AttachDef => {
            let Raw::Struct(component) = value else {
                return Err("readable original attachment component".into());
            };
            let field = component
                .fields()
                .iter()
                .find(|field| field.name() == "AttachDef")
                .ok_or("original AttachDef declaration")?;
            assert_eq!(
                field.storage().data_type(),
                DataType::StrongPointer,
                "the failed field is a declared pointer"
            );
            assert_eq!(
                u32::from(field.storage().target()),
                DEFINITION_INDEX,
                "the failed target's actual nominal family is SItemDefinition"
            );
            assert!(
                matches!(component.field("AttachDef"), Some(Raw::Error(_))),
                "the actual original AttachDef target is unreadable"
            );
        }
        Fault::Null => assert!(
            matches!(value, Raw::Null),
            "the exclusion control is an actual explicit null"
        ),
        Fault::Foreign => {
            let Raw::Struct(component) = value else {
                return Err("readable foreign component control".into());
            };
            assert_eq!(
                component.type_name(),
                FOREIGN,
                "the control has a known foreign concrete type"
            );
            assert!(
                matches!(
                    component.field("marker"),
                    Some(Raw::String("readable foreign source"))
                ),
                "the foreign control payload is readable"
            );
        }
    }
    Ok(())
}

const fn node(instance: TestInstance) -> TestValue {
    TestValue::Struct(instance)
}
fn pointer(name: &str) -> TestType {
    TestType::StrongPointer(name.to_owned())
}
