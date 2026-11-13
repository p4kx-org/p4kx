// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::path::Path;

use p4kx_extraction::{Archive, Exclusion, Filter, OpenOptions};
use p4kx_media::{ExportOptions, ExportOutcome, Index, MediaKind, MediaRecord};
use p4kx_testkit::{
    ArchiveBuilder, SplitDds, TestEntry, TestTextureFormat, other_test_key, test_key,
};
use serde_json::{Value, json};

use crate::support;
use crate::support::audio_provider::{TestResult, read_rows};

const HEADER: &str = "Data/paired.dds";
const BGRA: TestTextureFormat = TestTextureFormat::LegacyBgra8;

#[test]
fn texture_conversion_uses_private_encrypted_alpha_and_exact_surface_proofs() -> TestResult {
    let folder = tempfile::tempdir()?;
    let archive = split_fixture(folder.path())?;
    let options = ExportOptions {
        kinds: vec![MediaKind::Texture],
        include: Some(Filter::parse("*paired.dds")?),
        exclude: Exclusion::parse(["*paired.dds.1a"])?,
        texture_mip: Some(0),
        ..ExportOptions::default()
    };
    let output = folder.path().join("out");
    let index = support::published(p4kx_media::export(&archive, &output, &options)?)?;
    let rows = support::records(&output, &index, "texture.jsonl")?;
    let base = output.join(&index.artifact_base);
    let (ordinal, row) = find(&rows, "texture.dds-image")?;
    let artifact = base.join(check_png(row)?);
    assert_eq!(
        fs::read(&artifact)?.get(..8),
        Some(b"\x89PNG\r\n\x1a\n".as_slice()),
        "actual PNG file"
    );
    assert_eq!(
        fs::read(&artifact)?.get(37..41),
        Some(b"sRGB".as_slice()),
        "real PNG sRGB chunk after IHDR"
    );
    let sources: Vec<Value> = read_rows(base.join("sources.jsonl"))?;
    assert!(
        sources
            .iter()
            .all(|row| row.pointer("/identity/entry/path") != Some(&json!("Data/paired.dds.1a"))),
        "excluded alpha remains absent from public raw sources"
    );
    check_split_origins(&base, ordinal)?;
    let validator = jsonschema::validator_for(&p4kx_media::builtin_record_schema()?)?;
    for row in rows {
        validator
            .validate(&serde_json::to_value(row)?)
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn split_fixture(root: &Path) -> TestResult<Archive> {
    let color = SplitDds::new(BGRA, 4, 4)
        .split(1)
        .alpha(TestTextureFormat::R8);
    let alpha = color.alpha_tail().ok_or("fixture alpha")?;
    let path = root.join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(HEADER, cry_srgb(color.header_bytes())?))
        .entry(TestEntry::stored("Data/paired.dds.1", color.part(1)))
        .entry(TestEntry::stored("Data/paired.dds.a", alpha.header_bytes()))
        .entry(TestEntry::stored("Data/paired.dds.1a", alpha.part(1)).encrypted())
        .write_to(&path)?;
    Ok(Archive::open_with(
        path,
        OpenOptions::new().key(Some(test_key())),
    )?)
}

fn cry_srgb(mut header: Vec<u8>) -> TestResult<Vec<u8>> {
    header
        .get_mut(124..128)
        .ok_or("Cry marker")?
        .copy_from_slice(b"FYRC");
    header
        .get_mut(36..40)
        .ok_or("Cry flags")?
        .copy_from_slice(&0x800_u32.to_le_bytes());
    Ok(header)
}

fn check_png(row: &MediaRecord) -> TestResult<&str> {
    let payload = row.data.pointer("/payload").ok_or("image payload")?;
    for (field, value) in [
        ("level", json!(0)),
        ("width", json!(4)),
        ("height", json!(4)),
        ("alpha", json!("decoded-split-channel")),
        ("format", json!("png")),
        ("source_transfer", json!("srgb")),
        ("output_transfer", json!("srgb")),
        ("png_transform", json!("preserve_colour_transfer")),
        ("bit_depth", json!(8)),
        ("placement", json!("written")),
    ] {
        assert_eq!(payload.get(field), Some(&value), "image field {field}");
    }
    payload
        .pointer("/artifact/path")
        .and_then(Value::as_str)
        .ok_or_else(|| "image path".into())
}

fn check_split_origins(base: &Path, ordinal: usize) -> TestResult {
    let fields: Vec<Value> = read_rows(base.join("field-origins.jsonl"))?;
    let uses: Vec<Value> = read_rows(base.join("origin-uses.jsonl"))?;
    let artifact = field(&fields, ordinal, "/data/payload/artifact/digest/hex")?;
    assert_eq!(
        artifact.get("containsEncryptedSource"),
        Some(&json!(true)),
        "private used alpha bytes"
    );
    assert_eq!(
        artifact.get("sourceSetComplete"),
        Some(&json!(true)),
        "both original channels proved"
    );
    let width = field(&fields, ordinal, "/data/payload/width")?;
    assert_eq!(
        width.get("containsEncryptedSource"),
        Some(&json!(false)),
        "width only uses the color header"
    );
    for path in ["source_transfer", "output_transfer", "png_transform"] {
        let field = field(&fields, ordinal, &format!("/data/payload/{path}"))?;
        assert_eq!(
            field.get("containsEncryptedSource"),
            Some(&json!(false)),
            "texture transfer proof uses plain colour header"
        );
    }
    let transfer = field(&fields, ordinal, "/data/payload/output_transfer")?;
    let references = transfer
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("transfer source refs")?;
    assert!(
        uses.iter().any(|row| {
            row.get("id").is_some_and(|id| references.contains(id))
                && row.pointer("/location/field") == Some(&json!("reserved1"))
                && row.pointer("/location/indices/0") == Some(&json!("36"))
        }),
        "output transfer cites the original Cry flags"
    );
    let references = artifact
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("artifact source refs")?;
    let span_count = uses
        .iter()
        .filter(|row| {
            row.get("id").is_some_and(|id| references.contains(id))
                && row.pointer("/location/namespace") == Some(&json!("ddsMipPayload"))
        })
        .count();
    assert_eq!(span_count, 2, "only one color and one alpha mip contribute");
    Ok(())
}

fn field<'a>(fields: &'a [Value], ordinal: usize, path: &str) -> TestResult<&'a Value> {
    let ordinal = ordinal.to_string();
    fields
        .iter()
        .find(|row| {
            row.get("field").and_then(Value::as_str) == Some(path)
                && row.pointer("/scope/stream/kind").and_then(Value::as_str) == Some("texture")
                && row.pointer("/scope/ordinal").and_then(Value::as_str) == Some(ordinal.as_str())
        })
        .ok_or_else(|| format!("missing conversion origin {ordinal} {path}").into())
}

fn find<'a>(rows: &'a [MediaRecord], namespace: &str) -> TestResult<(usize, &'a MediaRecord)> {
    rows.iter()
        .enumerate()
        .find(|(_, row)| row.subresource.namespace == namespace)
        .ok_or_else(|| format!("missing {namespace}").into())
}

#[test]
fn texture_conversion_diagnoses_duplicate_missing_and_key_required_parts() -> TestResult {
    let color = SplitDds::new(BGRA, 4, 4).split(1);
    for (entries, expected) in [
        (
            vec![
                TestEntry::stored("Data/paired.dds.1", color.part(1)),
                TestEntry::stored("DATA/PAIRED.DDS.1", color.part(1)),
            ],
            "ambiguous",
        ),
        (Vec::new(), "missing"),
        (
            vec![TestEntry::stored("Data/paired.dds.1", color.part(1)).encrypted()],
            "key-required",
        ),
        (
            vec![TestEntry::stored("Data/paired.dds.1", b"short")],
            "inconsistent-layout",
        ),
    ] {
        let folder = tempfile::tempdir()?;
        let archive = support::archive_with(
            folder.path(),
            std::iter::once(TestEntry::stored(HEADER, color.header_bytes())).chain(entries),
        )?;
        let options = ExportOptions {
            kinds: vec![MediaKind::Texture],
            include: Some(Filter::parse("*paired.dds")?),
            diagnostic: true,
            texture_mip: Some(0),
            ..ExportOptions::default()
        };
        let output = folder.path().join("out");
        let ExportOutcome::Incomplete { index, diagnostic } =
            p4kx_media::export(&archive, &output, &options)?
        else {
            return Err("invalid required input produced a complete generation".into());
        };
        check_failure(
            diagnostic.parent().ok_or("diagnostic directory")?,
            &index,
            expected,
        )?;
    }
    Ok(())
}

fn check_failure(output: &Path, index: &Index, expected: &str) -> TestResult {
    let rows = support::records(output, index, "texture.jsonl")?;
    let (_, row) = find(&rows, "texture.dds-conversion")?;
    let failure = row
        .data
        .pointer("/payload/outcome/failure")
        .ok_or("conversion failure")?;
    let disposition = failure
        .pointer("/disposition/status")
        .or_else(|| failure.get("type"));
    assert_eq!(
        disposition,
        Some(&json!(expected)),
        "exact required-input refusal"
    );
    assert!(
        find(&rows, "texture.dds-image").is_err(),
        "no invented artifact"
    );
    Ok(())
}

#[test]
fn texture_conversion_keeps_wrong_key_distinct_from_a_bad_payload() -> TestResult {
    for wrong_key in [true, false] {
        let folder = tempfile::tempdir()?;
        let color = SplitDds::new(BGRA, 4, 4).split(1);
        let target = "Data/paired.dds.1";
        let bytes = color.part(1);
        let part = if wrong_key {
            TestEntry::stored(target, bytes).encrypted()
        } else {
            TestEntry::stored(target, bytes).with_crc32c(1)
        };
        let path = folder.path().join("Data.p4k");
        ArchiveBuilder::new()
            .entry(TestEntry::stored(HEADER, cry_srgb(color.header_bytes())?))
            .entry(part)
            .write_to(&path)?;
        let archive = Archive::open_with(path, OpenOptions::new().key(Some(other_test_key())))?;
        let options = ExportOptions {
            kinds: vec![MediaKind::Texture],
            include: Some(Filter::parse("*paired.dds")?),
            diagnostic: true,
            texture_mip: Some(0),
            ..ExportOptions::default()
        };
        let output = folder.path().join("out");
        let ExportOutcome::Incomplete { index, diagnostic } =
            p4kx_media::export(&archive, &output, &options)?
        else {
            return Err(format!(
                "an unreadable part produced a complete generation (wrong key: {wrong_key})"
            )
            .into());
        };
        let expected = if wrong_key { "wrong-key" } else { "unreadable" };
        check_failure(
            diagnostic.parent().ok_or("diagnostic directory")?,
            &index,
            expected,
        )?;
    }
    Ok(())
}
