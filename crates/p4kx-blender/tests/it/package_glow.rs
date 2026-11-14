// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::path::Path;

use p4kx_assembly::{ArchiveModels, Assembly, AssemblyOptions, LoadoutResolver};
use p4kx_blender::{PackageInfo, PackageWriteContext, manifest_schema, write_package_with_context};
use p4kx_extraction::{Archive, ArchiveWriteProtection, CancelToken, DataForgeEntry, OpenOptions};
use p4kx_testkit::{
    DataForgeBuilder, SplitDds, TestEntry, TestInstance, TestModel, TestRecord, TestStruct,
    TestTextureFormat, TestType, TestValue, entity_schema, test_key,
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
        TestEntry::stored("Data/Objects/Test/emission.dds", emission_dds()?),
        TestEntry::stored(
            r"Data\Objects\Test\glow.cgf.mtl",
            br#"<Material><SubMaterials><Material/><Material Shader="Illum" MtlFlags="1024"/><Material Shader="HardSurface" StringGenMask="%EMITTANCE_MAP" Glow="2" Emissive="0.2,0.4,0.8" Opacity="0.45"><Textures><Texture Map="TexSlot14" File="objects/test/emission.dds"/></Textures></Material></SubMaterials></Material>"#,
        ).encrypted(),
    ])?;
    let archive = Archive::open_with(fixture.path, OpenOptions::new().key(Some(test_key())))?;
    let input = DataForgeEntry::read(&archive)?;
    let forge = input.open()?;
    let mut resolver = LoadoutResolver::new(&forge).with_forge_entry(&input);
    let record = resolver.entities().find("SyntheticGlow")?;
    let tree = resolver.resolve(&record)?;
    let models = ArchiveModels::new(&archive).with_model_node_sources();
    let assembly = Assembly::build(&tree, &models, &AssemblyOptions::default())?;
    let cancel = CancelToken::new();
    let protection = ArchiveWriteProtection::new(archive.path(), cancel.flag())?;
    let textures = models.textures();
    let context = PackageWriteContext::new(&protection, cancel.flag())
        .with_entity_effects_source(&input, &forge)
        .with_archive_texture_source(&textures);
    let out = tempfile::tempdir()?;
    let location = std::env::var_os("P4KX_SYNTHETIC_GLOW_PACKAGE");
    let package = location.map_or_else(|| out.path().join("scene"), std::path::PathBuf::from);
    let run = || {
        write_package_with_context(
            &assembly,
            &models,
            &textures,
            &glb_options(0),
            &PackageInfo::default(),
            &package,
            &context,
        )
    };
    run()?;
    let first = fs::read(package.join("scene.json"))?;
    validate(&first, &package)?;
    validate_material(&first)?;
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
        Some(p4kx_blender::FORMAT_VERSION),
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

fn validate_material(bytes: &[u8]) -> TestResult {
    let manifest: Value = serde_json::from_slice(bytes)?;
    let materials = manifest
        .get("materials")
        .and_then(Value::as_array)
        .ok_or("materials")?;
    let source = materials
        .iter()
        .find(|row| row.get("shader").and_then(Value::as_str) == Some("HardSurface"))
        .ok_or("visible emission material")?;
    assert_eq!(
        source.pointer("/emission_source/emittance_requested"),
        Some(&Value::Bool(true)),
        "explicit original enabled route",
    );
    assert_eq!(
        source
            .pointer("/emission_source/preview/slot")
            .and_then(Value::as_str),
        Some("TexSlot14"),
        "source slot rather than name heuristic",
    );
    let field_origins = source
        .pointer("/emission_source/field_origins")
        .ok_or("emission origins")?;
    let source_library = source
        .pointer("/emission_source/source_library")
        .and_then(Value::as_u64)
        .ok_or("physical material library")?;
    let inventory = manifest
        .pointer(&format!(
            "/material_libraries/{source_library}/emission_provenance"
        ))
        .ok_or("emission source inventory")?;
    verify_emission_origin(field_origins, inventory)?;
    let texture = source
        .pointer("/textures/0/texture")
        .and_then(Value::as_str)
        .ok_or("published image")?;
    let textures = manifest
        .get("textures")
        .and_then(Value::as_array)
        .ok_or("textures")?;
    let texture = textures
        .iter()
        .find(|row| row.get("id").and_then(Value::as_str) == Some(texture))
        .ok_or("image")?;
    assert_eq!(
        texture.get("encoding").and_then(Value::as_str),
        Some("linear_colour"),
        "authored linear image transfer",
    );
    assert_eq!(
        texture.pointer("/source_metadata/field_origins/~1image/containsEncryptedSource"),
        Some(&Value::Bool(false)),
        "the encoded texture did not consume the encrypted material bytes",
    );
    let _: p4kx_blender::Manifest = serde_json::from_slice(bytes)?;
    Ok(())
}

fn verify_emission_origin(origins: &Value, inventory: &Value) -> TestResult {
    let attribute = origins
        .pointer("/~1attributes~12~1source~1raw")
        .ok_or("raw Glow coordinate receipt")?;
    assert_eq!(
        attribute.get("containsEncryptedSource"),
        Some(&Value::Bool(true)),
        "raw Glow depends on the encrypted XML payload",
    );
    let typed = origins
        .pointer("/~1attributes~12~1value~1value~1value")
        .ok_or("typed Glow coordinate receipt")?;
    assert_eq!(
        typed.get("containsEncryptedSource"),
        Some(&Value::Bool(true)),
        "converted numeric Glow depends on the encrypted original declaration",
    );
    assert_eq!(
        origins.pointer("/~1preview~1strength/containsEncryptedSource"),
        Some(&Value::Bool(true)),
        "static preview strength retains exact authored inputs",
    );
    verify_physical_glow_use(attribute, inventory)
}

fn verify_physical_glow_use(attribute: &Value, inventory: &Value) -> TestResult {
    let ids = attribute
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("source-use references")?;
    let observed = inventory
        .get("uses")
        .and_then(Value::as_array)
        .ok_or("physical uses")?;
    let [id] = ids.as_slice() else {
        return Err("expected one original raw Glow use".into());
    };
    let used = observed
        .iter()
        .find(|row| row.get("id") == Some(id))
        .ok_or("original use")?;
    assert_eq!(
        used.pointer("/location/field").and_then(Value::as_str),
        Some("Glow"),
        "original XML attribute",
    );
    assert_eq!(
        used.pointer("/location/indices"),
        Some(&serde_json::json!(["0", "2", "2"])),
        "original list, material and attribute positions",
    );
    let sources = inventory
        .get("sources")
        .and_then(Value::as_array)
        .ok_or("physical sources")?;
    let physical = sources
        .iter()
        .find(|row| row.get("id") == used.get("source"))
        .ok_or("original physical Entry")?;
    assert_eq!(
        physical
            .pointer("/storedEncryption")
            .and_then(Value::as_str),
        Some("encrypted"),
        "source Entry owns the retained payload",
    );
    Ok(())
}

fn emission_dds() -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let dds = SplitDds::new(
        TestTextureFormat::Dx10Block {
            dxgi_format: 28,
            block_bytes: 4,
        },
        1,
        1,
    );
    let mut bytes = dds.merged_bytes();
    let payload = bytes
        .get_mut(dds.header_len()..)
        .ok_or("RGBA payload span")?;
    if payload.len() != 4 {
        return Err("expected one exact RGBA pixel".into());
    }
    payload.copy_from_slice(&[128, 64, 32, 255]);
    Ok(bytes)
}
