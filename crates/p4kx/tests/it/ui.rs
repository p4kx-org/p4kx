// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! `p4kx ui assets|extract` end to end, for UI movie Entries of an Archive and files on disk,
//! plain and as JSON snapshots.

use std::error::Error;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

use assert_cmd::Command;
use p4kx_testkit::{
    ArchiveBuilder, SplitDds, TestEntry, TestFill, TestGlyph, TestMovie, TestShape,
    TestShapeRecord, TestSignature, TestTextureFormat, exporter_info_body, exports_body,
    external_image_body, font3_body, gray_jpeg, jpeg_body, lossless_body, morph_shape_body,
    place_object2_body, sprite_body, sub_image_body,
};
use predicates::prelude::predicate;
use serde_json::Value;

type TestResult = Result<(), Box<dyn Error>>;

const MOVIE: &str = "Data\\UI\\Screens\\SWF\\Panel.gfx";
const TEXTURE: &str = "Data\\UI\\Screens\\Textures\\panel_bg.dds";

fn movie() -> Vec<u8> {
    let start = TestShapeRecord::Move {
        to: (0, 0),
        fill0: 1,
        fill1: 0,
        line: 1,
    };
    let shape = TestShape::new(4, 3, [0, 2000, 0, 1010])
        .fill(TestFill::Solid([200, 30, 30, 255]))
        .line(20, [0, 0, 0, 255])
        .record(start)
        .record(TestShapeRecord::Line(2000, 0))
        .record(TestShapeRecord::Curve(0, 1010, -2000, 0));
    let glyph = TestGlyph {
        code: 65,
        advance: 1100,
        records: vec![start, TestShapeRecord::Line(900, 0)],
    };
    TestMovie::new(TestSignature::Cfx)
        .tag(1000, &exporter_info_body("Panel"))
        .tag(
            36,
            &lossless_body(1, 2, 1, &[[255, 0, 0, 255], [0, 0, 255, 128]], true),
        )
        .tag(
            1009,
            &external_image_body(2, 2, 19, (8, 4), ("bg", "../Textures/panel_bg.tif")),
        )
        .tag(1008, &sub_image_body(3, 2, [2, 1, 6, 3]))
        .tag(
            1009,
            &external_image_body(5, 2, 14, (16, 16), ("gone", "../Textures/gone.dds")),
        )
        .tag(21, &jpeg_body(7, &gray_jpeg(8, 8, 128), None))
        .tag(shape.code(), &shape.body())
        .tag(
            46,
            &morph_shape_body(6, ([1; 4], [2; 4]), &[start], &[start]),
        )
        .tag(75, &font3_body(8, "Electrolize", &[glyph]))
        .tag(
            39,
            &sprite_body(
                9,
                2,
                &[
                    (26, place_object2_body(1, 4)),
                    (43, b"idle\0".to_vec()),
                    (1, Vec::new()),
                    (1, Vec::new()),
                ],
            ),
        )
        .tag(56, &exports_body(&[(9, "PanelButton"), (1, "icon_alert")]))
        .tag(26, &place_object2_body(1, 9))
        .tag(777, b"future")
        .tag(1, &[])
        .tag(2, &[4, 0])
        .bytes()
}

fn write_archive(folder: &Path) -> Result<PathBuf, Box<dyn Error>> {
    let path = folder.join("Data.p4k");
    let mut builder = ArchiveBuilder::new().entry(TestEntry::zstd(MOVIE, movie()));
    for entry in SplitDds::new(TestTextureFormat::LegacyBgra8, 8, 4).entries(TEXTURE) {
        builder = builder.entry(entry);
    }
    builder.write_to(&path)?;
    Ok(path)
}

fn p4kx() -> Result<Command, Box<dyn Error>> {
    Ok(p4kx_testkit::p4kx()?)
}

fn json(dir: &Path, args: &[&OsStr]) -> Result<(Option<i32>, String), Box<dyn Error>> {
    let output = p4kx()?.arg("--json").args(args).output()?;
    let mut value: Value = serde_json::from_slice(&output.stdout)?;
    redact(&mut value, &dir.display().to_string());
    Ok((output.status.code(), serde_json::to_string_pretty(&value)?))
}

fn redact(value: &mut Value, prefix: &str) {
    match value {
        Value::String(text) => {
            if let Some(rest) = text.strip_prefix(prefix) {
                *text = format!("[DIR]{}", rest.replace('\\', "/"));
            }
        }
        Value::Array(items) => items.iter_mut().for_each(|item| redact(item, prefix)),
        Value::Object(fields) => fields.values_mut().for_each(|item| redact(item, prefix)),
        _ => {}
    }
}

#[test]
fn assets_of_an_entry_lists_every_definition_with_its_id() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;

    let output = p4kx()?
        .args(["ui", "assets"])
        .arg(&archive)
        .arg("data/ui/screens/swf/PANEL.gfx")
        .output()?;

    assert_eq!(
        output.status.code(),
        Some(1),
        "an unreadable tag: {output:?}"
    );
    insta::assert_snapshot!(String::from_utf8(output.stdout)?);
    Ok(())
}

#[test]
fn assets_as_json() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;

    let (code, json) = json(
        dir.path(),
        &[
            "ui".as_ref(),
            "assets".as_ref(),
            archive.as_os_str(),
            MOVIE.as_ref(),
        ],
    )?;

    assert_eq!(code, Some(1), "an unreadable tag");
    let mut report: Value = serde_json::from_str(&json)?;
    crate::ui_asset_origins::assert_all_fields(&report)?;
    let fields = report.as_object_mut().ok_or("UI assets report")?;
    assert!(
        fields.remove("fieldOrigins").is_some(),
        "origins accompany the old fields"
    );
    assert!(
        fields.remove("provenance").is_some(),
        "source inventory accompanies the old fields"
    );
    insta::assert_snapshot!(serde_json::to_string_pretty(&report)?);
    Ok(())
}

#[test]
fn assets_of_a_file_on_disk_lists_the_same_definitions() -> TestResult {
    let dir = tempfile::tempdir()?;
    let file = dir.path().join("Panel.swf");
    fs::write(&file, movie())?;

    p4kx()?
        .args(["ui", "assets"])
        .arg(&file)
        .assert()
        .code(1)
        .stdout(predicate::str::contains("Bitmaps: 5"))
        .stdout(predicate::str::contains("Sprites: 1"))
        .stdout(predicate::str::contains("PanelButton"));
    Ok(())
}

#[test]
fn extract_writes_png_and_json_and_fails_partly_on_a_missing_texture() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;
    let out = dir.path().join("assets");

    p4kx()?
        .args(["ui", "extract"])
        .arg(&archive)
        .arg(MOVIE)
        .arg("--out")
        .arg(&out)
        .assert()
        .code(1)
        .stdout(predicate::str::contains("extracted 6 of 7 definitions"))
        .stdout(predicate::str::contains("1 tags unreadable"))
        .stderr(predicate::str::contains("unreadable tag 2 at offset"))
        .stderr(predicate::str::contains(
            "../Textures/gone.dds, which does not exist",
        ));
    for file in [
        "bitmap_1.png",
        "bitmap_2.png",
        "bitmap_3.png",
        "bitmap_7.png",
        "shape_4.json",
        "morph_shape_6.json",
    ] {
        assert!(out.join(file).is_file(), "{file}");
    }
    assert!(!out.join("bitmap_5.png").exists(), "the missing texture");
    Ok(())
}

#[test]
fn extract_as_json() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;
    let out = dir.path().join("assets");

    let (code, json) = json(
        dir.path(),
        &[
            "ui".as_ref(),
            "extract".as_ref(),
            archive.as_os_str(),
            MOVIE.as_ref(),
            "--out".as_ref(),
            out.as_os_str(),
        ],
    )?;

    assert_eq!(code, Some(1), "partial failure");
    let mut report: Value = serde_json::from_str(&json)?;
    assert_eq!(report["originReceipt"], ".p4kx-origin.jsonl");
    let fields = report.as_object_mut().ok_or("extract report")?;
    assert!(fields.remove("fieldOrigins").is_some());
    assert!(fields.remove("provenance").is_some());
    assert!(fields.remove("originReceipt").is_some());
    insta::assert_snapshot!(serde_json::to_string_pretty(&report)?);
    Ok(())
}

#[test]
fn a_missing_entry_or_file_is_invalid_input_and_a_corrupt_movie_unreadable() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;
    p4kx()?
        .args(["ui", "assets"])
        .arg(&archive)
        .arg("Data\\UI\\missing.gfx")
        .assert()
        .code(2);
    p4kx()?
        .args(["ui", "assets"])
        .arg(dir.path().join("missing.gfx"))
        .assert()
        .code(2);
    let corrupt = dir.path().join("corrupt.gfx");
    fs::write(&corrupt, b"CWS\x0A\x00\x01\x00\x00garbage")?;
    p4kx()?
        .args(["ui", "assets"])
        .arg(&corrupt)
        .assert()
        .code(3)
        .stderr(predicate::str::contains("cannot decompress the movie"));
    Ok(())
}

#[test]
fn a_truncated_bitmap_tag_fails_the_extraction_with_its_code_and_offset() -> TestResult {
    let dir = tempfile::tempdir()?;
    let file = dir.path().join("Broken.gfx");
    let lossless = lossless_body(1, 2, 1, &[[1, 2, 3, 255], [4, 5, 6, 255]], false);
    let movie = TestMovie::new(TestSignature::Gfx)
        .tag(20, &lossless)
        .tag(20, lossless.get(..4).unwrap_or_default())
        .bytes();
    fs::write(&file, movie)?;
    let out = dir.path().join("out");

    p4kx()?
        .args(["ui", "extract"])
        .arg(&file)
        .arg("--out")
        .arg(&out)
        .assert()
        .code(1)
        .stdout(predicate::str::contains("extracted 1 of 1 definitions"))
        .stderr(predicate::str::contains("unreadable tag 20 at offset"));
    let (code, json) = json(
        dir.path(),
        &[
            "ui".as_ref(),
            "extract".as_ref(),
            file.as_os_str(),
            "--out".as_ref(),
            out.as_os_str(),
        ],
    )?;
    assert_eq!(code, Some(1), "partial failure");
    let value: Value = serde_json::from_str(&json)?;
    let failed = value.pointer("/failed/0").ok_or("one failure")?;
    assert_eq!(failed.get("kind"), Some(&Value::from("tag")), "{json}");
    assert_eq!(failed.get("code"), Some(&Value::from(20)), "{json}");
    assert!(failed.get("offset").is_some_and(Value::is_u64), "{json}");
    assert!(
        out.join("bitmap_1.png").is_file(),
        "the good bitmap is written"
    );
    Ok(())
}
