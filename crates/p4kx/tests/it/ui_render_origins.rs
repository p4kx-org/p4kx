// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::io::{self, Write as _};
use std::path::Path;

use p4kx_extraction::OriginDigestWriter;
use p4kx_testkit::{
    ArchiveBuilder, SplitDds, TestEntry, TestFill, TestMovie, TestPlace, TestShape,
    TestShapeRecord, TestSignature, TestTextureFormat, external_image_body,
};
use serde_json::Value;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const MOVIE: &str = "Data/UI/panel.gfx";
const TEXTURE: &str = "Data/UI/atlas.dds";

#[derive(Clone, Copy)]
enum EncryptedInput {
    None,
    Movie,
    Alpha,
}

fn archive(path: &Path, encrypted: EncryptedInput) -> TestResult {
    let shape = TestShape::new(8, 3, [0, 160, 0, 80])
        .fill(TestFill::ScaledBitmap {
            id: 2,
            scale: 20,
            at: (0, 0),
            repeating: false,
            smoothed: false,
        })
        .record(TestShapeRecord::Move {
            to: (0, 0),
            fill0: 0,
            fill1: 1,
            line: 0,
        })
        .record(TestShapeRecord::Line(160, 0))
        .record(TestShapeRecord::Line(0, 80))
        .record(TestShapeRecord::Line(-160, 0))
        .record(TestShapeRecord::Line(0, -80));
    let movie = TestMovie::new(TestSignature::Cfx)
        .stage(16, 8)
        .tag(
            1009,
            &external_image_body(2, 2, 19, (8, 4), ("atlas", "atlas.tif")),
        )
        .tag(shape.code(), &shape.body())
        .tag(26, &TestPlace::new(1, 8).body2())
        .tag(1, &[])
        .bytes();
    let movie = TestEntry::stored(MOVIE, movie);
    let movie = if matches!(encrypted, EncryptedInput::Movie) {
        movie.encrypted()
    } else {
        movie
    };
    let mut archive = ArchiveBuilder::new().entry(movie);
    let texture = SplitDds::new(TestTextureFormat::LegacyBgra8, 8, 4)
        .split(2)
        .alpha(TestTextureFormat::R8);
    for entry in texture.entries(TEXTURE) {
        let entry = if matches!(encrypted, EncryptedInput::Alpha)
            && entry.path() == "Data/UI/atlas.dds.2a"
        {
            entry.encrypted()
        } else {
            entry
        };
        archive = archive.entry(entry);
    }
    archive.write_to(path)?;
    Ok(())
}

fn render(data: &Path, out: &Path, svg: bool) -> TestResult<Value> {
    let mut command = p4kx_testkit::p4kx()?;
    command
        .args(["--json", "ui", "render"])
        .arg(data)
        .arg(MOVIE)
        .args(["--size", "32", "--out"])
        .arg(out);
    if svg {
        command.arg("--svg");
    }
    let output = command.output()?;
    assert!(output.status.success(), "{output:?}");
    Ok(serde_json::from_slice(&output.stdout)?)
}

fn verify_companion(
    folder: &Path,
    file: &str,
    summary: &Value,
    encrypted: bool,
) -> TestResult<Vec<u8>> {
    let name = format!("{file}.p4kx-origin.jsonl");
    assert_eq!(
        summary.get("originReceipt").and_then(Value::as_str),
        Some(name.as_str()),
        "relative sibling name"
    );
    for field in ["/movie", "/target", "/width", "/height", "/format"] {
        assert_eq!(
            marker(summary, field),
            Some(false),
            "only the bitmap payload is encrypted, field {field} has a different origin"
        );
    }
    assert_eq!(
        summary
            .pointer("/artifactOrigin/containsEncryptedSource")
            .and_then(Value::as_bool),
        Some(encrypted),
        "the frame includes the observed alpha payload"
    );
    let bytes = fs::read(folder.join(file))?;
    let mut measured = OriginDigestWriter::new(io::sink());
    measured.write_all(&bytes)?;
    let expected = serde_json::to_value(measured.finish()?)?;
    let rows = fs::read_to_string(folder.join(name))?
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<Vec<Value>, _>>()?;
    check_binding(&rows, file, &expected, encrypted)?;
    Ok(bytes)
}

fn check_binding(rows: &[Value], file: &str, expected: &Value, encrypted: bool) -> TestResult {
    let binding = rows.first().ok_or("receipt header")?;
    assert_eq!(
        binding
            .pointer("/binding/path/value")
            .and_then(Value::as_str),
        Some(file),
        "the actual output filename is bound"
    );
    for field in ["sha256", "byteLength"] {
        let bound = binding
            .get("binding")
            .and_then(|binding| binding.get(field));
        assert_eq!(bound, expected.get(field), "measured {field}");
    }
    let origin = rows
        .iter()
        .find(|row| row.get("type").and_then(Value::as_str) == Some("field"))
        .ok_or("artifact origin")?;
    assert_eq!(
        origin.pointer("/address/type").and_then(Value::as_str),
        Some("opaquePayload"),
        "the whole encoded image is the addressed datum"
    );
    assert_eq!(
        origin
            .get("containsEncryptedSource")
            .and_then(Value::as_bool),
        Some(encrypted),
        "the receipt binds the current source"
    );
    assert_eq!(
        origin.get("sourceSetComplete").and_then(Value::as_bool),
        Some(true),
        "every actual source is available for the synthetic frame"
    );
    assert_eq!(
        rows.last()
            .and_then(|row| row.get("type"))
            .and_then(Value::as_str),
        Some("end"),
        "the companion completed"
    );
    Ok(())
}

#[test]
fn png_and_svg_receipts_bind_exact_bytes_and_replace_identical_prior_origins() -> TestResult {
    let folder = tempfile::tempdir()?;
    let encrypted = folder.path().join("encrypted.p4k");
    let clear = folder.path().join("clear.p4k");
    archive(&encrypted, EncryptedInput::Alpha)?;
    archive(&clear, EncryptedInput::None)?;
    for (file, svg) in [("panel.png", false), ("panel.svg", true)] {
        let out = folder.path().join(file);
        let prior = render(&encrypted, &out, svg)?;
        let before = verify_companion(folder.path(), file, &prior, true)?;
        let replaced = render(&clear, &out, svg)?;
        let after = verify_companion(folder.path(), file, &replaced, false)?;
        assert_eq!(
            before, after,
            "same encoded {file} with a different original source"
        );
    }
    Ok(())
}

#[test]
fn derived_height_tracks_the_encrypted_stage_and_explicit_width_is_tool_authored() -> TestResult {
    let folder = tempfile::tempdir()?;
    let data = folder.path().join("data.p4k");
    let out = folder.path().join("panel.png");
    archive(&data, EncryptedInput::Movie)?;
    let report = render(&data, &out, false)?;
    assert_eq!(marker(&report, "/movie"), Some(false), "the directory name");
    assert_eq!(marker(&report, "/width"), Some(false), "the explicit width");
    assert_eq!(marker(&report, "/height"), Some(true), "the actual stage");
    assert_eq!(
        report
            .pointer("/artifactOrigin/containsEncryptedSource")
            .and_then(Value::as_bool),
        Some(true),
        "the drawing used the encrypted movie payload"
    );
    Ok(())
}

fn marker(summary: &Value, field: &str) -> Option<bool> {
    summary
        .get("fieldOrigins")?
        .get(field)?
        .get("containsEncryptedSource")?
        .as_bool()
}
