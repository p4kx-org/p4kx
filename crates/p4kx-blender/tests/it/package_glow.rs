// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::path::Path;

use p4kx_assembly::{ArchiveModels, Assembly, AssemblyOptions, LoadoutResolver};
use p4kx_blender::{PackageInfo, PackageWriteContext, manifest_schema, write_package_with_context};
use p4kx_extraction::{Archive, ArchiveWriteProtection, CancelToken, DataForgeEntry};
use p4kx_testkit::{
    DataForgeBuilder, TestEntry, TestInstance, TestModel, TestRecord, TestStruct, TestType,
    TestValue, entity_schema,
};
use serde_json::Value;

use crate::support::{TestResult, fixture, glb_options};

const MODEL_PATH: &str = "Objects/Test/glow.cgf";

#[test]
fn owner_glow_declaration_and_original_primitive_binding_are_published_by_the_package_writer()
-> TestResult {
    let model = TestModel::cube().with_material_library("glow");
    let fixture = fixture(vec![
        TestEntry::stored(r"Data\Game2.dcb", database()),
        TestEntry::stored(format!("Data/{MODEL_PATH}"), model.cgf()),
        TestEntry::stored("Data/Objects/Test/glow.cgfm", model.cgfm()),
        TestEntry::stored(
            r"Data\Objects\Test\glow.cgf.mtl",
            br#"<Material><SubMaterials><Material/><Material Shader="Illum" MtlFlags="1024"/><Material Shader="HardSurface"/></SubMaterials></Material>"#,
        ),
    ])?;
    let archive = Archive::open(fixture.path)?;
    let input = DataForgeEntry::read(&archive)?;
    let forge = input.open()?;
    let mut resolver = LoadoutResolver::new(&forge).with_forge_entry(&input);
    let record = resolver.entities().find("SyntheticGlow")?;
    let tree = resolver.resolve(&record)?;
    let models = ArchiveModels::new(&archive).with_model_node_sources();
    let assembly = Assembly::build(&tree, &models, &AssemblyOptions::default())?;
    let cancel = CancelToken::new();
    let protection = ArchiveWriteProtection::new(archive.path(), cancel.flag())?;
    let context = PackageWriteContext::new(&protection, cancel.flag())
        .with_entity_effects_source(&input, &forge);
    let out = tempfile::tempdir()?;
    let location = std::env::var_os("P4KX_SYNTHETIC_GLOW_PACKAGE");
    let package = location.map_or_else(|| out.path().join("scene"), std::path::PathBuf::from);
    let run = || {
        write_package_with_context(
            &assembly,
            &models,
            &models.textures(),
            &glb_options(0),
            &PackageInfo::default(),
            &package,
            &context,
        )
    };
    run()?;
    let first = fs::read(package.join("scene.json"))?;
    validate(&first, &package)?;
    run()?;
    assert_eq!(
        fs::read(package.join("scene.json"))?,
        first,
        "owner source schema is deterministic"
    );
    Ok(())
}

fn validate(bytes: &[u8], package: &Path) -> TestResult {
    let manifest: Value = serde_json::from_slice(bytes)?;
    jsonschema::validator_for(&manifest_schema())?
        .validate(&manifest)
        .map_err(|error| error.to_string())?;
    assert_eq!(
        manifest.get("version").and_then(Value::as_str),
        Some("1.18"),
        "authored helper version"
    );
    let authored = "/entity_glow/declarations/0/components/0";
    let selector = "/tagEffects/0/propertyLinks/0/subMeshName";
    assert_eq!(
        manifest
            .pointer(&format!("{authored}/authored{selector}"))
            .and_then(Value::as_str),
        Some("cube_mesh"),
        "original declaration text",
    );
    assert_eq!(
        manifest.pointer(&format!("{authored}/field_origins/~1tagEffects~10~1propertyLinks~10~1subMeshName/containsEncryptedSource")),
        Some(&Value::Bool(false)),
        "independent original DataForge selector evidence",
    );
    let owner = "/entity_glow/owners/0";
    assert_eq!(
        manifest.pointer(&format!("{owner}/node")),
        Some(&Value::from(0)),
        "actual Package node"
    );
    let subset = format!("{owner}/links/0/selection/subsets/0");
    assert_eq!(
        manifest
            .pointer(&format!("{subset}/node_name/value"))
            .and_then(Value::as_str),
        Some("cube_mesh"),
        "actual source NMC node"
    );
    assert_eq!(
        manifest.pointer(&format!("{subset}/material/source_slot")),
        Some(&Value::from(1)),
        "original subset material slot"
    );
    assert_eq!(
        manifest.pointer(&format!("{subset}/material/no_draw/value")),
        Some(&Value::Bool(true)),
        "source no-draw remains explicit"
    );
    let receipt: Value = serde_json::from_slice(&fs::read(package.join("receipt.json"))?)?;
    let files = receipt
        .get("files")
        .and_then(Value::as_array)
        .ok_or("receipt inventory")?;
    assert!(
        files
            .iter()
            .any(|row| row.get("path").and_then(Value::as_str) == Some("scene.json")),
        "manifest with evidence is covered by the package receipt"
    );
    Ok(())
}

fn database() -> Vec<u8> {
    let geometry = value(
        TestInstance::new("SGeometryResourceParams").value(
            "Geometry",
            value(
                TestInstance::new("SGeometryNodeParams").value(
                    "Geometry",
                    value(
                        TestInstance::new("SGeometryDataParams").value(
                            "Geometry",
                            value(
                                TestInstance::new("GlobalResourceGeometry")
                                    .value("path", TestValue::text(MODEL_PATH)),
                            ),
                        ),
                    ),
                ),
            ),
        ),
    );
    let class = |name: &str| TestType::Class(name.to_owned());
    entity_schema(DataForgeBuilder::new())
        .structure(
            TestStruct::new("SEntityComponentEffects")
                .parent("DataForgeComponentParams")
                .field("entityEffects", class("Effects")),
        )
        .structure(TestStruct::new("Effects").field("materialEffects", class("MaterialEffects")))
        .structure(TestStruct::new("MaterialEffects").array("tagEffects", class("Effect")))
        .structure(TestStruct::new("Effect").array("propertyLinks", class("PropertyLink")))
        .structure(
            TestStruct::new("PropertyLink")
                .field("materialProperty", TestType::String)
                .field("subMeshName", TestType::String),
        )
        .record(
            TestRecord::new(
                42,
                "EntityClassDefinition",
                "EntityClassDefinition.SyntheticGlow",
                "synthetic.xml",
            )
            .value("Components", TestValue::Array(vec![geometry, effects()])),
        )
        .build()
}

fn effects() -> TestValue {
    let link = value(
        TestInstance::new("PropertyLink")
            .value("materialProperty", TestValue::text("Glow"))
            .value("subMeshName", TestValue::text("cube_mesh")),
    );
    let effect =
        value(TestInstance::new("Effect").value("propertyLinks", TestValue::Array(vec![link])));
    let group = value(
        TestInstance::new("MaterialEffects").value("tagEffects", TestValue::Array(vec![effect])),
    );
    value(TestInstance::new("SEntityComponentEffects").value(
        "entityEffects",
        value(TestInstance::new("Effects").value("materialEffects", group)),
    ))
}

const fn value(input: TestInstance) -> TestValue {
    TestValue::Struct(input)
}
