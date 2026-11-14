// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogPublicationMode, build_catalog};
use p4kx_extraction::{Archive, DataForgeEntry, Value as DataForgeValue};
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::health_origin_assertions as proof;

#[path = "catalog_ammo_geometry_nested.rs"]
mod nested;

const ROOT: &str = "SGeometryResourceParams";
const MODEL_PATH: &str = "fixture/Geo/Model.cgf";
const MATERIAL_PATH: &str = "fixture/Materials/Finish.mtl";

#[test]
fn ammo_geometry_preserves_separate_authored_node_arenas() -> TestResult {
    let (_folder, archive) = common::open(&database(pointer(ROOT)), DcbOrigin::Encrypted)?;
    assert_original_geometry(&archive)?;
    let catalog: Value =
        serde_json::from_slice(build_catalog(&archive, &common::options())?.json())?;
    let row = proof::at(&catalog, "/definitions/ammoDefinitions/rows/0")?;
    let source = proof::at(
        proof::cell(row, "geometryResourceParams", "present")?,
        "/value",
    )?;
    proof::present(source, "cacheResources", &json!(false), Some(true))?;
    check_geometry(source)?;
    check_material(source)?;
    proof::closure(&catalog)
}

#[test]
fn ammo_geometry_rejects_the_same_payload_in_a_non_pointer_root() -> TestResult {
    let (_folder, archive) = common::open(&database(class(ROOT)), DcbOrigin::Encrypted)?;
    assert_original_geometry(&archive)?;
    let mut options = common::options();
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    let catalog: Value = serde_json::from_slice(build_catalog(&archive, &options)?.json())?;
    let row = proof::at(&catalog, "/definitions/ammoDefinitions/rows/0")?;
    let rejected = proof::cell(row, "geometryResourceParams", "invalid")?;
    let uses = proof::uses(&catalog, rejected)?;
    assert!(
        uses.iter().any(|used| {
            used.get("role").and_then(Value::as_str) == Some("selector")
                && used.pointer("/location/field").and_then(Value::as_str) == Some("$dataType")
        }),
        "the rejected geometry root retains the actual declaration storage"
    );
    proof::present(row, "size", &json!(0), Some(true))?;
    proof::closure(&catalog)
}

fn check_geometry(source: &Value) -> TestResult {
    proof::present(source, "geometry", &json!(0), Some(true))?;
    let [root, first, second, grandchild] = proof::rows(source, "/geometryNodes")? else {
        return Err("expected the root, two original siblings and their nested node".into());
    };
    assert_eq!(
        proof::at(root, "/subGeometry/value/0/value")?,
        &json!(1),
        "the first source slot points to the first local child"
    );
    assert_eq!(
        proof::at(root, "/subGeometry/value/1/value")?,
        &json!(2),
        "the second source slot keeps its own local child"
    );
    assert_eq!(
        proof::at(first, "/subGeometry/value/0/value")?,
        &json!(3),
        "the nested child follows the siblings in discovery order"
    );
    assert_eq!(
        proof::at(second, "/subGeometry/value")?,
        &json!([]),
        "an empty authored array stays present"
    );
    proof::present(grandchild, "tags", &json!("grandchild"), Some(true))?;
    proof::present(first, "tags", &json!("Repeated"), Some(true))?;
    proof::present(second, "tags", &json!("Repeated"), Some(true))?;
    assert_ne!(
        proof::at(first, "/tags/sourceUsesRefs")?,
        proof::at(second, "/tags/sourceUsesRefs")?,
        "matching tag strings are two original node slots"
    );
    assert_eq!(
        proof::at(root, "/scaleMultiplier/value/rawBits")?,
        "80000000",
        "the original negative zero retains its sign bit"
    );
    let model = proof::at(root, "/data/value/geometry/value")?;
    proof::present(model, "path", &json!(MODEL_PATH), Some(true))?;
    Ok(())
}

fn check_material(source: &Value) -> TestResult {
    proof::present(source, "material", &json!(0), Some(true))?;
    let [root, child] = proof::rows(source, "/materialNodes")? else {
        return Err("expected root material and its original variant".into());
    };
    assert_eq!(
        proof::at(root, "/materialVariants/value/0/value")?,
        &json!(1),
        "the original material variant uses its own local arena"
    );
    let resource = proof::at(child, "/material/value")?;
    proof::present(resource, "path", &json!(MATERIAL_PATH), Some(true))?;
    Ok(())
}

fn assert_original_geometry(archive: &Archive) -> TestResult {
    let entry = DataForgeEntry::read(archive)?;
    let forge = entry.open()?;
    let tree = forge.record(0).ok_or("original Ammo Record")?.tree()?;
    let Some(DataForgeValue::Struct(resource)) = tree.field("geometryResourceParams") else {
        return Err("expected readable original geometry resource".into());
    };
    let Some(DataForgeValue::Struct(geometry)) = resource.field("Geometry") else {
        return Err("expected original geometry node".into());
    };
    let Some(DataForgeValue::Array(children)) = geometry.field("SubGeometry") else {
        return Err("expected original geometry child array".into());
    };
    assert_eq!(
        children.len(),
        2,
        "the source has two authored sibling slots"
    );
    assert!(
        matches!(geometry.field("ScaleMultiplier"), Some(DataForgeValue::Single(value)) if value.to_bits() == 0x8000_0000),
        "the physical root stores the exact negative zero word"
    );
    Ok(())
}

fn database(root_storage: TestType) -> DataForgeBuilder {
    declarations(root_storage).record(
        TestRecord::new(
            0x8270,
            "AmmoParams",
            "AmmoParams.GeometryFixture",
            "fixture/ammo_geometry.xml",
        )
        .value("size", TestValue::UInt8(0))
        .value("geometryResourceParams", resource()),
    )
}

fn declarations(root_storage: TestType) -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new("AmmoParams")
                .field("size", TestType::UInt8)
                .field("geometryResourceParams", root_storage),
        )
        .structure(
            TestStruct::new(ROOT)
                .field("cacheResources", TestType::Bool)
                .field("Geometry", class("SGeometryNodeParams"))
                .field("Material", class("SMaterialNodeParams")),
        )
        .structure(
            TestStruct::new("SGeometryNodeParams")
                .field("Tags", TestType::String)
                .field("Geometry", class("SGeometryDataParams"))
                .field("ScaleMultiplier", TestType::Single)
                .array("SubGeometry", class("SGeometryNodeParams")),
        )
        .structure(
            TestStruct::new("SGeometryDataParams")
                .field("Geometry", class("GlobalResourceGeometry")),
        )
        .structure(
            TestStruct::new("SMaterialNodeParams")
                .field("Tags", TestType::String)
                .field("Material", class("GlobalResourceMaterial"))
                .array("materialVariants", class("SMaterialNodeParams")),
        )
        .structure(TestStruct::new("GlobalResourceGeometry").field("path", TestType::String))
        .structure(TestStruct::new("GlobalResourceMaterial").field("path", TestType::String))
}

fn resource() -> TestValue {
    let geometry_data = TestInstance::new("SGeometryDataParams")
        .value("Geometry", path("GlobalResourceGeometry", MODEL_PATH));
    let first = geometry_node("Repeated", vec![node(geometry_node("grandchild", vec![]))]);
    let second = geometry_node("Repeated", vec![]);
    let geometry = geometry_node("root", vec![node(first), node(second)])
        .value("Geometry", node(geometry_data))
        .value("ScaleMultiplier", TestValue::Single(-0.0));
    let variant = TestInstance::new("SMaterialNodeParams")
        .value("Tags", TestValue::text("finish"))
        .value("Material", path("GlobalResourceMaterial", MATERIAL_PATH));
    let material = TestInstance::new("SMaterialNodeParams")
        .value("Tags", TestValue::text("root"))
        .value("materialVariants", TestValue::Array(vec![node(variant)]));
    node(
        TestInstance::new(ROOT)
            .value("cacheResources", TestValue::Bool(false))
            .value("Geometry", node(geometry))
            .value("Material", node(material)),
    )
}

fn geometry_node(tags: &str, children: Vec<TestValue>) -> TestInstance {
    TestInstance::new("SGeometryNodeParams")
        .value("Tags", TestValue::text(tags))
        .value("SubGeometry", TestValue::Array(children))
}

fn path(source: &str, value: &str) -> TestValue {
    node(TestInstance::new(source).value("path", TestValue::text(value)))
}

fn pointer(target: &str) -> TestType {
    TestType::StrongPointer(target.to_owned())
}

fn class(target: &str) -> TestType {
    TestType::Class(target.to_owned())
}

const fn node(value: TestInstance) -> TestValue {
    TestValue::Struct(value)
}
