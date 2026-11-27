// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! `p4kx ui screens` and `p4kx ui render --screen` end to end over the synthetic sample ship:
//! the listing as text and JSON, rendered PNGs compared with a reference image, SVG output, the
//! report of what was left out and bad arguments.
//!
//! `P4KX_UPDATE_REFERENCES=1` rewrites the reference images from the current renderer.

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use p4kx_testkit::{
    ArchiveBuilder, GlobalIniBuilder, TestEntry, p4kx, screens_font_movie, screens_sample,
};
use predicates::prelude::{PredicateBooleanExt, predicate};
use serde_json::Value;

use crate::ui_panel::reference::assert_matches_reference;

#[path = "ui_screens_origins.rs"]
mod origins;

type TestResult = Result<(), Box<dyn Error>>;

const MFD: &str = "libs/foundry/records/ui/buildingblocks/test/test_mfd";

fn archive(dir: &Path) -> Result<PathBuf, Box<dyn Error>> {
    let path = dir.join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("Data\\Game2.dcb", screens_sample().build()))
        .write_to(&path)?;
    Ok(path)
}

#[test]
fn screens_lists_each_screen_with_its_assets() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = archive(dir.path())?;
    p4kx()?
        .args(["ui", "screens"])
        .arg(&archive)
        .arg("TEST_ScreenShip")
        .assert()
        .success()
        .stdout(predicate::str::contains("Entity:  TEST_ScreenShip"))
        .stdout(predicate::str::contains("Brand:   test"))
        .stdout(predicate::str::contains("Screens: 2"))
        .stdout(predicate::str::contains(
            "Left_MFD  TEST_Screen_MFD  200 x 100 px",
        ))
        .stdout(predicate::str::contains(format!("canvas:    {MFD}")))
        .stdout(predicate::str::contains(
            "uses:      2 canvases, 1 textures, 1 vector graphics, 0 movies, 1 localisation keys",
        ))
        .stdout(predicate::str::contains(
            "texture:   UI/Textures/Test/picture.dds",
        ))
        .stdout(predicate::str::contains("WidgetText 1"))
        .stdout(predicate::str::contains("WidgetText 1 (not drawn)").not())
        .stdout(predicate::str::contains("WidgetCircle 1"))
        .stdout(predicate::str::contains(
            "Radar  TEST_Screen_Panel  100 x 50 px",
        ))
        .stdout(predicate::str::contains("UI movies: 1"))
        .stdout(predicate::str::contains(
            "Left_MFD  TEST_Screen_MFD  Data/UI/test.gfx",
        ));
    Ok(())
}

#[test]
fn screens_json_carries_the_listing() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = archive(dir.path())?;
    let output = p4kx()?
        .args(["--json", "ui", "screens"])
        .arg(&archive)
        .arg("TEST_ScreenShip")
        .output()?;
    assert_eq!(output.status.code(), Some(0), "exit code: {output:?}");
    let listing: Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(listing["entity"], "TEST_ScreenShip", "the entity");
    assert_eq!(listing["brand"], "test", "the brand");
    let screens = listing["screens"].as_array().ok_or("no screens")?;
    assert_eq!(screens.len(), 2, "both screens");
    let mfd = screens.first().ok_or("no MFD")?;
    assert_eq!(mfd["port"], "Left_MFD");
    assert_eq!(mfd["canvas"], MFD);
    assert_eq!(mfd["assets"]["textures"][0], "UI/Textures/Test/picture.dds");
    assert_eq!(mfd["assets"]["localisation_keys"][0], "test_label");
    let text = mfd["elements"]
        .as_array()
        .and_then(|elements| elements.iter().find(|e| e["element"] == "WidgetText"))
        .ok_or("no text element")?;
    assert_eq!(
        (text["count"].as_u64(), text["rendered"].as_bool()),
        (Some(1), Some(true))
    );
    assert_eq!(
        listing["movies"][0]["movie"], "Data/UI/test.gfx",
        "the UI movie"
    );
    Ok(())
}

#[test]
fn screens_of_an_unknown_entity_is_invalid_input() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = archive(dir.path())?;
    p4kx()?
        .args(["ui", "screens"])
        .arg(&archive)
        .arg("NoSuchEntity")
        .assert()
        .code(2);
    Ok(())
}

#[test]
fn a_screen_renders_like_its_reference_and_lists_what_was_left_out() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = archive(dir.path())?;
    let out = dir.path().join("mfd.png");
    p4kx()?
        .args(["ui", "render"])
        .arg(&archive)
        .args(["--screen", MFD, "--brand", "test", "--out"])
        .arg(&out)
        .assert()
        .code(1)
        .stdout(predicate::str::contains("200 x 100 px"))
        .stdout(predicate::str::contains("in 2 canvases"))
        .stderr(predicate::str::contains(
            "left out: WidgetText x1: no renderer for this element type (label)",
        ))
        .stderr(predicate::str::contains("left out: WidgetImage x1"))
        .stderr(predicate::str::contains("left out: WidgetCustomShape x1"));
    assert_matches_reference(&out, "screen_mfd.png")
}

#[test]
fn texts_are_drawn_from_the_font_movie_in_the_language() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("Data\\Game2.dcb", screens_sample().build()))
        .entry(TestEntry::zstd(
            "Data\\UI\\fonts\\Shared\\fonts_en.gfx",
            screens_font_movie(),
        ))
        .entry(TestEntry::zstd(
            "Data\\Localization\\english\\global.ini",
            GlobalIniBuilder::new().text("test_label", "AA").build(),
        ))
        .write_to(&path)?;
    let out = dir.path().join("texts.png");
    p4kx()?
        .args(["ui", "render"])
        .arg(&path)
        .args(["--screen", MFD, "--brand", "test", "--out"])
        .arg(&out)
        .assert()
        .code(1)
        .stdout(predicate::str::contains("1 texts drawn and 0 left out"))
        .stderr(predicate::str::contains("no renderer for this element type (label)").not());
    Ok(())
}

#[test]
fn size_and_background_apply_to_screens_too() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = archive(dir.path())?;
    let out = dir.path().join("fitted.png");
    p4kx()?
        .args(["ui", "render"])
        .arg(&archive)
        .args(["--screen", MFD, "--brand", "test", "--size", "400x300"])
        .args(["--background", "#204080"])
        .arg("--out")
        .arg(&out)
        .assert()
        .code(1)
        .stdout(predicate::str::contains("400 x 300 px"));
    assert_matches_reference(&out, "screen_mfd_fitted.png")
}

#[test]
fn game_variables_are_listed_in_the_report_and_unknown_names_are_reported() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = archive(dir.path())?;
    let out = dir.path().join("vars.png");
    let output = p4kx()?
        .args(["--json", "ui", "render"])
        .arg(&archive)
        .args(["--screen", MFD, "--brand", "test"])
        .args(["--var", "powerstate=1", "--var", "isradar=true", "--out"])
        .arg(&out)
        .output()?;
    assert_eq!(output.status.code(), Some(1), "exit code: {output:?}");
    let report: Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(
        report["variables"],
        serde_json::json!([
            {"name": "powerstate", "value": "1", "used": false},
            {"name": "isradar", "value": "true", "used": false},
        ])
    );
    p4kx()?
        .args(["ui", "render"])
        .arg(&archive)
        .args(["--screen", MFD, "--brand", "test", "--var", "powerstate=1"])
        .arg("--out")
        .arg(&out)
        .assert()
        .code(1)
        .stdout(predicate::str::contains(
            "game variables in force: powerstate=1",
        ))
        .stderr(predicate::str::contains(
            "unknown game variable: powerstate (no operation of the screen reads it)",
        ));
    Ok(())
}

#[test]
fn without_a_variable_the_report_has_no_variables_field() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = archive(dir.path())?;
    let output = p4kx()?
        .args(["--json", "ui", "render"])
        .arg(&archive)
        .args(["--screen", MFD, "--brand", "test", "--out"])
        .arg(dir.path().join("rest.png"))
        .output()?;
    let report: Value = serde_json::from_slice(&output.stdout)?;
    assert!(report.get("variables").is_none(), "{report}");
    Ok(())
}

#[test]
fn a_malformed_or_stray_variable_is_invalid_input() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = archive(dir.path())?;
    for setting in ["powerstate", "=1", "a=\u{7}"] {
        p4kx()?
            .args(["ui", "render"])
            .arg(&archive)
            .args(["--screen", MFD, "--var", setting, "--out"])
            .arg(dir.path().join("bad.png"))
            .assert()
            .code(2);
    }
    let many: Vec<String> = (0..65)
        .flat_map(|at| ["--var".to_owned(), format!("v{at}=1")])
        .collect();
    p4kx()?
        .args(["ui", "render"])
        .arg(dir.path().join("missing.p4k"))
        .args(["--screen", MFD])
        .args(&many)
        .arg("--out")
        .arg(dir.path().join("many.png"))
        .assert()
        .code(2)
        .stderr(predicate::str::contains("65 game variables are set"));
    p4kx()?
        .args(["ui", "render"])
        .arg(dir.path().join("panel.gfx"))
        .args(["--var", "a=1", "--out"])
        .arg(dir.path().join("movie.png"))
        .assert()
        .code(2);
    Ok(())
}

#[test]
fn a_screen_renders_as_svg_with_a_path_per_shape() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = archive(dir.path())?;
    let out = dir.path().join("mfd.svg");
    let output = p4kx()?
        .args(["--json", "ui", "render"])
        .arg(&archive)
        .args(["--screen", MFD, "--brand", "test", "--svg", "--out"])
        .arg(&out)
        .output()?;
    assert_eq!(output.status.code(), Some(1), "exit code: {output:?}");
    let report: Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(report["format"], "svg", "the format");
    assert_eq!(
        (report["width"].as_u64(), report["height"].as_u64()),
        (Some(200), Some(100))
    );
    let shapes = report["shapes"].as_u64().ok_or("no shape count")?;
    let svg = fs::read_to_string(&out)?;
    assert_eq!(
        svg.matches("<path").count() as u64,
        shapes,
        "one path per shape"
    );
    let unsupported = report["unsupported"]
        .as_array()
        .ok_or("no unsupported list")?;
    assert!(
        unsupported
            .iter()
            .any(|entry| entry["element"] == "WidgetText"),
        "the JSON report lists the text"
    );
    Ok(())
}

#[test]
fn rendering_an_unknown_screen_is_invalid_input() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = archive(dir.path())?;
    p4kx()?
        .args(["ui", "render"])
        .arg(&archive)
        .args(["--screen", "libs/none", "--out"])
        .arg(dir.path().join("none.png"))
        .assert()
        .code(2)
        .stderr(predicate::str::contains(
            "no BuildingBlocks canvas Record matches",
        ));
    Ok(())
}

#[test]
fn a_screen_and_a_movie_entry_exclude_each_other() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = archive(dir.path())?;
    p4kx()?
        .args(["ui", "render"])
        .arg(&archive)
        .arg("data/ui/panel.gfx")
        .args(["--screen", MFD, "--out"])
        .arg(dir.path().join("x.png"))
        .assert()
        .code(2);
    Ok(())
}
