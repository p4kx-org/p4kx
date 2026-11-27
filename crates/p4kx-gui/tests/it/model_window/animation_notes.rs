// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The animation count of the Info tab beyond the preview's bound, models with nothing to
//! animate, and the animations the export summary reports as not written.

use super::animations::site;
use crate::glb_exporting::finished;
use crate::viewing::{Site, TestResult, model_preview, opened_at, select};
use crate::waiting::{RunQuiet, wait_for_model};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use p4kx_gui::{
    AnimationCount, AnimationNoteKind, Language, MAX_SKELETON_BONES, Model, ModelExport,
    ModelExportStatus, ModelExportSummary, PREVIEW_ANIMATION_BOUNDS,
};
use p4kx_testkit::{TestAnimation, TestEntry, TestModel, TestPositions, TestSkeleton, TestTrack};

pub(super) fn crowded() -> Site {
    let mut site = site();
    site.entries.extend(
        (0..=PREVIEW_ANIMATION_BOUNDS.files).map(|number| {
            TestEntry::stored(format!("Data\\Objects\\extra{number}.caf"), vec![0; 4])
        }),
    );
    site
}

fn stray() -> TestAnimation {
    TestAnimation::new("stray.caf", 30).track(
        TestTrack::new(TestSkeleton::controller_id(7))
            .rotations(&[(0, [0.0, 0.0, 0.0, 1.0]), (30, [0.0, 0.0, 0.0, 1.0])]),
    )
}

pub(super) fn with_stray() -> Site {
    let mut site = site();
    site.entries
        .push(TestEntry::zstd("Data\\Objects\\stray.caf", stray().caf()));
    site
}

fn rejected_skin() -> Site {
    let cube = TestModel::cube()
        .with_positions(TestPositions::Float)
        .with_nodes(Vec::new())
        .with_skeleton(&TestSkeleton::chain(
            u16::try_from(MAX_SKELETON_BONES + 1).unwrap_or(u16::MAX),
        ));
    let weights = (0..24).map(|_| vec![(1, 255)]).collect();
    let arm = cube.with_bone_map(false, weights);
    let swing = TestAnimation::new("swing.caf", 30).track(
        TestTrack::new(TestSkeleton::controller_id(1))
            .rotations(&[(0, [0.0, 0.0, 0.0, 1.0]), (30, [0.0, 0.0, 0.0, 1.0])]),
    );
    Site {
        entries: vec![
            TestEntry::zstd("Data\\Objects\\arm.skin", arm.cgf()),
            TestEntry::zstd("Data\\Objects\\arm.skinm", arm.cgfm()),
            TestEntry::zstd("Data\\Objects\\swing.caf", swing.caf()),
        ],
        installation: false,
        key: false,
    }
}

#[test]
fn a_skinned_model_the_viewer_rejects_still_counts_and_offers_its_animations() -> TestResult {
    let (_folder, mut harness) = opened_at(
        Language::BritishEnglish,
        crate::headless::ready(),
        rejected_skin(),
        |b| b,
    )?;

    select(&mut harness, "arm.skin")?;

    let preview = model_preview(harness.state()).ok_or("no preview")?;
    assert!(
        preview.playback().is_none(),
        "the viewer rejects the skeleton"
    );
    assert_eq!(
        preview.info().map(|info| info.animations),
        Some(AnimationCount::Counted(1)),
        "but its animation is counted"
    );
    harness.state_mut().begin_model_export();
    let setup = harness
        .state_mut()
        .model_export_setup_mut()
        .ok_or("no setup")?;
    assert!(setup.animations_offered(), "and offered");
    Ok(())
}

fn summary(model: &Model) -> Result<ModelExportSummary, String> {
    match model.model_export().map(ModelExport::status) {
        Some(ModelExportStatus::Finished(summary)) => Ok(summary.clone()),
        other => Err(format!("no summary: {other:?}")),
    }
}

fn exported(
    language: Language,
    site: Site,
) -> Result<(tempfile::TempDir, Harness<'static, Model>), Box<dyn std::error::Error>> {
    let (folder, mut harness) = opened_at(language, crate::headless::ready(), site, |b| b)?;
    select(&mut harness, "door.cga")?;
    harness.state_mut().begin_model_export();
    let setup = harness
        .state_mut()
        .model_export_setup_mut()
        .ok_or("no setup")?;
    setup.set_animations(true);
    setup.set_file(folder.path().join("door.glb"));
    assert!(harness.state_mut().submit_model_export(|| {}), "started");
    finished(&mut harness)?;
    Ok((folder, harness))
}

#[test]
fn more_animation_files_than_the_preview_reads_show_not_read_and_stay_exportable() -> TestResult {
    let (folder, mut harness) = opened_at(
        Language::BritishEnglish,
        crate::headless::ready(),
        crowded(),
        |b| b,
    )?;

    select(&mut harness, "door.cga")?;

    let preview = model_preview(harness.state()).ok_or("no preview")?;
    let info = preview.info().ok_or("no info")?;
    assert_eq!(info.animations, AnimationCount::TooMany, "not counted");
    assert!(
        preview.animation_count_provenance().is_none(),
        "a refused read has no contributing inputs"
    );
    assert!(
        preview
            .playback()
            .is_some_and(|playback| playback.animations().is_empty()),
        "nothing to play"
    );
    harness.get_by_label("Info").click();
    harness.run_quiet();
    assert!(
        harness.query_by_label("Not read").is_some(),
        "the Info tab says so"
    );
    harness.state_mut().begin_model_export();
    let setup = harness
        .state_mut()
        .model_export_setup_mut()
        .ok_or("no setup")?;
    assert!(setup.animations_offered(), "still offered");
    assert_eq!(setup.animation_count(), 0, "without a count");
    setup.set_animations(true);
    setup.set_file(folder.path().join("door.glb"));
    assert!(harness.state_mut().submit_model_export(|| {}), "started");
    finished(&mut harness)?;

    let done = summary(harness.state())?;
    assert!(done.animations_asked(), "asked");
    assert_eq!(done.animations_written(), 3, "the readable ones");
    Ok(())
}

#[test]
fn a_model_with_nothing_to_animate_reads_no_animations() -> TestResult {
    let mut beside = site();
    let cube = TestModel::cube();
    beside.entries.extend([
        TestEntry::zstd("Data\\Objects\\cube.cgf", cube.cgf()),
        TestEntry::zstd("Data\\Objects\\cube.cgfm", cube.cgfm()),
    ]);
    let (_folder, mut harness) = opened_at(
        Language::BritishEnglish,
        crate::headless::ready(),
        beside,
        |b| b,
    )?;

    harness.get_by_label("cube.cgf").click();
    wait_for_model(&mut harness, "the model to be read", |model| {
        model_preview(model).is_some_and(|preview| !preview.is_loading())
    })?;

    let preview = model_preview(harness.state()).ok_or("no preview")?;
    assert!(!preview.is_reading_animations(), "nothing is read");
    assert!(
        preview.animation_count_provenance().is_none(),
        "a static model has no animation count inputs"
    );
    assert_eq!(
        preview.info().map(|info| info.animations),
        Some(AnimationCount::Counted(0)),
        "although animation files lie next to it"
    );
    Ok(())
}

#[test]
fn an_animation_no_bone_matches_is_reported_as_not_written() -> TestResult {
    let (_folder, harness) = exported(Language::BritishEnglish, with_stray())?;

    let done = summary(harness.state())?;
    assert_eq!(done.animations_written(), 3, "the stray one has no channel");
    assert_eq!(done.unread_animations_total(), 2, "additive and stray");
    let stray: Vec<_> = done
        .unread_animations()
        .iter()
        .filter(|note| matches!(note.kind, AnimationNoteKind::NoBoneMatched { .. }))
        .collect();
    let [note] = stray.as_slice() else {
        return Err(format!("notes: {:?}", done.unread_animations()).into());
    };
    assert_eq!(note.name.as_deref(), Some("stray"), "named");
    assert_eq!(
        note.kind,
        AnimationNoteKind::NoBoneMatched { skipped_tracks: 1 },
        "its track"
    );
    for label in [
        "Animations not written (2)",
        "no track matches a bone, one track skipped",
    ] {
        assert!(harness.query_by_label_contains(label).is_some(), "{label}");
    }
    Ok(())
}

#[test]
fn the_unmatched_animation_is_reported_in_the_interface_language() -> TestResult {
    let (_folder, harness) = exported(Language::German, with_stray())?;

    for label in [
        "Nicht geschriebene Animationen (2)",
        "keine Spur passt zu einem Knochen, eine Spur übersprungen",
    ] {
        assert!(harness.query_by_label_contains(label).is_some(), "{label}");
    }
    Ok(())
}
