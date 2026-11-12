// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};

use p4kx_extraction::OriginDigestWriter;
use p4kx_screens::StillFormat;
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, GlobalIniBuilder, SplitDds, TestCanvas, TestDim, TestEntry,
    TestTextureFormat, TestValue, TestWidget, p4kx, screens_font_movie, screens_schema,
    screens_text_sample,
};
use serde_json::Value;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const CANVAS: &str = "libs/foundry/records/ui/buildingblocks/test/origin_panel";
const TEXT_CANVAS: &str = "libs/foundry/records/ui/buildingblocks/test/text_sample";
const TEXTURE: &str = "Data/UI/origins/panel.dds";
const SELECTED_ALPHA: &str = "Data/UI/origins/panel.dds.2a";
const UNUSED_ALPHA: &str = "Data/UI/origins/panel.dds.3a";
const CLEAR_VECTOR: &str = "Data/UI/origins/clear.svg";
const OFFSTAGE_VECTOR: &str = "Data/UI/origins/offstage.svg";
const FONT: &str = "Data/UI/fonts/Shared/fonts_en.gfx";
const INI: &str = "Data/Localization/english/global.ini";
const VECTOR: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><rect width="10" height="10" fill="#0000ff"/></svg>"##;

fn vector(name: &str, x: f32, path: &str) -> TestWidget {
    TestWidget::new("BuildingBlocks_WidgetCustomShape", name)
        .parent("root")
        .size(TestDim::Fixed(8.0), TestDim::Fixed(4.0))
        .offset(x, 4.0)
        .with("svgPath", TestValue::text(path))
        .with("renderShape", TestValue::Bool(true))
        .with("enableColorOverlay", TestValue::Bool(false))
}

fn image_source() -> Vec<u8> {
    let image = TestWidget::new("BuildingBlocks_WidgetImage", "panel")
        .parent("root")
        .size(TestDim::Fixed(8.0), TestDim::Fixed(4.0))
        .with("imagePath", TestValue::text("UI/origins/panel.tif"));
    let canvas = TestCanvas::new(0x82b4, "origin_panel", 16.0, 8.0)
        .raw()
        .widget(TestWidget::display("root"))
        .widget(image)
        .widget(vector("clear", 0.0, "UI/origins/clear.svg"))
        .widget(vector("offstage", 512.0, "UI/origins/offstage.svg"));
    screens_schema(DataForgeBuilder::new())
        .record(canvas.record())
        .build()
}

#[derive(Clone, Copy)]
enum EncryptedInput {
    NoSelected,
    Forge,
    Alpha,
}

fn image_archive(folder: &Path, encrypted_input: EncryptedInput) -> TestResult<PathBuf> {
    let path = folder.join("Data.p4k");
    let forge = TestEntry::zstd("Data/Game2.dcb", image_source());
    let mut builder = ArchiveBuilder::new()
        .entry(if matches!(encrypted_input, EncryptedInput::Forge) {
            forge.encrypted()
        } else {
            forge
        })
        .entry(TestEntry::stored(CLEAR_VECTOR, VECTOR.as_bytes().to_vec()))
        .entry(TestEntry::zstd(OFFSTAGE_VECTOR, VECTOR.as_bytes().to_vec()).encrypted())
        .entry(TestEntry::zstd(FONT, screens_font_movie()).encrypted())
        .entry(
            TestEntry::zstd(
                INI,
                GlobalIniBuilder::new().text("unused", "unused").build(),
            )
            .encrypted(),
        );
    let dds = SplitDds::new(TestTextureFormat::LegacyBgra8, 32, 16)
        .split(3)
        .alpha(TestTextureFormat::R8);
    for entry in dds.entries(TEXTURE) {
        let encrypted = entry.path() == UNUSED_ALPHA
            || (matches!(encrypted_input, EncryptedInput::Alpha) && entry.path() == SELECTED_ALPHA);
        builder = builder.entry(if encrypted { entry.encrypted() } else { entry });
    }
    builder.write_to(&path)?;
    Ok(path)
}

fn render(archive: &Path, canvas: &str, path: &Path, format: StillFormat) -> TestResult<Value> {
    let mut command = p4kx()?;
    command
        .args(["--json", "ui", "render"])
        .arg(archive)
        .args(["--screen", canvas, "--size", "32", "--out"])
        .arg(path);
    if format == StillFormat::Svg {
        command.arg("--svg");
    }
    let output = command.output()?;
    assert!(matches!(output.status.code(), Some(0 | 1)), "{output:?}");
    Ok(serde_json::from_slice(&output.stdout)?)
}

fn source_names(summary: &Value) -> TestResult<Vec<&str>> {
    let sources = summary
        .pointer("/provenance/sources")
        .and_then(Value::as_array)
        .ok_or("source inventory")?;
    Ok(sources
        .iter()
        .filter_map(|source| source.pointer("/path/value").and_then(Value::as_str))
        .collect())
}

fn marker(summary: &Value, path: &str) -> Option<bool> {
    summary
        .get("fieldOrigins")?
        .get(path)?
        .get("containsEncryptedSource")?
        .as_bool()
}

fn use_for<'a>(summary: &'a Value, namespace: &str, encrypted: bool) -> TestResult<Vec<&'a Value>> {
    let uses = summary
        .pointer("/provenance/uses")
        .and_then(Value::as_array)
        .ok_or("use inventory")?;
    let state = if encrypted {
        "encrypted"
    } else {
        "unencrypted"
    };
    Ok(uses
        .iter()
        .filter(|row| {
            row.pointer("/location/namespace").and_then(Value::as_str) == Some(namespace)
                && row.get("effectiveEncryption").and_then(Value::as_str) == Some(state)
        })
        .collect())
}

fn verify_artifact(folder: &Path, output: &Path, summary: &Value, encrypted: bool) -> TestResult {
    let name = summary
        .get("originReceipt")
        .and_then(Value::as_str)
        .ok_or("relative receipt name")?;
    let receipt = fs::read_to_string(folder.join(name))?;
    let rows: Vec<Value> = receipt
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()?;
    let header = rows.first().ok_or("receipt header")?;
    let artifact = fs::read(output)?;
    let mut measured = OriginDigestWriter::new(io::sink());
    measured.write_all(&artifact)?;
    let digest = serde_json::to_value(measured.finish()?)?;
    assert_eq!(
        header.pointer("/binding/sha256"),
        digest.get("sha256"),
        "digest of the committed bytes"
    );
    assert_eq!(
        header.pointer("/binding/byteLength"),
        digest.get("byteLength"),
        "measured byte length of the committed output",
    );
    assert_eq!(
        summary
            .pointer("/artifactOrigin/containsEncryptedSource")
            .and_then(Value::as_bool),
        Some(encrypted),
        "unsupported {:?}, skipped {:?}",
        summary.get("unsupported"),
        summary.get("skipped")
    );
    let field = rows
        .iter()
        .find(|row| row.get("type").and_then(Value::as_str) == Some("field"))
        .ok_or("opaque artifact field")?;
    for name in [
        "containsEncryptedSource",
        "sourceSetComplete",
        "sourceUsesRefs",
    ] {
        assert_eq!(
            field.get(name),
            summary.pointer(&format!("/artifactOrigin/{name}")),
            "the JSON and committed receipt agree on {name}",
        );
    }
    Ok(())
}

#[test]
fn screen_png_origin_contains_only_selected_dds_mips_and_visible_vector() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = image_archive(dir.path(), EncryptedInput::Alpha)?;
    let output = dir.path().join("screen.png");
    let report = render(&archive, CANVAS, &output, StillFormat::Png)?;
    verify_artifact(dir.path(), &output, &report, true)?;
    assert_eq!(marker(&report, "/width"), Some(false));
    assert_eq!(marker(&report, "/height"), Some(false));
    assert_eq!(marker(&report, "/screen"), Some(false));
    let names = source_names(&report)?;
    assert!(names.contains(&CLEAR_VECTOR), "painted SVG");
    assert!(names.contains(&SELECTED_ALPHA), "selected alpha mip");
    assert!(!names.contains(&UNUSED_ALPHA), "unused alpha mip");
    assert!(!names.contains(&OFFSTAGE_VECTOR), "no offstage PNG paint");
    let selected = use_for(&report, "ddsMipPayload", true)?;
    assert_eq!(selected.len(), 1);
    assert_eq!(
        selected
            .first()
            .and_then(|row| row.pointer("/location/indices/0"))
            .and_then(Value::as_str),
        Some("1"),
        "chosen mip, counted from the original DDS header"
    );
    Ok(())
}

#[test]
fn svg_can_embed_a_vector_that_png_does_not_paint() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = image_archive(dir.path(), EncryptedInput::NoSelected)?;
    let png = dir.path().join("screen.png");
    let raster = render(&archive, CANVAS, &png, StillFormat::Png)?;
    verify_artifact(dir.path(), &png, &raster, false)?;
    let svg = dir.path().join("screen.svg");
    let document = render(&archive, CANVAS, &svg, StillFormat::Svg)?;
    verify_artifact(dir.path(), &svg, &document, true)?;
    assert!(source_names(&document)?.contains(&OFFSTAGE_VECTOR));
    let raster_names = source_names(&raster)?;
    assert!(!raster_names.contains(&OFFSTAGE_VECTOR));
    assert!(
        !raster_names.contains(&FONT),
        "an eagerly loaded font movie drew no text"
    );
    assert!(
        !raster_names.contains(&INI),
        "an eagerly loaded language supplied no value"
    );
    Ok(())
}

#[test]
fn an_explicit_width_and_a_derived_height_have_different_origins() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = image_archive(dir.path(), EncryptedInput::Forge)?;
    let output = dir.path().join("screen.png");
    let report = render(&archive, CANVAS, &output, StillFormat::Png)?;
    verify_artifact(dir.path(), &output, &report, true)?;
    assert_eq!(marker(&report, "/width"), Some(false));
    assert_eq!(marker(&report, "/height"), Some(true));
    assert_eq!(marker(&report, "/target"), Some(false));
    assert_eq!(marker(&report, "/format"), Some(false));
    Ok(())
}

#[test]
fn screen_text_origin_uses_the_winning_encrypted_ini_value() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = dir.path().join("Data.p4k");
    let ini = GlobalIniBuilder::new()
        .text("test_hello", "obsolete")
        .text("test_hello", "AB")
        .build();
    ArchiveBuilder::new()
        .entry(TestEntry::zstd(
            "Data/Game2.dcb",
            screens_text_sample().build(),
        ))
        .entry(TestEntry::stored(FONT, screens_font_movie()))
        .entry(TestEntry::zstd(INI, ini).encrypted())
        .write_to(&archive)?;
    let output = dir.path().join("screen.png");
    let report = render(&archive, TEXT_CANVAS, &output, StillFormat::Png)?;
    verify_artifact(dir.path(), &output, &report, true)?;
    assert_eq!(marker(&report, "/height"), Some(false));
    assert!(source_names(&report)?.contains(&FONT));
    assert!(source_names(&report)?.contains(&INI));
    let values = use_for(&report, "globalIniBytes", true)?;
    assert_eq!(values.len(), 1, "only the used winning translation");
    assert_eq!(
        values
            .first()
            .and_then(|row| row.pointer("/location/indices/1"))
            .and_then(Value::as_str),
        Some("2"),
        "only the winning value occupies the recorded byte range"
    );
    Ok(())
}

#[test]
fn screen_text_retains_the_encrypted_font_and_the_measured_utf16_input() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = dir.path().join("Data.p4k");
    let mut ini = vec![0xff, 0xfe];
    ini.extend(
        "test_hello=AB\r\n"
            .encode_utf16()
            .flat_map(u16::to_le_bytes),
    );
    let original_length = ini.len().to_string();
    ArchiveBuilder::new()
        .entry(TestEntry::zstd(
            "Data/Game2.dcb",
            screens_text_sample().build(),
        ))
        .entry(TestEntry::zstd(FONT, screens_font_movie()).encrypted())
        .entry(TestEntry::stored(INI, ini))
        .write_to(&archive)?;
    let output = dir.path().join("screen.png");
    let report = render(&archive, TEXT_CANVAS, &output, StillFormat::Png)?;
    verify_artifact(dir.path(), &output, &report, true)?;
    assert_eq!(marker(&report, "/height"), Some(false));
    let fonts = use_for(&report, "gfxTag", true)?;
    assert_eq!(
        fonts
            .first()
            .and_then(|row| row.pointer("/location/field"))
            .and_then(Value::as_str),
        Some("screenFont")
    );
    let raw = use_for(&report, "globalIniInputBytes", false)?;
    assert_eq!(
        raw.first()
            .and_then(|row| row.pointer("/location/indices/1"))
            .and_then(Value::as_str),
        Some(original_length.as_str())
    );
    let decoded = use_for(&report, "globalIniDecodedBytes", false)?;
    assert_eq!(
        decoded
            .first()
            .and_then(|row| row.pointer("/location/indices/1"))
            .and_then(Value::as_str),
        Some("2")
    );
    Ok(())
}
