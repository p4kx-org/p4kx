// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogError, CatalogPublicationMode};
use p4kx_extraction::{DataForge, DataType, RecordTree, StructValue, Value, ValueError};
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestValue};

use crate::health_origin_fixture::TestResult;
use crate::mission_canonical_health;
use crate::mission_values_fixture::{self as fixture, AutoLayout, Owner};

#[path = "mission_values_auto_declarations.rs"]
mod declaration_evidence;

const VALUE: &str = "MissionPropertyValue_AutoSpawns";
const OPTIONS: &str = "SpawnClosetSearchOptions_Tags";
const ENTITY: &str = "SpawnClosetSearchOptions_EntityDeclaration";
const LOOKALIKE: &str = "LookalikeTagList";

enum Shape {
    Struct(&'static str),
    Record(&'static str),
    Dangling,
    EmptyArray,
}

struct Case {
    layout: AutoLayout,
    path: &'static [&'static str],
    field: &'static str,
    data_type: DataType,
    target: &'static str,
    shape: Shape,
}

#[test]
fn auto_spawns_rejects_a_reference_declared_for_another_record_family() -> TestResult {
    verify(&Case {
        layout: AutoLayout::WrongAutoTarget,
        path: &[],
        field: "autoSpawns",
        data_type: DataType::Reference,
        target: "Definition",
        shape: Shape::Record("AutoSpawns"),
    })
}

#[test]
fn auto_spawns_rejects_a_search_pointer_declared_for_another_struct() -> TestResult {
    verify(&Case {
        layout: AutoLayout::WrongSearchTarget,
        path: &[],
        field: "searchOptions",
        data_type: DataType::StrongPointer,
        target: "Definition",
        shape: Shape::Struct(OPTIONS),
    })
}

#[test]
fn auto_spawns_rejects_inline_storage_for_the_search_pointer() -> TestResult {
    verify(&Case {
        layout: AutoLayout::InlineSearch,
        path: &[],
        field: "searchOptions",
        data_type: DataType::Class,
        target: OPTIONS,
        shape: Shape::Struct(OPTIONS),
    })
}

#[test]
fn auto_spawns_rejects_a_lookalike_closet_tag_list() -> TestResult {
    verify(&Case {
        layout: AutoLayout::WrongClosetType,
        path: &["searchOptions"],
        field: "closetPositiveTags",
        data_type: DataType::Class,
        target: LOOKALIKE,
        shape: Shape::Struct(LOOKALIKE),
    })
}

#[test]
fn auto_spawns_rejects_a_tag_array_with_incompatible_empty_storage() -> TestResult {
    verify(&Case {
        layout: AutoLayout::WrongTagArray,
        path: &["searchOptions", "closetPositiveTags"],
        field: "tags",
        data_type: DataType::Class,
        target: LOOKALIKE,
        shape: Shape::EmptyArray,
    })
}

#[test]
fn auto_spawns_rejects_an_unrelated_entity_reference_declaration() -> TestResult {
    verify(&Case {
        layout: AutoLayout::WrongEntityTarget,
        path: &["searchOptions"],
        field: "entityDeclaration",
        data_type: DataType::Reference,
        target: "Definition",
        shape: Shape::Dangling,
    })
}

fn verify(case: &Case) -> TestResult {
    for owner in [Owner::Template, Owner::Broker, Owner::Generator] {
        let source = fixture::property("WrongNominal", value(case.layout));
        let fixture = fixture::database_with(vec![source], owner, case.layout);
        let parent = original(&fixture, owner, case)?;
        let normal = fixture::build(&fixture, CatalogPublicationMode::Normal);
        let refusal = normal
            .err()
            .and_then(|error| error.downcast::<CatalogError>().ok());
        assert!(
            matches!(refusal.as_deref(), Some(CatalogError::IncompleteExport(_))),
            "the genuinely incompatible declaration of {} prevents Normal export",
            case.field
        );
        let report = fixture::build(&fixture, CatalogPublicationMode::DiagnosticPartial)?;
        let mut path = case.path.to_vec();
        path.push(case.field);
        let path = path.join(".");
        assert!(
            mission_canonical_health::example(
                &report,
                Some("unsupportedValue"),
                &parent,
                |field| field == path
            )?,
            "the exact failed original declaration {parent}.{path} has a retained diagnostic"
        );
    }
    Ok(())
}

fn value(layout: AutoLayout) -> TestInstance {
    let search = if matches!(layout, AutoLayout::WrongEntityTarget) {
        TestInstance::new(ENTITY).value("entityDeclaration", TestValue::Reference(fixture::MISSING))
    } else {
        let positive = if matches!(layout, AutoLayout::WrongClosetType) {
            TestInstance::new(LOOKALIKE)
        } else {
            TestInstance::new("TagList")
        };
        TestInstance::new(OPTIONS)
            .value("closetPositiveTags", fixture::object(positive))
            .value(
                "closetNegativeTags",
                fixture::object(TestInstance::new("TagList")),
            )
    };
    TestInstance::new(VALUE)
        .value("autoSpawns", TestValue::Reference(fixture::AUTO_TARGET))
        .value("searchOptions", fixture::object(search))
}

fn original(builder: &DataForgeBuilder, owner: Owner, case: &Case) -> TestResult<String> {
    let bytes = builder.build();
    let forge = DataForge::open(&bytes)?;
    let record = forge.record(0).ok_or("actual physical mission owner")?;
    let tree = record.tree()?;
    assert!(
        tree.is_from(&record),
        "original Record and actual tree belong together"
    );
    let mut source = mission_property(&tree, owner)?;
    for member in case.path {
        source = structure(source.field(member))?;
    }
    let field = source
        .fields()
        .iter()
        .find(|field| field.name() == case.field)
        .ok_or("actual declared field")?;
    let storage = field.storage();
    assert_eq!(
        storage.data_type(),
        case.data_type,
        "original storage for {}",
        case.field
    );
    assert_eq!(
        storage.is_array(),
        matches!(case.shape, Shape::EmptyArray),
        "original cardinality"
    );
    let declared = forge
        .structs()
        .nth(usize::from(storage.target()))
        .ok_or("actual declared target")?
        .name()?;
    assert_eq!(
        declared, case.target,
        "genuine nominal target for {}",
        case.field
    );
    original_shape(field.value(), &case.shape)?;
    Ok(source.type_name().to_owned())
}

fn mission_property<'tree, 'source>(
    tree: &'tree RecordTree<'source>,
    owner: Owner,
) -> Result<&'tree StructValue<'source>, Box<dyn std::error::Error>> {
    let properties = match owner {
        Owner::Template => tree.field("contractProperties"),
        Owner::Broker => tree.field("properties"),
        Owner::Generator => {
            let handler = first(tree.field("generators"))?;
            let parameters = structure(handler.field("contractParams"))?;
            parameters.field("propertyOverrides")
        }
    };
    let property = first(properties)?;
    let source = structure(property.field("value"))?;
    assert_eq!(source.type_name(), VALUE, "the original selected property");
    Ok(source)
}

fn original_shape(value: &Value<'_>, shape: &Shape) -> TestResult {
    match shape {
        Shape::Struct(name) => assert_eq!(
            structure(Some(value))?.type_name(),
            *name,
            "concrete original target"
        ),
        Shape::Record(name) => {
            let Value::Reference(reference) = value else {
                return Err("actual Reference resolves before typed checking".into());
            };
            assert_eq!(
                reference.type_name(),
                *name,
                "actual resolved Record family"
            );
            assert_eq!(
                reference.guid().to_string(),
                "79000000-0000-4000-8000-000000000007",
                "exact original independent AutoSpawns GUID"
            );
        }
        Shape::Dangling => assert!(
            matches!(value, Value::Error(ValueError::DanglingReference { guid })
                if guid.to_string() == "79000000-0000-4000-8000-000000000002"),
            "the exact original unresolved entity GUID"
        ),
        Shape::EmptyArray => assert!(
            matches!(value, Value::Array(elements) if elements.is_empty()),
            "no materialized element can reveal the wrong array declaration"
        ),
    }
    Ok(())
}

fn first<'read, 'source>(
    value: Option<&'read Value<'source>>,
) -> TestResult<&'read StructValue<'source>> {
    let Some(Value::Array(values)) = value else {
        return Err("original array".into());
    };
    let [element] = values.as_ref() else {
        return Err("one original declaration".into());
    };
    structure(Some(element))
}

fn structure<'read, 'source>(
    value: Option<&'read Value<'source>>,
) -> TestResult<&'read StructValue<'source>> {
    let Some(Value::Struct(source)) = value else {
        return Err("original owned struct".into());
    };
    Ok(source)
}
