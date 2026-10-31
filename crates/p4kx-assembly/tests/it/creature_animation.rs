// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! A creature Entity that names an animation database writes its clips as animations of the
//! bones of its skeleton.

use crate::assembly_support::{Fixture, TestResult, fixture};

use p4kx_assembly::{
    ArchiveModels, Assembly, AssemblyOptions, CreatureClips, IssueKind, LoadoutResolver,
    creature_animations,
};
use p4kx_extraction::{Archive, DataForgeEntry};
use p4kx_gltf::GlbOptions;
use p4kx_testkit::{
    CREATURE, CREATURE_ANIMATED, CREATURE_ANIMATION_DATABASE_LOST, CREATURE_ANIMATION_NESTED_LOST,
    CREATURE_ANIMATION_PARAMS_LOST, check_glb, creature_entries,
};

/// What exporting a creature with its clips came to.
struct Exported {
    clips: CreatureClips,
    left_out: Vec<String>,
    targets: Vec<String>,
    written: usize,
}

fn open() -> Result<(Fixture, Archive), Box<dyn std::error::Error>> {
    let fixture = fixture(creature_entries())?;
    let archive = Archive::open(&fixture.path)?;
    Ok((fixture, archive))
}

/// Builds `class`, reads its clips and writes the GLB with them.
fn export(class: &str) -> Result<Exported, Box<dyn std::error::Error>> {
    let (_fixture, archive) = open()?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let mut resolver = LoadoutResolver::new(&forge);
    let record = resolver.entities().find(class)?;
    let tree = resolver.resolve(&record)?;
    let models = ArchiveModels::new(&archive);
    let assembly = Assembly::build(&tree, &models, &AssemblyOptions::default())?;
    let clips = creature_animations(&models, &assembly, &tree)?;
    let fitted = assembly.animate(&clips.set.animations);
    let left_out = fitted
        .issues()
        .iter()
        .map(|issue| issue.kind().to_string())
        .collect();
    let mut bytes = Vec::new();
    assembly.write_glb_animated(&fitted, &GlbOptions::default(), &mut bytes)?;
    let glb = check_glb(&bytes)?;
    let targets = glb
        .document
        .animations()
        .flat_map(|animation| animation.channels().collect::<Vec<_>>())
        .filter_map(|channel| channel.target().node().name().map(str::to_owned))
        .collect();
    let written = glb.document.animations().len();
    Ok(Exported {
        clips,
        left_out,
        targets,
        written,
    })
}

fn names(clips: &CreatureClips) -> Vec<&str> {
    let mut names: Vec<&str> = clips
        .set
        .animations
        .iter()
        .map(|animation| animation.name.as_str())
        .collect();
    names.sort_unstable();
    names
}

#[test]
fn the_clips_the_database_names_are_found_and_the_blend_space_without_a_file_is_skipped()
-> TestResult {
    let exported = export(CREATURE_ANIMATED)?;

    assert_eq!(names(&exported.clips), ["lean", "stray", "wave"]);
    assert!(
        exported.clips.issues.is_empty(),
        "{:?}",
        exported.clips.issues
    );
    Ok(())
}

#[test]
fn a_clip_that_moves_a_bone_is_written_on_the_node_of_that_bone() -> TestResult {
    let exported = export(CREATURE_ANIMATED)?;

    assert_eq!(exported.written, 1);
    assert_eq!(exported.targets, ["tip"]);
    Ok(())
}

#[test]
fn additive_clips_and_clips_without_a_bone_are_left_out_and_listed_one_per_reason() -> TestResult {
    let exported = export(CREATURE_ANIMATED)?;

    let left_out = exported.left_out;
    assert_eq!(left_out.len(), 2, "{left_out:?}");
    assert!(
        left_out.iter().any(|text| text.contains("additive")),
        "{left_out:?}"
    );
    assert!(
        left_out
            .iter()
            .any(|text| text.contains("no track names a bone")),
        "{left_out:?}"
    );
    Ok(())
}

#[test]
fn a_database_the_archive_lacks_is_listed_and_the_creature_stays_static() -> TestResult {
    let exported = export(CREATURE_ANIMATION_DATABASE_LOST)?;

    assert_eq!(names(&exported.clips), Vec::<&str>::new());
    assert_eq!(exported.written, 0);
    let [issue] = exported.clips.issues.as_slice() else {
        return Err(format!("one issue, got {:?}", exported.clips.issues).into());
    };
    assert_eq!(issue.kind().code(), "animations_unavailable");
    assert!(
        matches!(issue.kind(), IssueKind::AnimationsUnavailable { reason } if reason.contains("absent.adb")),
        "{issue:?}"
    );
    assert!(!issue.kind().skips());
    Ok(())
}

#[test]
fn a_skeleton_without_a_parameter_file_is_listed() -> TestResult {
    let exported = export(CREATURE_ANIMATION_PARAMS_LOST)?;

    assert_eq!(names(&exported.clips), Vec::<&str>::new());
    assert_eq!(exported.written, 0);
    let [issue] = exported.clips.issues.as_slice() else {
        return Err(format!("one issue, got {:?}", exported.clips.issues).into());
    };
    assert!(
        matches!(issue.kind(), IssueKind::AnimationsUnavailable { reason } if reason.contains("short_skel.chrparams")),
        "{issue:?}"
    );
    Ok(())
}

#[test]
fn a_creature_that_names_no_database_reads_nothing_and_lists_nothing() -> TestResult {
    let exported = export(CREATURE)?;

    assert_eq!(names(&exported.clips), Vec::<&str>::new());
    assert_eq!(exported.clips.issues, []);
    assert_eq!(exported.left_out, Vec::<String>::new());
    assert_eq!(exported.written, 0);
    Ok(())
}

#[test]
fn clips_beyond_the_budget_are_left_out_with_a_limit_reason() -> TestResult {
    let (_fixture, archive) = open()?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let mut resolver = LoadoutResolver::new(&forge);
    let record = resolver.entities().find(CREATURE_ANIMATED)?;
    let tree = resolver.resolve(&record)?;
    let models = ArchiveModels::new(&archive);
    let assembly = Assembly::build(&tree, &models, &AssemblyOptions::default())?;
    let clips = creature_animations(&models, &assembly, &tree)?;

    let fitted = assembly.animate_within(&clips.set.animations, 0);

    assert!(fitted.is_empty());
    assert!(
        fitted.issues().iter().any(|issue| matches!(
            issue.kind(),
            IssueKind::AnimationNotUsed { reason, .. } if reason.starts_with("limit")
        )),
        "{:?}",
        fitted.issues()
    );
    Ok(())
}

#[test]
fn a_nested_database_that_cannot_be_read_is_listed_and_the_rest_is_still_written() -> TestResult {
    let exported = export(CREATURE_ANIMATION_NESTED_LOST)?;

    assert_eq!(names(&exported.clips), ["wave"]);
    assert_eq!(exported.written, 1);
    let [issue] = exported.clips.issues.as_slice() else {
        return Err(format!("one issue, got {:?}", exported.clips.issues).into());
    };
    assert!(
        matches!(issue.kind(), IssueKind::AnimationsUnavailable { reason } if reason.contains("broken.adb")),
        "{issue:?}"
    );
    assert_eq!(
        issue.path(),
        "",
        "an issue of the root Entity has no port path"
    );
    Ok(())
}
