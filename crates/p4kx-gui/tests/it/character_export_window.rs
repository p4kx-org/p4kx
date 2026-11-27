// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The export of the Character of the Characters tab as GLB: its setup, with progress, cancel and
//! the refusal of a Star Citizen installation.

use std::fs;
use std::io::{self, Write as _};
use std::path::Path;
use std::process;

use crate::characters::{
    TestResult, Window, built, opened, opened_in_installation, pick, settle, wait_until,
};
use egui::accesskit::Role;
use egui_kittest::kittest::Queryable;
use p4kx_extraction::{GlbOptions, OriginDigestWriter};
use p4kx_gui::ui::Dialog;
use p4kx_gui::{
    CharacterExportStatus, CharacterExportSummary, ChooserKind, Language, ModelExportFailure,
};
use p4kx_testkit::check_glb;
use serde_json::Value;

fn ended(window: &mut Window) -> TestResult {
    wait_until(&mut window.1, |model| {
        model
            .characters()
            .export()
            .is_some_and(|export| !export.is_running())
    })
}

fn composed() -> Result<Window, Box<dyn std::error::Error>> {
    compose(opened(Language::BritishEnglish)?)
}

pub(crate) fn compose(mut window: Window) -> Result<Window, Box<dyn std::error::Error>> {
    pick(&mut window.1, ChooserKind::Head, "Runner.chf")?;
    wait_until(&mut window.1, |model| {
        model.characters().head().is_some() && built(model)
    })?;
    pick(&mut window.1, ChooserKind::Piece, "TEST_Torso")?;
    window
        .1
        .state_mut()
        .model
        .characters_mut()
        .set_expressions_enabled(true);
    settle(&mut window.1)?;
    Ok(window)
}

fn start(window: &mut Window, file: &Path) -> Result<bool, &'static str> {
    let model = &mut window.1.state_mut().model;
    if !model.begin_character_export() {
        return Err("the setup did not open");
    }
    model
        .characters_mut()
        .export_setup_mut()
        .ok_or("no setup")?
        .set_file(file.to_path_buf());
    Ok(model.submit_character_export(|| {}))
}

#[test]
fn the_export_button_opens_the_setup_which_asks_the_host_for_a_file() -> TestResult {
    let mut window = opened(Language::BritishEnglish)?;
    window
        .1
        .get_by_role_and_label(Role::Button, "Export…")
        .click();
    window.1.run_steps(3);

    let setup = window
        .1
        .state()
        .model
        .characters()
        .export_setup()
        .ok_or("no setup")?;
    assert!(!setup.is_ready(), "no file chosen yet");
    assert!(window.1.query_by_label("Export Character").is_some());

    window
        .1
        .get_by_role_and_label(Role::Button, "Choose file…")
        .click();
    window.1.run_steps(2);

    assert_eq!(
        window.1.state().dialog,
        Some(Dialog::CharacterFile),
        "asked"
    );
    Ok(())
}

#[test]
fn the_character_is_written_as_one_skinned_glb_with_its_morph_targets() -> TestResult {
    let mut window = composed()?;
    let file = window.0.path().join("character");

    assert!(start(&mut window, &file)?, "started");
    window.1.run_steps(1);
    ended(&mut window)?;

    let model = &window.1.state().model;
    let export = model.characters().export().ok_or("no export")?;
    let CharacterExportStatus::Finished(summary) = export.status() else {
        return Err(format!("not finished: {export:?}").into());
    };
    let written = window.0.path().join("character.glb");
    assert_eq!(
        summary.file(),
        written,
        "a name without extension gets .glb"
    );
    assert!(summary.package().is_none(), "a GLB file");
    assert_eq!(summary.parts(), 4, "body, torso, head and teeth");
    let bytes = fs::read(&written)?;
    let armour = model.characters().armour().ok_or("the shown Character")?;
    let mut ordinary = io::BufWriter::new(Vec::new());
    armour.write_glb(
        &GlbOptions::default().with_lod(summary.lod()),
        &mut ordinary,
    )?;
    assert_eq!(
        bytes,
        ordinary.into_inner()?,
        "the ordinary GLB bytes are unchanged"
    );
    check_companion(summary, &bytes)?;
    let checked = check_glb(&bytes)?;
    let skin = checked.document.skins().next().ok_or("a skin")?;
    assert_eq!(skin.joints().count(), 3, "the male skeleton");
    let targets: usize = checked
        .document
        .meshes()
        .flat_map(|mesh| mesh.primitives())
        .map(|primitive| primitive.morph_targets().count())
        .sum();
    assert!(targets >= 2, "the expressions are morph targets: {targets}");
    Ok(())
}

fn check_companion(summary: &CharacterExportSummary, bytes: &[u8]) -> TestResult {
    let path = summary
        .origin_receipt()
        .ok_or("the Character GLB receipt")?;
    let expected = summary.file().with_extension("glb.p4kx-origin.jsonl");
    assert_eq!(
        fs::canonicalize(path)?,
        fs::canonicalize(expected)?,
        "the summary points to the canonical receipt"
    );
    let rows = fs::read_to_string(path)?
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<Vec<Value>, _>>()?;
    let binding = rows
        .first()
        .and_then(|row| row.get("binding"))
        .ok_or("the artifact binding")?;
    let mut measured = OriginDigestWriter::new(io::sink());
    measured.write_all(bytes)?;
    let expected = serde_json::to_value(measured.finish()?)?;
    for key in ["sha256", "byteLength"] {
        assert_eq!(binding.get(key), expected.get(key), "binding field {key}");
    }
    assert!(
        rows.iter().any(|row| {
            row.pointer("/address/path")
                .and_then(Value::as_str)
                .is_some_and(|path| path.starts_with("/accessors/") && path.ends_with("/count"))
                && row
                    .get("sourceUsesRefs")
                    .and_then(Value::as_array)
                    .is_some_and(|refs| !refs.is_empty())
        }),
        "an emitted geometry field names a physical producer"
    );
    Ok(())
}

#[test]
fn the_progress_window_shows_while_it_runs_and_the_summary_after() -> TestResult {
    let mut window = composed()?;
    let file = window.0.path().join("seen.glb");
    start(&mut window, &file)?;

    ended(&mut window)?;
    window.1.run_steps(3);

    assert!(
        window.1.query_by_label("Character exported").is_some(),
        "the summary is shown"
    );
    window.1.get_by_label("Close").click();
    window.1.run_steps(3);
    assert!(
        window.1.state().model.characters().export().is_none(),
        "dismissed"
    );
    Ok(())
}

#[test]
fn a_stopped_export_leaves_no_file() -> TestResult {
    let mut window = composed()?;
    let file = window.0.path().join("stopped.glb");
    let hold = window.1.state_mut().model.characters_mut().hold_export();
    start(&mut window, &file)?;

    window.1.state_mut().model.characters_mut().stop_export();
    drop(hold);

    let export = window
        .1
        .state()
        .model
        .characters()
        .export()
        .ok_or("no export")?;
    assert!(
        matches!(export.status(), CharacterExportStatus::Cancelled),
        "the held export is cancelled"
    );
    assert!(!file.try_exists()?, "no partial file");
    assert!(
        !file.with_extension("glb.p4kx-origin.jsonl").try_exists()?,
        "no receipt"
    );
    Ok(())
}

#[test]
fn a_failed_receipt_reports_that_the_character_glb_was_written() -> TestResult {
    let mut window = composed()?;
    let file = window.0.path().join("character.glb");
    let receipt = file.with_extension("glb.p4kx-origin.jsonl");
    let staging = receipt.with_extension(format!("jsonl.p4kx-{}.part", process::id()));
    fs::write(&staging, b"reserved receipt staging path")?;

    assert!(start(&mut window, &file)?, "the export started");
    ended(&mut window)?;
    let export = window
        .1
        .state()
        .model
        .characters()
        .export()
        .ok_or("no export")?;
    assert!(
        matches!(export.status(), CharacterExportStatus::Failed(ModelExportFailure::Failed(reason))
            if reason.contains("the GLB was committed")),
        "the failure names the GLB that already committed: {export:?}"
    );
    let bytes = fs::read(&file)?;
    assert!(
        check_glb(&bytes)?.document.skins().next().is_some(),
        "the committed character GLB is valid"
    );
    assert!(!receipt.try_exists()?, "no companion was published");
    assert_eq!(
        fs::read(staging)?,
        b"reserved receipt staging path",
        "the unrelated staging file is preserved"
    );
    Ok(())
}

#[test]
fn a_file_inside_an_installation_is_refused_before_the_export_starts() -> TestResult {
    let (window, channel) = opened_in_installation(Language::BritishEnglish)?;
    let mut window = compose(window)?;
    let file = channel.join("character.glb");

    assert!(window.1.state_mut().model.begin_character_export());
    let model = &mut window.1.state_mut().model;
    let setup = model
        .characters_mut()
        .export_setup_mut()
        .ok_or("no setup")?;
    setup.set_file(file.clone());
    assert!(setup.refusal().is_some(), "named in the setup");
    assert!(!setup.is_ready());
    assert!(!model.submit_character_export(|| {}), "not started");
    window.1.run_steps(3);

    assert!(
        window
            .1
            .query_by_label_contains("Star Citizen installation")
            .is_some(),
        "the dialog says so"
    );
    assert!(window.1.state().model.characters().export().is_none());
    assert!(!file.exists(), "nothing written");
    Ok(())
}
