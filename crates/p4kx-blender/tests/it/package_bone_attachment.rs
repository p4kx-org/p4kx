// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! A bone attachment of a character definition is an object node below its part.

use p4kx_assembly::{ArchiveModels, Assembly, AssemblyOptions, ForgePalettes, LoadoutResolver};
use p4kx_blender::{Manifest, NodeKind, PackageInfo, write_package};
use p4kx_extraction::{Archive, DataForgeEntry};
use p4kx_testkit::{
    BONE_THRUSTER, DataForgeBuilder, TestEntity, TestEntry, TestModel, TestMtl, TestMtlMaterial,
    TestSkeleton, bone_thruster_definition, bone_thruster_entries, bone_thruster_housing,
    entity_schema, tint_palette_record,
};

use crate::support::{TestResult, assemble, fixture, glb_options, never};

#[test]
fn an_attachment_is_an_object_node_with_a_mesh_below_its_part() -> TestResult {
    let definition =
        bone_thruster_definition(&bone_thruster_housing("tip", r"Objects\Test\housing.cgf"));
    let fixture = fixture(bone_thruster_entries(&definition))?;
    let archive = Archive::open(&fixture.path)?;
    let assembly = assemble(&archive, BONE_THRUSTER)?;
    let models = ArchiveModels::new(&archive);
    let dir = tempfile::tempdir()?;
    let out = dir.path().join("thruster");

    write_package(
        &assembly,
        &models,
        &models.textures(),
        &glb_options(0),
        &PackageInfo::new(None, None),
        &out,
        &never,
    )?;

    let manifest: Manifest =
        serde_json::from_str(&std::fs::read_to_string(out.join("scene.json"))?)?;
    let housing = manifest
        .nodes
        .iter()
        .find(|node| node.name == "Housing")
        .ok_or("the attachment node")?;
    assert_eq!(housing.kind, NodeKind::Object);
    assert_eq!(housing.path, "Housing");
    assert_eq!(housing.parent, Some(0));
    assert!(housing.entity.is_none(), "no Entity identity");
    assert!(housing.mesh.is_some(), "the housing is drawn");
    Ok(())
}

#[test]
fn a_rig_owner_has_no_mesh_asset_and_the_rigid_mesh_keeps_its_material_and_palette() -> TestResult {
    let housing = bone_thruster_housing("tip", r"Objects\Test\housing.cgf");
    let definition = format!(
        r#"<CharacterDefinition><Model File="Objects\Test\rig.cga"/>
        <AttachmentList>{housing}</AttachmentList></CharacterDefinition>"#
    );
    let fixture = fixture(rig_only_entries(&definition)?)?;
    let archive = Archive::open(&fixture.path)?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let mut resolver = LoadoutResolver::new(&forge).with_forge_entry(&entry);
    let record = resolver.entities().find(BONE_THRUSTER)?;
    let tree = resolver.resolve(&record)?;
    let models = ArchiveModels::new(&archive).with_model_node_sources();
    let palettes = ForgePalettes::new(&forge).with_entry(&entry);
    let assembly =
        Assembly::build_with_palettes(&tree, &models, &palettes, &AssemblyOptions::default())?;
    let root = assembly.root().ok_or("the rig owner")?;
    let [rigid] = root.attachments() else {
        return Err("one placed housing".into());
    };
    assert!(
        rigid.material_selector().is_some() && !rigid.uses_attachment_material(),
        "the explicit Entity material replaces the CDF material"
    );
    assert_eq!(
        rigid
            .material_archive_origins()
            .library()
            .map(|origin| origin.entry().path()),
        Some(r"Data\Objects\Test\override.mtl"),
        "the effective material comes from the overriding Archive Entry"
    );

    let scratch = tempfile::tempdir()?;
    let out = scratch.path().join("rig-only");
    write_package(
        &assembly,
        &models,
        &models.textures(),
        &glb_options(0),
        &PackageInfo::default(),
        &out,
        &never,
    )?;
    let manifest: Manifest = serde_json::from_slice(&std::fs::read(out.join("scene.json"))?)?;
    verify_rig_only_package(&manifest)
}

fn verify_rig_only_package(manifest: &Manifest) -> TestResult {
    let [owner, housing] = manifest.nodes.as_slice() else {
        return Err("the owner and one object child".into());
    };
    assert!(
        owner.mesh.is_none(),
        "the rig owner has no drawable GLB asset"
    );
    assert_eq!(
        housing.parent,
        Some(0),
        "the housing belongs to the rig owner"
    );
    let [mesh] = manifest.meshes.as_slice() else {
        return Err("one drawable package mesh".into());
    };
    assert_eq!(
        housing.mesh.as_deref(),
        Some(mesh.id.as_str()),
        "the child uses the sole drawn mesh"
    );
    assert!(mesh.triangles > 0, "the child GLB has drawable triangles");
    assert!(
        mesh.palette_id.is_some(),
        "the mesh retains the owner's named palette"
    );
    let bindings = manifest
        .palette_provenance
        .as_ref()
        .and_then(|value| value.get("bindings"))
        .and_then(serde_json::Value::as_array)
        .ok_or("the palette provenance bindings")?;
    let [binding] = bindings.as_slice() else {
        return Err("only the rendered housing has a palette binding".into());
    };
    assert_eq!(
        binding.get("node").and_then(serde_json::Value::as_u64),
        Some(1),
        "the drawn child owns the effective palette binding"
    );
    assert!(
        binding.get("inheritedFrom").is_none(),
        "the meshless owner has no drawable palette binding"
    );
    Ok(())
}

fn rig_only_entries(definition: &str) -> Result<Vec<TestEntry>, Box<dyn std::error::Error>> {
    const PALETTE: u128 = 0x31;
    let entity = TestEntity::new(1, BONE_THRUSTER, "entities/test_thruster.xml")
        .geometry("Objects/Test/thruster.cdf", Some("Objects/Test/override"))
        .palette(PALETTE, "");
    let palette = tint_palette_record(PALETTE, "p_rig", [[255, 0, 0]; 3], [255; 3], [51, 102, 153]);
    let database = entity_schema(DataForgeBuilder::new())
        .record(entity.record())
        .record(palette);
    let mut entries = bone_thruster_entries(definition);
    let entry = entries
        .iter_mut()
        .find(|entry| entry.path() == r"Data\Game2.dcb")
        .ok_or("the fixture DataForge file")?;
    *entry = TestEntry::zstd(r"Data\Game2.dcb", database.build());
    let rig = TestModel::cube()
        .helpers_only()
        .with_compact_skeleton(&TestSkeleton::two_bones());
    let material = TestMtlMaterial::new("overriding").diffuse([0.5; 3]);
    entries.extend([
        TestEntry::stored(r"Data\Objects\Test\rig.cga", rig.cgf()),
        TestEntry::stored(
            r"Data\Objects\Test\override.mtl",
            TestMtl::multi(&[material.clone(), material.clone(), material]).cryxml(),
        ),
    ]);
    Ok(entries)
}
