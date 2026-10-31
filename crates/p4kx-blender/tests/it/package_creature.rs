// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! A creature with its skeleton is written as one skinned rig mesh; one without stays static.

use std::path::Path;

use p4kx_assembly::{ArchiveModels, Assembly};
use p4kx_blender::{
    FORMAT_VERSION, IssueOrigin, Manifest, NodeKind, PackageInfo, PackageSummary, RigPartOrigin,
    manifest_schema, write_package,
};
use p4kx_extraction::Archive;
use p4kx_testkit::{
    CREATURE, CREATURE_COLLARED, CREATURE_DEFINITION, CREATURE_HORNED, CREATURE_LOADOUT_LOST,
    CREATURE_NO_SKELETON, CheckedGlb, TestEntry, TestMtl, TestMtlMaterial, check_glb,
    creature_entries,
};

use crate::support::{
    TestResult, assemble, assemble_with_interior, fixture, glb_options, names_in, never,
};

struct Written {
    _dir: tempfile::TempDir,
    out: std::path::PathBuf,
    assembly: Assembly,
    summary: PackageSummary,
    manifest: Manifest,
    raw: serde_json::Value,
}

fn write(class: &str) -> Result<Written, Box<dyn std::error::Error>> {
    write_from(creature_entries(), class, false)
}

fn write_from(
    entries: Vec<TestEntry>,
    class: &str,
    interior: bool,
) -> Result<Written, Box<dyn std::error::Error>> {
    let fixture = fixture(entries)?;
    let archive = Archive::open(&fixture.path)?;
    let assembly = if interior {
        assemble_with_interior(&archive, class)?
    } else {
        assemble(&archive, class)?
    };
    let models = ArchiveModels::new(&archive);
    let dir = tempfile::tempdir()?;
    let out = dir.path().join("creature");
    let summary = write_package(
        &assembly,
        &models,
        &models.textures(),
        &glb_options(0),
        &PackageInfo::new(None, None),
        &out,
        &never,
    )?;
    let text = std::fs::read_to_string(out.join("scene.json"))?;
    Ok(Written {
        _dir: dir,
        out,
        assembly,
        summary,
        manifest: serde_json::from_str(&text)?,
        raw: serde_json::from_str(&text)?,
    })
}

fn glb_of(folder: &Path, file: &str) -> Result<CheckedGlb, Box<dyn std::error::Error>> {
    Ok(check_glb(&std::fs::read(folder.join(file))?)?)
}

#[test]
fn a_creature_with_its_skeleton_is_one_skinned_rig_mesh() -> TestResult {
    let written = write(CREATURE)?;
    let manifest = &written.manifest;

    assert_eq!(manifest.version, FORMAT_VERSION);
    assert_eq!(names_in(&written.out.join("meshes"))?, ["m0001.glb"]);
    assert!(manifest.textures.is_empty() && manifest.materials.is_empty());
    let [node] = manifest.nodes.as_slice() else {
        return Err("one node".into());
    };
    assert_eq!(node.kind, NodeKind::Entity);
    assert_eq!(node.mesh.as_deref(), Some("m0001"));
    assert_eq!(
        node.entity
            .as_ref()
            .map(|entity| entity.class_name.as_str()),
        Some(CREATURE)
    );
    let [mesh] = manifest.meshes.as_slice() else {
        return Err("one mesh".into());
    };
    let rig = mesh.rig.as_ref().ok_or("the mesh has a rig")?;
    assert_eq!(rig.body, None, "a creature has no body variant");
    assert_eq!((rig.bones, rig.skinned_meshes), (3, 1));
    assert_eq!(rig.animations, []);
    let skin = written
        .assembly
        .part_at("Body_ItemPort")
        .ok_or("the skin")?;
    let [part] = rig.parts.as_slice() else {
        return Err("one skinned part".into());
    };
    assert_eq!(part.name, skin.name());
    assert_eq!(part.origin, RigPartOrigin::Skin);
    assert_eq!(Some(part.file.as_str()), skin.geometry());
    assert_eq!(
        Some(rig.skeleton.as_str()),
        written.assembly.root().and_then(|root| root.geometry())
    );

    let glb = glb_of(&written.out, &mesh.file)?;
    assert_eq!(glb.document.skins().len(), 1);
    assert_eq!(written.summary.bones, 3);
    assert_eq!(written.summary.files, 2);
    Ok(())
}

#[test]
fn a_rigid_part_of_a_creature_is_in_the_rig_file_and_not_a_skinned_part() -> TestResult {
    let written = write(CREATURE_HORNED)?;

    let rig = written.manifest.meshes[0].rig.as_ref().ok_or("a rig")?;
    assert_eq!(rig.parts.len(), 1, "only the body is skinned");
    assert_eq!(written.manifest.nodes.len(), 1);
    let glb = glb_of(&written.out, "meshes/m0001.glb")?;
    let horn = glb
        .document
        .nodes()
        .any(|node| node.name().is_some_and(|name| name.contains("Horn")));
    assert!(horn, "the horn hangs on its bone in the same file");
    Ok(())
}

#[test]
fn the_skin_of_a_character_definition_is_a_skinned_part_too() -> TestResult {
    let written = write(CREATURE_DEFINITION)?;

    let rig = written.manifest.meshes[0].rig.as_ref().ok_or("a rig")?;
    assert_eq!(rig.skinned_meshes, 2);
    assert_eq!(rig.parts.len(), 2, "the root's own skin and the port's");
    Ok(())
}

#[test]
fn a_creature_without_its_skeleton_stays_a_set_of_static_meshes() -> TestResult {
    let written = write(CREATURE_NO_SKELETON)?;

    assert!(
        written
            .manifest
            .meshes
            .iter()
            .all(|mesh| mesh.rig.is_none())
    );
    assert!(
        !written.manifest.meshes.is_empty(),
        "the skin is still drawn"
    );
    assert_eq!(written.summary.bones, 0);
    let glb = glb_of(&written.out, &written.manifest.meshes[0].file)?;
    assert_eq!(glb.document.skins().len(), 0);
    Ok(())
}

#[test]
fn a_creature_with_a_prop_on_a_skin_port_keeps_it_in_the_rig_file() -> TestResult {
    let written = write(CREATURE_COLLARED)?;

    assert_eq!(written.manifest.meshes.len(), 1);
    assert!(written.manifest.meshes[0].rig.is_some());
    Ok(())
}

#[test]
fn the_scene_json_of_a_creature_has_no_body_and_matches_the_schema() -> TestResult {
    let written = write(CREATURE)?;

    let rig = &written.raw["meshes"][0]["rig"];
    assert!(rig.get("body").is_none(), "{rig}");
    assert_eq!(rig["parts"][0]["origin"], "skin");
    let validator = jsonschema::draft202012::new(&manifest_schema())?;
    let violations: Vec<String> = validator
        .iter_errors(&written.raw)
        .take(5)
        .map(|error| format!("{} at {}", error, error.instance_path()))
        .collect();
    assert_eq!(violations.len(), 0, "{violations:?}");
    Ok(())
}

#[test]
fn a_creature_with_bones_and_no_skin_stays_static() -> TestResult {
    let written = write(CREATURE_LOADOUT_LOST)?;

    assert_eq!(
        written.assembly.creature_skins().map(Iterator::count),
        Some(0),
        "the skeleton is there and no part is skinned"
    );
    assert!(
        written
            .manifest
            .meshes
            .iter()
            .all(|mesh| mesh.rig.is_none())
    );
    assert_eq!(written.summary.bones, 0, "the writer has no bones to bind");
    Ok(())
}

#[test]
fn the_interior_of_a_creature_is_left_out_and_listed() -> TestResult {
    let written = write_from(creature_entries(), CREATURE, true)?;

    assert!(written.assembly.interior().is_some());
    let left_out: Vec<_> = written
        .manifest
        .issues
        .iter()
        .filter(|issue| issue.kind == "interior_not_written")
        .collect();
    assert_eq!(left_out.len(), 1, "{:?}", written.manifest.issues);
    assert_eq!(left_out[0].origin, IssueOrigin::Assembly);
    assert_eq!(written.manifest.nodes.len(), 1);
    let plain = write(CREATURE)?;
    assert!(
        plain
            .manifest
            .issues
            .iter()
            .all(|issue| issue.kind != "interior_not_written")
    );
    Ok(())
}

#[test]
fn a_texture_the_rig_file_cannot_embed_is_an_issue_of_the_manifest() -> TestResult {
    let library = TestMtl::multi(&[
        TestMtlMaterial::new("unused").texture("TexSlot1", "Objects/Test/missing_diff.tif"),
        TestMtlMaterial::new("caps").texture("TexSlot1", "Objects/Test/missing_diff.tif"),
        TestMtlMaterial::new("sides").texture("TexSlot1", "Objects/Test/missing_diff.tif"),
    ]);
    let mut entries = creature_entries();
    entries.push(TestEntry::stored(
        r"Data\objects\test\cube.mtl",
        library.cryxml(),
    ));

    let written = write_from(entries, CREATURE, false)?;

    let found = written
        .manifest
        .issues
        .iter()
        .any(|issue| issue.origin == IssueOrigin::Texture && issue.kind == "texture");
    assert!(found, "{:?}", written.manifest.issues);
    Ok(())
}
