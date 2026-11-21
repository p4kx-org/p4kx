// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::build_catalog;
use p4kx_extraction::{Archive, DataForgeEntry, Value as Raw};
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::health_origin_assertions as proof;

#[path = "catalog_ammo_geometry_derived.rs"]
mod derived;

const MODEL_TAG: u128 = 0x8291;
const PALETTE: u128 = 0x8292;
const DISTANCE: u128 = 0x8293;
const ATTACH_FLAGS: u32 = 4_000_000_000;
const SHAPE_EXCLUDE: u16 = 65_000;
const LOD: u8 = 240;
const SIMULATION_PATH: &str = "fixture/Physics/SoftMesh.cgf";

#[test]
fn ammo_geometry_keeps_typed_nested_fields_and_nominal_links() -> TestResult {
    let (_folder, archive) =
        common::open(&database("SGeometryActorModelTag"), DcbOrigin::Encrypted)?;
    verify_original_data(&archive)?;
    let built = build_catalog(&archive, &common::options())?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let root = proof::at(
        &catalog,
        "/definitions/ammoDefinitions/rows/0/geometryResourceParams/value",
    )?;
    proof::present(root, "rootOverridePaint", &json!(true), Some(true))?;
    proof::present(root, "inheritModelTagFromHost", &json!(false), Some(true))?;
    proof::present(
        root,
        "inheritMaterialFromSpawnHost",
        &json!(true),
        Some(true),
    )?;
    check_tags(root)?;
    let data = proof::at(root, "/geometryNodes/0/data/value")?;
    check_geometry_data(data)?;
    check_simulation(data)?;
    let schema = p4kx_export::catalog_schema();
    let validator = jsonschema::validator_for(&schema)?;
    validator
        .validate(&catalog)
        .map_err(|error| error.to_string())?;
    proof::closure(&catalog)
}

fn check_tags(root: &Value) -> TestResult {
    let actor = proof::at(root, "/modelTag/value")?;
    assert_eq!(
        proof::at(actor, "/type")?,
        "actor",
        "the concrete Actor alternative is retained"
    );
    check_reference(proof::at(actor, "/tag/value")?, MODEL_TAG)?;
    let [first, second] = proof::rows(root, "/meshSetup/value/visibilityTags/value/tags/value")?
    else {
        return Err("two original mesh visibility tag slots".into());
    };
    for slot in [first, second] {
        check_reference(proof::at(slot, "/value")?, MODEL_TAG)?;
    }
    assert_ne!(
        proof::at(first, "/sourceUsesRefs")?,
        proof::at(second, "/sourceUsesRefs")?,
        "repeated tags retain their actual authored positions"
    );
    Ok(())
}

fn check_geometry_data(data: &Value) -> TestResult {
    proof::present(data, "attachFlags", &json!(ATTACH_FLAGS), Some(true))?;
    proof::present(
        data,
        "protosBShapeExclude",
        &json!(SHAPE_EXCLUDE),
        Some(true),
    )?;
    proof::present(data, "lodRatio", &json!(LOD), Some(true))?;
    proof::present(data, "slot", &json!("FixtureSlot"), Some(true))?;
    proof::present(data, "visFp", &json!(false), Some(true))?;
    check_reference(proof::at(data, "/viewDistRatio/value")?, DISTANCE)?;
    let palette = proof::at(data, "/palette/value")?;
    check_reference(proof::at(palette, "/rootRecord/value")?, PALETTE)?;
    proof::present(palette, "childPath", &json!("Fixture/Swatches"), Some(true))?;
    let swizzle = proof::at(palette, "/swizzleOverride/value")?;
    proof::present(swizzle, "primary", &json!(2), Some(true))?;
    proof::present(swizzle, "secondary", &json!(0), Some(true))?;
    proof::present(swizzle, "tertiary", &json!(1), Some(true))?;
    Ok(())
}

fn check_simulation(data: &Value) -> TestResult {
    let sim = proof::at(data, "/simulationGeometry/value")?;
    proof::present(
        proof::at(sim, "/geometry/value")?,
        "path",
        &json!(SIMULATION_PATH),
        Some(true),
    )?;
    let params = proof::at(sim, "/simulationParams/value")?;
    proof::present(params, "compoundingAllowed", &json!(false), Some(true))?;
    assert_eq!(
        proof::at(params, "/mass/value/rawBits")?,
        "3fa00000",
        "the actual inherited mass word is projected without conversion"
    );
    proof::present(
        params,
        "enforceCosseratStretchShear",
        &json!(true),
        Some(true),
    )?;
    proof::present(params, "gridDim", &json!(-3), Some(true))?;
    proof::present(
        params,
        "visualBindingMode",
        &json!("FixtureBinding"),
        Some(true),
    )?;
    assert_eq!(
        proof::at(params, "/bindingOffset/value/rawBits")?,
        "80000000",
        "the controller retains the exact source sign bit"
    );
    Ok(())
}

fn check_reference(reference: &Value, guid: u128) -> TestResult {
    assert_eq!(
        proof::at(reference, "/status")?,
        "present",
        "the nominal original target is readable"
    );
    proof::present(reference, "guid", &json!(common::guid(guid)), Some(true))?;
    Ok(())
}

fn verify_original_data(archive: &Archive) -> TestResult {
    let entry = DataForgeEntry::read(archive)?;
    let forge = entry.open()?;
    let tree = forge.record(0).ok_or("physical Ammo Record")?.tree()?;
    let Some(Raw::Struct(root)) = tree.field("geometryResourceParams") else {
        return Err("original geometry resource".into());
    };
    let Some(Raw::Struct(node)) = root.field("Geometry") else {
        return Err("original geometry root".into());
    };
    let Some(Raw::Struct(data)) = node.field("Geometry") else {
        return Err("original geometry data".into());
    };
    assert!(
        matches!(data.field("AttachFlags"), Some(Raw::UInt32(ATTACH_FLAGS)))
            && matches!(
                data.field("ProtosBShapeExclude"),
                Some(Raw::UInt16(SHAPE_EXCLUDE))
            )
            && matches!(data.field("LodRatio"), Some(Raw::UInt8(LOD))),
        "all three literal integer widths are present in the Record Tree"
    );
    let Some(Raw::Struct(sim)) = data.field("SimulationGeometry") else {
        return Err("original simulation geometry".into());
    };
    let Some(Raw::Struct(params)) = sim.field("SimulationParams") else {
        return Err("original simulation controller".into());
    };
    assert!(
        matches!(params.field("Mass"), Some(Raw::Single(value)) if value.to_bits() == 0x3fa0_0000),
        "the original inherited mass appears in the controller Record Tree"
    );
    assert!(
        matches!(
            params.field("enforceCosseratStrechShear"),
            Some(Raw::Bool(true))
        ),
        "the original declaration uses the exact source spelling"
    );
    Ok(())
}

fn database(model_tag_type: &str) -> DataForgeBuilder {
    declarations(model_tag_type)
        .record(
            TestRecord::new(
                0x8290,
                "AmmoParams",
                "AmmoParams.NestedGeometryFixture",
                "fixture/ammo_nested_geometry.xml",
            )
            .value("geometryResourceParams", resource(model_tag_type)),
        )
        .record(TestRecord::new(
            MODEL_TAG,
            "Tag",
            "Tag.Geometry",
            "fixture/geometry_tag.xml",
        ))
        .record(TestRecord::new(
            PALETTE,
            "TintPaletteTree",
            "TintPaletteTree.Fixture",
            "fixture/geometry_palette.xml",
        ))
        .record(TestRecord::new(
            DISTANCE,
            "SViewDistanceRatioParams",
            "SViewDistanceRatioParams.Fixture",
            "fixture/geometry_distance.xml",
        ))
}

fn declarations(model_tag_type: &str) -> DataForgeBuilder {
    let forge = DataForgeBuilder::new()
        .version(8)
        .enumeration("FixtureSlotType", ["FixtureSlot"])
        .enumeration("FixtureBindingType", ["FixtureBinding"])
        .structure(
            TestStruct::new("AmmoParams")
                .field("geometryResourceParams", pointer("SGeometryResourceParams")),
        )
        .structure(
            TestStruct::new("SGeometryResourceParams")
                .field("ModelTag", pointer("SGeometryModelTagBase"))
                .field("meshsetup", class("SGeometryMeshsetupParams"))
                .field("Geometry", class("SGeometryNodeParams"))
                .field("rootOverridePaint", TestType::Bool)
                .field("inheritModelTagFromHost", TestType::Bool)
                .field("inheritMaterialFromSpawnHost", TestType::Bool),
        )
        .structure(TestStruct::new("SGeometryModelTagBase"))
        .structure(
            TestStruct::new(model_tag_type)
                .parent("SGeometryModelTagBase")
                .field("tag", reference("Tag")),
        )
        .structure(
            TestStruct::new("SGeometryMeshsetupParams").field("visibilityTags", class("TagList")),
        )
        .structure(TestStruct::new("TagList").array("tags", reference("Tag")))
        .structure(
            TestStruct::new("SGeometryNodeParams").field("Geometry", class("SGeometryDataParams")),
        );
    data_declarations(forge)
        .structure(TestStruct::new("Tag"))
        .structure(TestStruct::new("TintPaletteTree"))
        .structure(TestStruct::new("SViewDistanceRatioParams"))
}

fn data_declarations(forge: DataForgeBuilder) -> DataForgeBuilder {
    forge
        .structure(
            TestStruct::new("SGeometryDataParams")
                .field("AttachFlags", TestType::UInt32)
                .field("ProtosBShapeExclude", TestType::UInt16)
                .field("LodRatio", TestType::UInt8)
                .field("Slot", TestType::Enum("FixtureSlotType".to_owned()))
                .field("VisFP", TestType::Bool)
                .field("viewDistRatio", reference("SViewDistanceRatioParams"))
                .field("Palette", class("TintPaletteRef"))
                .field("SimulationGeometry", pointer("SSoftbodyGeometryParams")),
        )
        .structure(
            TestStruct::new("TintPaletteRef")
                .field("RootRecord", reference("TintPaletteTree"))
                .field("ChildPath", TestType::String)
                .field("SwizzleOverride", pointer("TintPaletteSwizzle")),
        )
        .structure(
            TestStruct::new("TintPaletteSwizzle")
                .field("primary", TestType::Int32)
                .field("secondary", TestType::Int32)
                .field("tertiary", TestType::Int32),
        )
        .structure(TestStruct::new("GlobalResourceGeometry").field("path", TestType::String))
        .structure(
            TestStruct::new("SSoftbodyGeometryParams")
                .field("Geometry", class("GlobalResourceGeometry"))
                .field(
                    "SimulationParams",
                    class("SEntitySoftExPhysicsControllerParams"),
                ),
        )
        .structure(
            TestStruct::new("SEntityBasePhysicsControllerParams")
                .field("Mass", TestType::Single)
                .field("compoundingAllowed", TestType::Bool),
        )
        .structure(
            TestStruct::new("SEntitySoftExPhysicsControllerParams")
                .parent("SEntityBasePhysicsControllerParams")
                .field("bindingOffset", TestType::Single)
                .field("enforceCosseratStrechShear", TestType::Bool)
                .field("gridDim", TestType::Int32)
                .field(
                    "visualBindingMode",
                    TestType::Enum("FixtureBindingType".to_owned()),
                ),
        )
}

fn resource(model_tag_type: &str) -> TestValue {
    let actor = TestInstance::new(model_tag_type).value("tag", TestValue::Reference(MODEL_TAG));
    let visibility = TestInstance::new("TagList").value(
        "tags",
        TestValue::Array(vec![
            TestValue::Reference(MODEL_TAG),
            TestValue::Reference(MODEL_TAG),
        ]),
    );
    let setup =
        TestInstance::new("SGeometryMeshsetupParams").value("visibilityTags", node(visibility));
    let root = TestInstance::new("SGeometryNodeParams").value("Geometry", geometry_data());
    node(
        TestInstance::new("SGeometryResourceParams")
            .value("ModelTag", node(actor))
            .value("meshsetup", node(setup))
            .value("Geometry", node(root))
            .value("rootOverridePaint", TestValue::Bool(true))
            .value("inheritModelTagFromHost", TestValue::Bool(false))
            .value("inheritMaterialFromSpawnHost", TestValue::Bool(true)),
    )
}

fn geometry_data() -> TestValue {
    let swizzle = TestInstance::new("TintPaletteSwizzle")
        .value("primary", TestValue::Int32(2))
        .value("secondary", TestValue::Int32(0))
        .value("tertiary", TestValue::Int32(1));
    let palette = TestInstance::new("TintPaletteRef")
        .value("RootRecord", TestValue::Reference(PALETTE))
        .value("ChildPath", TestValue::text("Fixture/Swatches"))
        .value("SwizzleOverride", node(swizzle));
    node(
        TestInstance::new("SGeometryDataParams")
            .value("AttachFlags", TestValue::UInt32(ATTACH_FLAGS))
            .value("ProtosBShapeExclude", TestValue::UInt16(SHAPE_EXCLUDE))
            .value("LodRatio", TestValue::UInt8(LOD))
            .value("Slot", TestValue::text("FixtureSlot"))
            .value("VisFP", TestValue::Bool(false))
            .value("viewDistRatio", TestValue::Reference(DISTANCE))
            .value("Palette", node(palette))
            .value("SimulationGeometry", simulation()),
    )
}

fn simulation() -> TestValue {
    let geometry =
        TestInstance::new("GlobalResourceGeometry").value("path", TestValue::text(SIMULATION_PATH));
    let controller = TestInstance::new("SEntitySoftExPhysicsControllerParams")
        .value("Mass", TestValue::Single(1.25))
        .value("compoundingAllowed", TestValue::Bool(false))
        .value("bindingOffset", TestValue::Single(-0.0))
        .value("enforceCosseratStrechShear", TestValue::Bool(true))
        .value("gridDim", TestValue::Int32(-3))
        .value("visualBindingMode", TestValue::text("FixtureBinding"));
    node(
        TestInstance::new("SSoftbodyGeometryParams")
            .value("Geometry", node(geometry))
            .value("SimulationParams", node(controller)),
    )
}

fn pointer(target: &str) -> TestType {
    TestType::StrongPointer(target.to_owned())
}

fn class(target: &str) -> TestType {
    TestType::Class(target.to_owned())
}

fn reference(target: &str) -> TestType {
    TestType::ReferenceTo(target.to_owned())
}

const fn node(value: TestInstance) -> TestValue {
    TestValue::Struct(value)
}
