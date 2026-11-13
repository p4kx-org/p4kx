// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::{Archive, OpenOptions};
use p4kx_media::{ExportOptions, MediaKind, MediaRecord, Status};
use p4kx_testkit::{ArchiveBuilder, SplitDds, TestEntry, TestTextureFormat, test_key};
use serde_json::{Value, json};

use crate::support;
use crate::support::audio_provider::{TestResult, read_rows};

const FORMAT: TestTextureFormat = TestTextureFormat::Dx10Pixel {
    dxgi_format: 28,
    pixel_bytes: 4,
};

#[test]
fn default_provider_retains_authored_words_and_exact_field_owners() -> TestResult {
    let root = tempfile::tempdir()?;
    let fixture = SplitDds::new(FORMAT, 8, 4).array(2).split(1);
    let mut header = fixture.header_bytes();
    set_word(&mut header, 32, u32::MAX)?;
    set_word(&mut header, 144, 0xdead_beef)?;
    let archive = fixture_archive(
        root.path(),
        [
            TestEntry::stored("Data/array.dds", header.as_slice()).encrypted(),
            TestEntry::stored("Data/array.dds.1", fixture.mip(0)),
            TestEntry::stored("Data/array.dds.a", fixture.header_bytes()),
        ],
    )?;
    let output = root.path().join("out");
    let index = support::published(p4kx_media::export(&archive, &output, &options(false))?)?;
    let rows = support::records(&output, &index, "texture.jsonl")?;
    let [color, alpha] = rows.as_slice() else {
        return Err("exactly two selected DDS header rows".into());
    };
    verify_rows(color, alpha)?;
    let base = output.join(&index.artifact_base);
    verify_origins(&base)?;
    verify_schema(color)
}

#[test]
fn malformed_header_retains_raw_words_only_in_diagnostic_output() -> TestResult {
    let root = tempfile::tempdir()?;
    let mut bytes = SplitDds::new(FORMAT, 8, 4).header_bytes();
    set_word(&mut bytes, 16, 0)?;
    let archive = fixture_archive(
        root.path(),
        [
            TestEntry::stored("Data/invalid.dds", bytes),
            TestEntry::stored("Data/missing.dds", b"DDS "),
        ],
    )?;
    let output = root.path().join("out");
    let error = p4kx_media::export(&archive, &output, &options(false))
        .expect_err("ordinary export refuses malformed DDS topology");
    assert!(
        matches!(&error, p4kx_media::MediaError::Source { cause, .. }
            if matches!(cause.as_ref(), p4kx_media::MediaError::ProjectionLoss { kind: MediaKind::Texture, .. })),
        "malformed DDS produces a typed texture failure: {error}"
    );

    let p4kx_media::ExportOutcome::Incomplete { index, diagnostic } =
        p4kx_media::export(&archive, &output, &options(true))?
    else {
        return Err("expected a diagnostic generation".into());
    };
    let directory = diagnostic.parent().ok_or("diagnostic directory")?;
    let rows = support::records(directory, &index, "texture.jsonl")?;
    assert!(
        !output.join("media-index.json").exists(),
        "diagnostics do not replace a complete index"
    );

    let [invalid, truncated] = rows.as_slice() else {
        return Err("both malformed sources have diagnostic rows".into());
    };
    assert_eq!(invalid.status, Status::Partial, "normalization failed");
    assert_eq!(
        invalid.data.pointer("/payload/raw/legacy/width"),
        Some(&json!(0)),
        "raw width survives"
    );
    assert_eq!(
        truncated.data.pointer("/payload/raw"),
        Some(&Value::Null),
        "no fabricated incomplete raw words"
    );
    verify_schema(invalid)
}

fn options(diagnostic: bool) -> ExportOptions {
    ExportOptions {
        kinds: vec![MediaKind::Texture],
        diagnostic,
        ..ExportOptions::default()
    }
}

fn fixture_archive(
    root: &Path,
    entries: impl IntoIterator<Item = TestEntry>,
) -> TestResult<Archive> {
    let path = root.join("synthetic.p4k");
    entries
        .into_iter()
        .fold(ArchiveBuilder::new(), ArchiveBuilder::entry)
        .write_to(&path)?;
    Ok(Archive::open_with(
        path,
        OpenOptions::new().key(Some(test_key())),
    )?)
}

fn verify_rows(color: &MediaRecord, alpha: &MediaRecord) -> TestResult {
    assert_eq!(color.status, Status::Complete, "source metadata projected");
    for (path, expected) in [
        ("/payload/alpha_tail", json!(false)),
        ("/payload/decoder", json!("png")),
        ("/payload/split_parts/expected_count", json!(1)),
        (
            "/payload/source_metadata/raw/legacy/reserved1/0",
            json!(u32::MAX),
        ),
        (
            "/payload/source_metadata/raw/dx10/misc_flags2",
            json!(0xdead_beef_u32),
        ),
        ("/payload/source_metadata/normalized/array_size", json!(2)),
    ] {
        assert_eq!(
            color.data.pointer(path),
            Some(&expected),
            "header data at {path}"
        );
    }
    assert_eq!(
        alpha.data.pointer("/payload/alpha_tail"),
        Some(&json!(true)),
        "alpha header"
    );
    let raw = color
        .source
        .raw
        .as_ref()
        .ok_or("exact source bytes retained")?;
    assert_eq!(
        color
            .data
            .pointer("/payload/source_metadata/header_source_bytes"),
        Some(&json!(raw.size.to_string())),
        "shared lossless DDS size"
    );
    Ok(())
}

fn verify_origins(base: &Path) -> TestResult {
    let fields: Vec<Value> = read_rows(base.join("field-origins.jsonl"))?;
    let uses: Vec<Value> = read_rows(base.join("origin-uses.jsonl"))?;
    let sources: Vec<Value> = read_rows(base.join("origin-sources.jsonl"))?;
    let value = "/data/payload/source_metadata/raw/dx10/misc_flags2";
    for (ordinal, encrypted, entry) in [("0", true, 0), ("1", false, 2)] {
        let field = fields
            .iter()
            .find(|row| {
                row.get("field").and_then(Value::as_str) == Some(value)
                    && row.pointer("/scope/ordinal").and_then(Value::as_str) == Some(ordinal)
                    && row.pointer("/scope/stream/kind").and_then(Value::as_str) == Some("texture")
            })
            .ok_or("immediate field origin")?;
        assert_eq!(
            field.get("containsEncryptedSource"),
            Some(&json!(encrypted)),
            "each header supplies its own words"
        );
        assert_eq!(
            field.get("sourceSetComplete"),
            Some(&json!(true)),
            "complete physical source evidence"
        );
        let references = field
            .get("sourceUsesRefs")
            .and_then(Value::as_array)
            .ok_or("source refs")?;
        check_uses(references, &uses, &sources, entry)?;
    }
    Ok(())
}

fn check_uses(references: &[Value], uses: &[Value], sources: &[Value], entry: u32) -> TestResult {
    let mut exact_word = false;
    for id in references {
        let usage = uses
            .iter()
            .find(|row| row.get("id") == Some(id))
            .ok_or("source use")?;
        let owner_id = usage.get("source").ok_or("source owner id")?;
        let owner = sources
            .iter()
            .find(|row| row.get("id") == Some(owner_id))
            .ok_or("physical owner")?;
        assert_eq!(
            owner.get("entryIndex"),
            Some(&json!(entry)),
            "no unrelated sibling contributes"
        );
        exact_word |= usage.pointer("/location/indices") == Some(&json!(["144", "4"]))
            && usage.get("role").and_then(Value::as_str) == Some("payload");
    }
    assert!(exact_word, "exact miscFlags2 byte range");
    Ok(())
}

fn set_word(bytes: &mut [u8], offset: usize, word: u32) -> TestResult {
    bytes
        .get_mut(offset..offset + 4)
        .ok_or("fixture header word")?
        .copy_from_slice(&word.to_le_bytes());
    Ok(())
}

fn verify_schema(row: &MediaRecord) -> TestResult {
    let validator = jsonschema::validator_for(&p4kx_media::builtin_record_schema()?)?;
    let json = serde_json::to_value(row)?;
    validator
        .validate(&json)
        .map_err(|error| error.to_string())?;
    let mut tampered = json;
    *tampered
        .pointer_mut("/subresource/indices")
        .ok_or("header indices")? = json!([1]);
    assert!(
        !validator.is_valid(&tampered),
        "header records have no indices"
    );
    Ok(())
}
