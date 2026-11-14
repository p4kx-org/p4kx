// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! `p4kx ui render` end to end: rendered PNGs compared with reference images within a
//! tolerance, the size and background options, the report of what was left out, and bad
//! arguments.
//!
//! `P4KX_UPDATE_REFERENCES=1` rewrites the reference images from the current renderer.

use crate::ui_panel::{self, reference};

use std::error::Error;
use std::fs;

use p4kx_testkit::{ArchiveBuilder, TestEntry};
use predicates::prelude::predicate;
use reference::assert_matches_reference;
use serde_json::Value;
use ui_panel::{p4kx, panel, write_panel};

type TestResult = Result<(), Box<dyn Error>>;

const MOVIE: &str = "Data\\UI\\Screens\\SWF\\Panel.gfx";

#[test]
fn a_panel_renders_like_its_reference_at_stage_size() -> TestResult {
    let dir = tempfile::tempdir()?;
    let file = write_panel(dir.path())?;
    let out = dir.path().join("panel.png");
    p4kx()?
        .args(["ui", "render"])
        .arg(&file)
        .arg("--out")
        .arg(&out)
        .assert()
        .code(1)
        .stdout(predicate::str::contains("64 x 48 px; 1 elements left out"))
        .stderr(predicate::str::contains(
            "skipped: definition 50 at depth 7: DefineButton2 definitions are not drawn",
        ));
    assert_matches_reference(&out, "panel.png")
}

#[test]
fn size_and_background_scale_the_panel_and_fill_under_it() -> TestResult {
    let dir = tempfile::tempdir()?;
    let file = write_panel(dir.path())?;
    let fitted = dir.path().join("fitted.png");
    p4kx()?
        .args(["ui", "render"])
        .arg(&file)
        .arg("--out")
        .arg(&fitted)
        .args(["--size", "160x48", "--background", "#20304080"])
        .assert()
        .code(1);
    assert_matches_reference(&fitted, "panel_fitted.png")?;
    let wide = dir.path().join("wide.png");
    p4kx()?
        .args(["ui", "render"])
        .arg(&file)
        .arg("--out")
        .arg(&wide)
        .args(["--size", "128", "--background", "movie"])
        .assert()
        .code(1);
    assert_matches_reference(&wide, "panel_wide.png")
}

#[test]
fn the_json_report_lists_what_was_left_out() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = dir.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::zstd(MOVIE, panel()))
        .write_to(&archive)?;
    let out = dir.path().join("panel.png");
    let output = p4kx()?
        .args(["--json", "ui", "render"])
        .arg(&archive)
        .arg("data/ui/screens/swf/panel.gfx")
        .arg("--out")
        .arg(&out)
        .output()?;
    assert_eq!(output.status.code(), Some(1), "a text: {output:?}");
    let report: Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(report["movie"], MOVIE, "the Entry");
    assert_eq!(
        (report["width"].as_u64(), report["height"].as_u64()),
        (Some(64), Some(48)),
        "size"
    );
    assert_eq!(
        report["skipped"],
        serde_json::json!([{
            "id": 50,
            "depth": 7,
            "reason": "not_drawn",
            "drawn": false,
            "message": "DefineButton2 definitions are not drawn",
        }]),
        "the text"
    );
    assert_eq!(report["failed"], serde_json::json!([]), "nothing failed");
    let mut redacted = report;
    assert_eq!(
        redacted["target"].as_str(),
        Some(out.display().to_string().as_str()),
        "the PNG"
    );
    redacted["target"] = Value::from("[DIR]/panel.png");
    let fields = redacted.as_object_mut().ok_or("render report object")?;
    for key in [
        "originReceipt",
        "artifactOrigin",
        "fieldOrigins",
        "provenance",
    ] {
        assert!(
            fields.remove(key).is_some(),
            "missing source evidence {key}"
        );
    }
    insta::assert_snapshot!(serde_json::to_string_pretty(&redacted)?);
    assert_matches_reference(&out, "panel.png")
}

#[test]
fn bad_sizes_backgrounds_and_outputs_are_invalid_input() -> TestResult {
    let dir = tempfile::tempdir()?;
    let file = write_panel(dir.path())?;
    for (option, value) in [
        ("--size", "0x10"),
        ("--size", "20000"),
        ("--size", "8000x8000"),
        ("--size", "wide"),
        ("--background", "red"),
        ("--background", "#12345"),
    ] {
        p4kx()?
            .args(["ui", "render"])
            .arg(&file)
            .arg("--out")
            .arg(dir.path().join("x.png"))
            .arg(option)
            .arg(value)
            .assert()
            .code(2);
    }
    p4kx()?
        .args(["ui", "render"])
        .arg(&file)
        .arg("--out")
        .arg(&file)
        .assert()
        .code(2)
        .stderr(predicate::str::contains("is one of the inputs"));
    assert_eq!(fs::read(&file)?, panel(), "the movie is untouched");
    Ok(())
}
