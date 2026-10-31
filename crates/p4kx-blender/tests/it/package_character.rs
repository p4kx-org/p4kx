// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! A Character written as a package: the skinned GLB with its skeleton, morph targets and
//! animations, and the manifest that lists them.

use std::error::Error;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use p4kx_assembly::character::{Dna, DnaBlend};
use p4kx_assembly::{ArchiveModels, Armour, AssemblyOptions, BodyVariant, Expressions, Piece};
use p4kx_blender::{
    CharacterScene, FORMAT_VERSION, Manifest, NodeKind, PackageError, PackageInfo, PackageSummary,
    RigPartOrigin, manifest_schema, write_character_package,
};
use p4kx_chunks::ChunkFile;
use p4kx_extraction::Archive;
use p4kx_gltf::GlbOptions;
use p4kx_model::Animation;
use p4kx_testkit::{
    HEAD_REGIONS, HEAD_VERTICES, MALE_HEAD_FOLDER, MALE_RIG, MALE_RIG_HASH, TestAnimation,
    TestSkeleton, TestTrack, check_glb, head_entries, head_rig,
};

use crate::support::{TestResult, fixture, names_in, never};

fn head_path() -> String {
    format!("{MALE_HEAD_FOLDER}/{MALE_RIG}_head.skin")
}

fn dna() -> Dna {
    let blends = |value, head_id| vec![DnaBlend { value, head_id }];
    Dna {
        gender_hash: MALE_RIG_HASH,
        label: p4kx_assembly::character::Label,
        variant_hash: 0,
        blends_per_part: 1,
        header_unknown: 0,
        max_head_id: 2,
        face_parts: (0..HEAD_REGIONS)
            .map(|part| {
                if part == 3 {
                    blends(65535, 2)
                } else {
                    blends(0, 0)
                }
            })
            .collect(),
    }
}

fn animation(source: &TestAnimation) -> Result<Animation, Box<dyn Error>> {
    let bytes = source.caf();
    Ok(Animation::read_caf(
        source.name(),
        &ChunkFile::parse(bytes.as_slice())?,
    )?)
}

fn face_animation() -> TestAnimation {
    TestAnimation::new("blink.caf", 30).track(TestTrack::new(crc32fast::hash(b"CTL_a")).positions(
        &[
            (0, [0.0, 0.0, 0.0]),
            (10, [1.0, 0.0, 0.0]),
            (20, [0.0, 0.0, 0.0]),
        ],
    ))
}

fn body_animation() -> TestAnimation {
    TestAnimation::new("nod.caf", 30).track(
        TestTrack::new(TestSkeleton::controller_id(2))
            .rotations(&[(0, [0.0, 0.0, 0.0, 1.0]), (10, [0.0, 0.0, 0.0, 1.0])]),
    )
}

struct Written {
    _fixture: crate::support::Fixture,
    _dir: tempfile::TempDir,
    out: PathBuf,
    manifest: Manifest,
    raw: serde_json::Value,
    summary: PackageSummary,
}

fn write(expressions: bool, animated: bool) -> Result<Written, Box<dyn Error>> {
    let fixture = fixture(head_entries(Some(
        head_rig(HEAD_VERTICES, MALE_RIG_HASH).build(),
    )))?;
    let archive = Archive::open(&fixture.path)?;
    let models = ArchiveModels::new(&archive);
    let piece = Piece::file("head", head_path(), None).shaped(dna());
    let piece = if expressions {
        piece.with_expressions(Expressions::All)
    } else {
        piece
    };
    let armour = Armour::build(
        BodyVariant::Male,
        &[piece],
        &models,
        &AssemblyOptions::default(),
    )?;
    let list = if animated {
        vec![animation(&face_animation())?, animation(&body_animation())?]
    } else {
        Vec::new()
    };
    let fitted = armour.animate(&list, &models);
    let dir = tempfile::tempdir()?;
    let out = dir.path().join("character");
    let info = PackageInfo::new(Some("LIVE".to_owned()), None);
    let scene = CharacterScene::new("male_v7", &armour, &fitted)
        .with_origin(Some("chars/hero.chf"), Some("Loadout.xml"));
    let summary = write_character_package(
        &scene,
        &models.textures(),
        &GlbOptions::default(),
        &info,
        &out,
        &never,
    )?;
    let text = std::fs::read_to_string(out.join("scene.json"))?;
    Ok(Written {
        manifest: serde_json::from_str(&text)?,
        raw: serde_json::from_str(&text)?,
        _fixture: fixture,
        _dir: dir,
        out,
        summary,
    })
}

fn mesh_file(out: &Path) -> PathBuf {
    out.join("meshes").join("m0001.glb")
}

#[test]
fn a_character_is_a_folder_of_a_manifest_and_one_skinned_mesh() -> TestResult {
    let written = write(true, true)?;

    assert_eq!(
        names_in(&written.out)?,
        ["meshes", "scene.json", "textures"]
    );
    assert_eq!(names_in(&written.out.join("meshes"))?, ["m0001.glb"]);
    assert_eq!(names_in(&written.out.join("textures"))?.len(), 0);
    assert_eq!(written.manifest.version, FORMAT_VERSION);
    assert_eq!(written.manifest.version, "1.6");
    let [node] = written.manifest.nodes.as_slice() else {
        return Err("one node".into());
    };
    assert_eq!(node.kind, NodeKind::Character);
    assert_eq!(node.name, "male_v7");
    assert_eq!(node.mesh.as_deref(), Some("m0001"));
    assert_eq!(node.rotation, [0.0, 0.0, 0.0, 1.0]);
    assert_eq!(
        written.raw["nodes"][0]["kind"], "character",
        "the kind is written in snake case"
    );
    assert_eq!(written.manifest.source.channel.as_deref(), Some("LIVE"));
    assert_eq!(written.manifest.source.entity.class_name, "male_v7");
    assert_eq!(written.manifest.source.entity.guid, "");
    let character = written
        .manifest
        .source
        .character
        .as_ref()
        .ok_or("character")?;
    assert_eq!(character.name, "male_v7");
    assert_eq!(character.body, "male");
    assert_eq!(character.chf.as_deref(), Some("hero.chf"), "no folder");
    assert_eq!(character.loadout.as_deref(), Some("Loadout.xml"));
    Ok(())
}

#[test]
fn the_manifest_of_a_character_matches_its_schema() -> TestResult {
    let written = write(true, true)?;
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
fn the_rig_lists_the_parts_morph_targets_and_animations_the_glb_holds() -> TestResult {
    let written = write(true, true)?;
    let glb = check_glb(&std::fs::read(mesh_file(&written.out))?)?;

    let mesh = written.manifest.meshes.first().ok_or("mesh")?;
    let rig = mesh.rig.as_ref().ok_or("rig")?;
    assert_eq!(rig.body, "male");
    assert_eq!(
        rig.bones,
        glb.document.skins().next().ok_or("skin")?.joints().count()
    );
    assert!(rig.skinned_meshes >= 2, "body and head are skinned");
    assert_eq!(mesh.materials.len(), 0);
    assert_eq!(
        mesh.bytes,
        std::fs::metadata(mesh_file(&written.out))?.len()
    );

    let head = rig
        .parts
        .iter()
        .find(|part| part.name == "head")
        .ok_or("head part")?;
    assert_eq!(head.origin, RigPartOrigin::Piece);
    assert!(head.morph_targets > 0, "the expressions are morph targets");
    assert_eq!(
        rig.morph_targets,
        rig.parts
            .iter()
            .map(|part| part.morph_targets)
            .sum::<usize>()
    );
    assert_eq!(
        rig.parts.first().map(|part| part.origin),
        Some(RigPartOrigin::Body)
    );

    let in_glb: Vec<&str> = glb
        .document
        .animations()
        .filter_map(|animation| animation.name())
        .collect();
    let listed: Vec<&str> = rig
        .animations
        .iter()
        .map(|clip| clip.name.as_str())
        .collect();
    assert_eq!(listed.len(), 2);
    assert_eq!(in_glb.len(), 2);
    for name in &listed {
        assert!(in_glb.contains(name), "{name} is in the file: {in_glb:?}");
    }
    assert!(
        rig.animations
            .iter()
            .all(|clip| clip.channels > 0 && clip.keys > 0)
    );
    assert_eq!(written.summary.animations(), 2);
    assert_eq!(written.summary.bones, rig.bones);
    assert_eq!(written.summary.morph_targets, rig.morph_targets);
    assert_eq!(written.summary.files, 2);
    Ok(())
}

#[test]
fn a_character_without_animations_or_expressions_has_neither_in_its_rig() -> TestResult {
    let written = write(false, false)?;

    let rig = written
        .manifest
        .meshes
        .first()
        .and_then(|mesh| mesh.rig.as_ref())
        .ok_or("rig")?;
    assert_eq!(rig.animations.len(), 0);
    assert_eq!(rig.morph_targets, 0);
    assert!(
        written.raw["meshes"][0]["rig"]["animations"]
            .as_array()
            .is_some_and(Vec::is_empty)
    );
    Ok(())
}

#[test]
fn an_animation_that_fits_nothing_is_an_issue_not_a_clip() -> TestResult {
    let fixture = fixture(head_entries(Some(
        head_rig(HEAD_VERTICES, MALE_RIG_HASH).build(),
    )))?;
    let archive = Archive::open(&fixture.path)?;
    let models = ArchiveModels::new(&archive);
    let armour = Armour::build(
        BodyVariant::Male,
        &[Piece::file("head", head_path(), None).shaped(dna())],
        &models,
        &AssemblyOptions::default(),
    )?;
    let stranger = TestAnimation::new("stranger.caf", 30)
        .track(TestTrack::new(12345).positions(&[(0, [0.0; 3]), (5, [1.0, 0.0, 0.0])]));
    let list = [animation(&stranger)?];
    let fitted = armour.animate(&list, &models);
    let dir = tempfile::tempdir()?;

    let summary = write_character_package(
        &CharacterScene::new("male_v7", &armour, &fitted),
        &models.textures(),
        &GlbOptions::default(),
        &PackageInfo::default(),
        &dir.path().join("character"),
        &never,
    )?;

    assert_eq!(summary.animations(), 0);
    let kinds: Vec<&str> = summary
        .issues
        .iter()
        .map(|issue| issue.kind.as_str())
        .collect();
    assert_eq!(kinds, ["animation_not_used"]);
    Ok(())
}

#[test]
fn a_cancelled_run_leaves_no_package_and_no_temporary_folder() -> TestResult {
    let fixture = fixture(head_entries(Some(
        head_rig(HEAD_VERTICES, MALE_RIG_HASH).build(),
    )))?;
    let archive = Archive::open(&fixture.path)?;
    let models = ArchiveModels::new(&archive);
    let armour = Armour::build(BodyVariant::Male, &[], &models, &AssemblyOptions::default())?;
    let fitted = armour.animate(&[], &models);
    let dir = tempfile::tempdir()?;
    let out = dir.path().join("character");

    let result = write_character_package(
        &CharacterScene::new("male_v7", &armour, &fitted),
        &models.textures(),
        &GlbOptions::default(),
        &PackageInfo::default(),
        &out,
        &|| true,
    );

    assert!(matches!(result, Err(PackageError::Cancelled)), "{result:?}");
    assert_eq!(names_in(dir.path())?.len(), 0);
    Ok(())
}

#[test]
fn a_run_cancelled_while_the_file_is_written_is_cancelled_and_leaves_nothing() -> TestResult {
    let fixture = fixture(head_entries(Some(
        head_rig(HEAD_VERTICES, MALE_RIG_HASH).build(),
    )))?;
    let archive = Archive::open(&fixture.path)?;
    let models = ArchiveModels::new(&archive);
    let armour = Armour::build(BodyVariant::Male, &[], &models, &AssemblyOptions::default())?;
    let fitted = armour.animate(&[], &models);
    let dir = tempfile::tempdir()?;
    let polls = AtomicUsize::new(0);

    let result = write_character_package(
        &CharacterScene::new("male_v7", &armour, &fitted),
        &models.textures(),
        &GlbOptions::default(),
        &PackageInfo::default(),
        &dir.path().join("character"),
        &|| polls.fetch_add(1, Ordering::SeqCst) >= 2,
    );

    assert!(matches!(result, Err(PackageError::Cancelled)), "{result:?}");
    assert!(polls.load(Ordering::SeqCst) >= 3, "the writer polled");
    assert_eq!(names_in(dir.path())?.len(), 0);
    Ok(())
}

#[test]
fn an_output_that_is_not_a_package_is_refused_and_left_alone() -> TestResult {
    let fixture = fixture(head_entries(None))?;
    let archive = Archive::open(&fixture.path)?;
    let models = ArchiveModels::new(&archive);
    let armour = Armour::build(BodyVariant::Male, &[], &models, &AssemblyOptions::default())?;
    let fitted = armour.animate(&[], &models);
    let dir = tempfile::tempdir()?;
    std::fs::write(dir.path().join("mine.txt"), b"mine")?;

    let result = write_character_package(
        &CharacterScene::new("male_v7", &armour, &fitted),
        &models.textures(),
        &GlbOptions::default(),
        &PackageInfo::default(),
        dir.path(),
        &never,
    );

    assert!(
        matches!(result, Err(PackageError::NotAPackage { .. })),
        "{result:?}"
    );
    assert_eq!(names_in(dir.path())?, ["mine.txt"]);
    Ok(())
}
