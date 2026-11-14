// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{ArchiveModels, Interior, InteriorOptions};
use p4kx_blender::{
    AuthoredMaterialOrigin, InteriorScene, Manifest, PackageInfo, write_interior_package,
};
use p4kx_extraction::{Archive, FieldPath, OpenOptions, select_entries};
use p4kx_testkit::{
    SOC_IDENTITY, TestContainer, TestEntry, TestModel, TestObjectContainer, TestSoc, test_key,
};
use serde_json::Value;

use crate::support::{TestResult, fixture, glb_options, never};

#[test]
fn a_plain_interior_material_keeps_its_encrypted_container_ancestor() -> TestResult {
    const CONTAINER: &str = "Data/ObjectContainers/fixture.socpak";
    const MODEL: &str = "Objects/Test/fixture";
    let model = TestModel::cube().with_material_library("fixture");
    let scene = TestSoc::new()
        .model("objects/test/fixture.cgf")
        .object(0, SOC_IDENTITY);
    let description = TestObjectContainer::new("fixture").stream("fixture.soc");
    let container = TestContainer::new()
        .entry(TestEntry::stored("fixture.xml", description.xml()))
        .entry(TestEntry::deflated("fixture.soc", scene.bytes()))
        .entry(TestEntry::stored(format!("{MODEL}.cgf"), model.cgf()))
        .entry(TestEntry::stored(format!("{MODEL}.cgfm"), model.cgfm()))
        .entry(TestEntry::stored(
            format!("{MODEL}.mtl"),
            br#"<Material Shader="Illum" Glow="2"><Opaque Data="first&amp;last"><Nested/></Opaque></Material>"#,
        ));
    let fixture = fixture(vec![
        TestEntry::stored(CONTAINER, container.build()).encrypted(),
    ])?;
    let archive = Archive::open_with(fixture.path, OpenOptions::new().key(Some(test_key())))?;
    let entries = select_entries(&archive, &[CONTAINER])?;
    let entry = entries
        .first()
        .ok_or("the encrypted container must be admitted")?;
    let opened = archive.open_container(entry)?;
    let interior = Interior::build(
        &[&archive, &opened],
        entry.path(),
        &InteriorOptions::default(),
    )?;
    let models = ArchiveModels::new(&archive);
    let folder = tempfile::tempdir()?;
    let target = folder.path().join("scene");
    write_interior_package(
        &InteriorScene::new("fixture", &interior),
        &models,
        &models.textures(),
        &glb_options(0),
        &PackageInfo::default(),
        &target,
        &never,
    )?;
    let manifest: Manifest = serde_json::from_slice(&std::fs::read(target.join("scene.json"))?)?;
    check_encrypted_material(&manifest)?;
    check_authored_material(&manifest)
}

fn check_encrypted_material(manifest: &Manifest) -> TestResult {
    let material = manifest
        .materials
        .first()
        .ok_or_else(|| format!("the placed model's material: {:?}", manifest.issues))?;
    let emission = material
        .emission_source
        .as_ref()
        .ok_or("authored Glow on the placed model")?;
    let raw = FieldPath::parse("/attributes/1/source/raw")?;
    let origin = emission
        .field_origins
        .get(&raw)
        .ok_or("Glow origin in the Interior")?;
    assert_eq!(
        origin.contains_encrypted_source(),
        Some(true),
        "the original effective encryption includes the containing Entry"
    );
    assert!(
        origin.source_set_complete(),
        "the whole containing context is established"
    );
    let inventory = manifest
        .material_libraries
        .get(emission.source_library)
        .and_then(|library| library.emission_provenance.as_ref())
        .ok_or("the library has physical provenance")?;
    let sources = inventory
        .get("sources")
        .and_then(Value::as_array)
        .ok_or("the original library and ancestor sources")?;
    assert_eq!(
        sources.len(),
        2,
        "both the plain leaf and encrypted parent survive"
    );
    Ok(())
}

fn check_authored_material(manifest: &Manifest) -> TestResult {
    let material = manifest.materials.first().ok_or("placed model material")?;
    let library = manifest
        .material_libraries
        .get(material.source_library.ok_or("material library link")?)
        .ok_or("package source row")?;
    let origins = library
        .authored_origin
        .as_ref()
        .ok_or("authored provenance")?;
    let row = serde_json::to_value(library)?;
    for (field_path, _) in origins.field_origins.iter() {
        assert!(
            row.pointer(field_path.as_str()).is_some(),
            "field pointer {} refers to an emitted value",
            field_path.as_str()
        );
    }
    let pointer = "/authored/root/children/0/syntax/attributes/0/lexeme";
    assert_eq!(
        row.pointer(pointer).and_then(Value::as_str),
        Some("first&amp;last"),
        "source XML retains the literal token on an unknown element"
    );
    check_authored_inventory(origins, pointer)
}

fn check_authored_inventory(origins: &AuthoredMaterialOrigin, pointer: &str) -> TestResult {
    let field = origins
        .field_origins
        .get(&FieldPath::parse(pointer)?)
        .ok_or("original authored field")?;
    assert_eq!(
        field.contains_encrypted_source(),
        Some(true),
        "effective parent encryption"
    );
    assert!(
        field.source_set_complete(),
        "leaf and parent archive facts are known"
    );
    let matching = origins
        .provenance
        .get("uses")
        .and_then(Value::as_array)
        .ok_or("source uses")?
        .iter()
        .find(|item| {
            item.get("id").and_then(Value::as_str).is_some_and(|id| {
                field
                    .source_uses_refs()
                    .iter()
                    .any(|reference| reference.as_str() == id)
            })
        })
        .ok_or("authored use reference")?;
    assert_eq!(
        matching
            .pointer("/location/namespace")
            .and_then(Value::as_str),
        Some("materialXmlDeclaration"),
        "authored declaration and parsed attribute coordinates are distinct"
    );
    assert_eq!(
        matching.pointer("/location/indices"),
        Some(&serde_json::json!(["0", "0"])),
        "original unknown child and attribute positions"
    );
    let sources = origins
        .provenance
        .get("sources")
        .and_then(Value::as_array)
        .ok_or("physical sources")?;
    assert_eq!(
        sources.len(),
        2,
        "physical leaf and encrypted ancestor are retained"
    );
    Ok(())
}
