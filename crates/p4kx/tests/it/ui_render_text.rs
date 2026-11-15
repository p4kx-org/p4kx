// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! `p4kx ui render` with texts end to end: glyph outlines of the embedded font drawn in the PNG
//! and written as paths in the SVG, localisation keys resolved from the Archive, and the report of
//! missing fonts and glyphs.
//!
//! `P4KX_UPDATE_REFERENCES=1` rewrites the reference images from the current renderer.

use crate::ui_panel::reference;

use std::error::Error;
use std::fs;
use std::path::Path;

use p4kx_testkit::{
    ArchiveBuilder, GlobalIniBuilder, TestEditText, TestEntry, TestGlyph, TestMovie, TestPlace,
    TestShapeRecord, TestSignature, TestTextRecord, define_text_body, font3_body,
};
use predicates::prelude::predicate;
use reference::assert_matches_reference;
use serde_json::Value;

type TestResult = Result<(), Box<dyn Error>>;

const MOVIE: &str = "Data\\UI\\Screens\\SWF\\Text.gfx";
const INI: &str = "Data\\Localization\\english\\global.ini";
const FONT: u16 = 3;

fn glyph(code: char, records: Vec<TestShapeRecord>) -> TestGlyph {
    TestGlyph {
        code: u16::try_from(u32::from(code)).unwrap_or(0),
        advance: 12_000,
        records,
    }
}

fn line(edges: &[(i32, i32)]) -> Vec<TestShapeRecord> {
    let mut records = vec![TestShapeRecord::Move {
        to: (0, 0),
        fill0: 0,
        fill1: 1,
        line: 0,
    }];
    records.extend(edges.iter().map(|&(x, y)| TestShapeRecord::Line(x, y)));
    records
}

/// A 100 x 60 pixel movie with a font of a square and a triangle, a red static text of three
/// glyphs, and a green text field holding `field`.
fn text_movie(field: &str) -> Vec<u8> {
    let square = glyph(
        'A',
        line(&[(10_000, 0), (0, -14_000), (-10_000, 0), (0, 14_000)]),
    );
    let triangle = glyph('C', line(&[(10_000, 0), (-10_000, -14_000), (0, 14_000)]));
    let record = TestTextRecord {
        font: Some((FONT, 480)),
        color: Some([255, 0, 0, 255]),
        x: Some(100),
        y: Some(560),
        glyphs: vec![(0, 280), (1, 280), (0, 280)],
    };
    let field = TestEditText::new(8, [0, 2000, 600, 1200])
        .text(field)
        .font(FONT, 400)
        .color([0, 255, 0, 255])
        .html();
    TestMovie::new(TestSignature::Cfx)
        .stage(100, 60)
        .tag(75, &font3_body(FONT, "Orbitron", &[square, triangle]))
        .tag(
            11,
            &define_text_body(7, [0, 2000, 0, 600], false, &[record]),
        )
        .tag(37, &field.body())
        .tag(26, &TestPlace::new(1, 7).body2())
        .tag(26, &TestPlace::new(2, 8).body2())
        .tag(1, &[])
        .bytes()
}

fn write(dir: &Path, field: &str) -> Result<std::path::PathBuf, Box<dyn Error>> {
    let file = dir.join("Text.gfx");
    fs::write(&file, text_movie(field))?;
    Ok(file)
}

fn archive(dir: &Path, field: &str) -> Result<std::path::PathBuf, Box<dyn Error>> {
    let path = dir.join("Data.p4k");
    let ini = GlobalIniBuilder::new()
        .text("ui_a", "A")
        .text("ui_c", "C%ls");
    ArchiveBuilder::new()
        .entry(TestEntry::zstd(MOVIE, text_movie(field)))
        .entry(TestEntry::zstd(INI, ini.build()))
        .write_to(&path)?;
    Ok(path)
}

#[test]
fn a_text_renders_like_its_reference() -> TestResult {
    let dir = tempfile::tempdir()?;
    let file = write(dir.path(), "AC")?;
    let out = dir.path().join("text.png");
    p4kx_testkit::p4kx()?
        .args(["ui", "render"])
        .arg(&file)
        .arg("--out")
        .arg(&out)
        .args(["--size", "200"])
        .assert()
        .success()
        .stdout(predicate::str::contains("0 elements left out"));
    assert_matches_reference(&out, "text.png")
}

#[test]
fn the_svg_draws_the_text_as_paths() -> TestResult {
    let dir = tempfile::tempdir()?;
    let file = write(dir.path(), "A&amp;C")?;
    let out = dir.path().join("text.svg");
    p4kx_testkit::p4kx()?
        .args(["ui", "render", "--svg"])
        .arg(&file)
        .arg("--out")
        .arg(&out)
        .assert()
        .code(1);
    let svg = fs::read_to_string(&out)?;
    assert!(svg.contains("<path"), "{svg}");
    assert!(
        !svg.contains("<text") && !svg.contains("<tspan") && !svg.contains("font-family"),
        "{svg}"
    );
    assert!(
        !svg.contains("A&amp;C") && !svg.contains("&amp;amp;"),
        "{svg}"
    );
    Ok(())
}

#[test]
fn keys_resolve_in_the_language_of_the_archive_and_unknown_ones_are_listed() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = archive(dir.path(), "@ui_a,P=x")?;
    let plain = write(dir.path(), "A")?;
    let expected = dir.path().join("expected.png");
    let localised = dir.path().join("localised.png");
    p4kx_testkit::p4kx()?
        .args(["ui", "render"])
        .arg(&plain)
        .arg("--out")
        .arg(&expected)
        .assert()
        .success();
    p4kx_testkit::p4kx()?
        .args(["ui", "render"])
        .arg(&archive)
        .arg("data/ui/screens/swf/text.gfx")
        .arg("--out")
        .arg(&localised)
        .args(["--lang", "en"])
        .assert()
        .success();
    assert_eq!(
        fs::read(&expected)?,
        fs::read(&localised)?,
        "the key draws as its text"
    );
    let unknown = self::archive(dir.path(), "@ui_nothing")?;
    let output = p4kx_testkit::p4kx()?
        .args(["--json", "ui", "render"])
        .arg(&unknown)
        .arg(MOVIE)
        .arg("--out")
        .arg(dir.path().join("unknown.png"))
        .args(["--lang", "en"])
        .output()?;
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let report: Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(report["skipped"][0]["reason"], "unlocalised");
    assert_eq!(report["skipped"][0]["drawn"], true);
    Ok(())
}

#[test]
fn a_truncated_global_ini_fails_before_writing_the_image() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("Data.p4k");
    let out = dir.path().join("x.png");
    let ini = GlobalIniBuilder::new().text("ui_a", "A").build();
    let declared_size = u64::try_from(ini.len())? + 1;
    ArchiveBuilder::new()
        .entry(TestEntry::zstd(MOVIE, text_movie("@ui_a")))
        .entry(TestEntry::stored(INI, ini).with_uncompressed_size(declared_size))
        .write_to(&path)?;
    p4kx_testkit::p4kx()?
        .args(["ui", "render"])
        .arg(&path)
        .arg(MOVIE)
        .arg("--out")
        .arg(&out)
        .args(["--lang", "en"])
        .assert()
        .code(3)
        .stderr(predicate::str::contains("cannot read the Entry"))
        .stderr(predicate::str::contains("global.ini"));
    assert!(!out.exists(), "a failed input produces no image");
    Ok(())
}

#[test]
fn a_language_needs_an_archive() -> TestResult {
    let dir = tempfile::tempdir()?;
    let file = write(dir.path(), "A")?;
    p4kx_testkit::p4kx()?
        .args(["ui", "render"])
        .arg(&file)
        .arg("--out")
        .arg(dir.path().join("x.png"))
        .args(["--lang", "en"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("ENTRY"));
    Ok(())
}

#[test]
fn missing_glyphs_are_drawn_as_boxes_and_listed() -> TestResult {
    let dir = tempfile::tempdir()?;
    let file = write(dir.path(), "AB")?;
    let output = p4kx_testkit::p4kx()?
        .args(["--json", "ui", "render"])
        .arg(&file)
        .arg("--out")
        .arg(dir.path().join("boxes.png"))
        .output()?;
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let report: Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(
        report["skipped"],
        serde_json::json!([{
            "id": 8,
            "depth": 2,
            "reason": "missing_glyph",
            "drawn": true,
            "message": "the font has no glyph for U+0042; an empty box is drawn",
        }])
    );
    Ok(())
}
