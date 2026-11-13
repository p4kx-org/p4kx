// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::path::Path;

use p4kx_extraction::model::{CafImage, CafImageError, CafImagePath};
use p4kx_extraction::{Archive, Filter, OpenOptions};
use p4kx_media::{ExportOptions, ExportOutcome, MediaKind, MediaRecord, Status};
use p4kx_testkit::{ArchiveBuilder, TestAnimation, TestChunkFile, TestEntry, test_key};
use serde_json::{Value, json};

use super::support;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const IMAGE: &str = "Data/Animations/Animations.img";
const DIRECTIONAL: &str = "Data/Animations/DirectionalBlends.img";
const CAF_CHUNK: u32 = 0x3702_75B7;
const DIRECTIONAL_CHUNK: u32 = 0x6B8D_F259;
const PATH_BYTES: usize = 256;
const RECORD_BYTES: usize = 308;

#[test]
fn caf_image_cursor_preserves_occurrences_and_invalid_lexemes() -> TestResult {
    let path = field(b"animations/gear/demo.caf");
    let bad_utf8 = field(&[0xff]);
    let fixture = image(&[path, path, bad_utf8, [b'x'; PATH_BYTES]])?;
    let image = CafImage::read(&fixture, 32)?;
    assert_eq!(image.header_range(), 32..40);
    assert_eq!(image.record_count(), 4);
    assert_eq!(image.trailer_range(), 1272..1280);
    let actual: Vec<_> = image
        .records()
        .map(|record| (record.range(), record.path_field_range(), record.path()))
        .collect();
    assert_eq!(
        actual,
        [
            (
                40..348,
                40..296,
                CafImagePath::Text("animations/gear/demo.caf")
            ),
            (
                348..656,
                348..604,
                CafImagePath::Text("animations/gear/demo.caf")
            ),
            (656..964, 656..912, CafImagePath::InvalidEncoding),
            (964..1272, 964..1220, CafImagePath::Unterminated),
        ],
    );
    Ok(())
}

#[test]
fn caf_image_rejects_unproved_extents_and_accepts_empty_tables() -> TestResult {
    let mut fixture = image(&[])?;
    assert_eq!(CafImage::read(&fixture, 0)?.records().len(), 0);
    fixture.push(1);
    assert!(matches!(
        CafImage::read(&fixture, 0),
        Err(CafImageError::TrailingBytes(1))
    ));
    assert!(matches!(
        CafImage::read(fixture.get(..7).ok_or("short prefix fixture")?, 0),
        Err(CafImageError::Truncated(_))
    ));
    fixture
        .get_mut(..4)
        .ok_or("fixture count")?
        .copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(matches!(
        CafImage::read(&fixture, 0),
        Err(CafImageError::Truncated(_) | CafImageError::Extent)
    ));
    fixture
        .get_mut(4..8)
        .ok_or("fixture format word")?
        .copy_from_slice(&9_u32.to_le_bytes());
    assert!(matches!(
        CafImage::read(&fixture, 0),
        Err(CafImageError::UnsupportedFormat(9))
    ));
    assert!(matches!(
        CafImage::read(&fixture, u64::MAX),
        Err(CafImageError::Extent)
    ));
    Ok(())
}

#[test]
fn animation_image_export_keeps_source_bytes_links_and_opaque_directional_data() -> TestResult {
    let temporary = tempfile::tempdir()?;
    let fixture = image(&[
        field(b"Animations\\Gear\\Demo.CAF"),
        field(b"Data/Animations/Gear/Demo.caf"),
        field(b"Animations/Gear/missing.caf"),
        field(&[0xff]),
    ])?;
    let image = TestChunkFile::ivo()
        .chunk(CAF_CHUNK, 0x902, fixture)
        .bytes();
    let directional = TestChunkFile::ivo()
        .chunk(DIRECTIONAL_CHUNK, 0x901, b"unproved")
        .bytes();
    let archive = archive(temporary.path(), &image, &directional)?;
    let root = temporary.path().join("out");
    let index = support::published(p4kx_media::export(&archive, &root, &options())?)?;
    let base = root.join(&index.artifact_base);
    let rows = support::records(&root, &index, "animation.jsonl")?;
    validate_rows(&rows)?;
    check_layouts(&rows)?;
    let records: Vec<_> = rows
        .iter()
        .enumerate()
        .filter(|(_, row)| row.subresource.namespace == "animation-image-record")
        .collect();
    assert_eq!(records.len(), 4);
    check_paths(&records)?;
    check_origin(&base, records.first().ok_or("first CAF image record")?.0)?;
    let raw = rows
        .iter()
        .find(|row| {
            row.subresource.namespace == "animation-entry"
                && row.data.pointer("/payload/family").and_then(Value::as_str) == Some("caf-image")
        })
        .and_then(|row| row.source.raw.as_ref())
        .ok_or("original image")?;
    assert_eq!(
        fs::read(base.join(&raw.path))?,
        image,
        "raw encrypted image is recoverable"
    );
    Ok(())
}

#[test]
fn animation_image_lookup_preserves_filtered_target_identity() -> TestResult {
    let temporary = tempfile::tempdir()?;
    let caf = image(&[field(b"Animations/gear/demo.caf")])?;
    let image = TestChunkFile::ivo().chunk(CAF_CHUNK, 0x902, caf).bytes();
    let directional = TestChunkFile::ivo()
        .chunk(DIRECTIONAL_CHUNK, 0x901, b"opaque")
        .bytes();
    let archive = archive(temporary.path(), &image, &directional)?;
    let root = temporary.path().join("out");
    let options = ExportOptions {
        include: Some(Filter::parse(IMAGE)?),
        ..options()
    };
    let index = support::published(p4kx_media::export(&archive, &root, &options)?)?;
    let rows = support::records(&root, &index, "animation.jsonl")?;
    let row = rows
        .iter()
        .find(|row| row.subresource.namespace == "animation-image-record")
        .ok_or("image record")?;
    assert_eq!(
        row.data
            .pointer("/payload/path/loose_entry/resolution/status"),
        Some(&json!("not-selected")),
        "the exact target is recognizable outside selection"
    );
    Ok(())
}

#[test]
fn animation_image_future_and_invalid_chunks_keep_original_diagnostic_bytes() -> TestResult {
    let temporary = tempfile::tempdir()?;
    let valid = image(&[])?;
    let malformed = valid.get(..7).ok_or("short prefix")?;
    let fixture = TestChunkFile::ivo()
        .chunk(CAF_CHUNK, 0x902, malformed)
        .chunk(CAF_CHUNK, 0x903, valid)
        .bytes();
    let directional = TestChunkFile::ivo()
        .chunk(DIRECTIONAL_CHUNK, 0x901, b"opaque")
        .bytes();
    let archive = archive(temporary.path(), &fixture, &directional)?;
    let root = temporary.path().join("out");
    let options = ExportOptions {
        diagnostic: true,
        ..options()
    };
    let ExportOutcome::Incomplete { diagnostic, .. } =
        p4kx_media::export(&archive, &root, &options)?
    else {
        return Err("image framing problems must retain a diagnostic generation".into());
    };
    let base = diagnostic.parent().ok_or("diagnostic base")?;
    let text = fs::read_to_string(base.join("animation.jsonl"))?;
    let rows = text
        .lines()
        .map(serde_json::from_str::<MediaRecord>)
        .collect::<Result<Vec<_>, _>>()?;
    let statuses: Vec<_> = rows
        .iter()
        .filter(|row| row.subresource.namespace == "animation-image-index")
        .map(|row| (row.status, row.data.pointer("/payload/layout/status")))
        .collect();
    assert_eq!(
        statuses,
        [
            (Status::Partial, Some(&json!("invalid-framing"))),
            (Status::Partial, Some(&json!("unsupported-version"))),
            (Status::Complete, Some(&json!("directional-opaque"))),
        ],
        "malformed and future layouts remain distinct"
    );
    let raw = rows
        .iter()
        .find(|row| row.subresource.namespace == "animation-image-index")
        .and_then(|row| row.source.raw.as_ref())
        .ok_or("diagnostic source raw bytes")?;
    assert_eq!(
        fs::read(base.join(&raw.path))?,
        fixture,
        "diagnostics preserve exact image bytes"
    );
    Ok(())
}

fn field(path: &[u8]) -> [u8; PATH_BYTES] {
    let mut field = [0; PATH_BYTES];
    assert!(
        path.len() < PATH_BYTES,
        "path fits test field with its terminator"
    );
    for (output, byte) in field.iter_mut().zip(path) {
        *output = *byte;
    }
    field
}

fn image<const COUNT: usize>(fields: &[[u8; PATH_BYTES]; COUNT]) -> TestResult<Vec<u8>> {
    let mut bytes = Vec::new();
    bytes.extend(u32::try_from(COUNT)?.to_le_bytes());
    bytes.extend(2_u32.to_le_bytes());
    for path in fields {
        bytes.extend(path);
        bytes.extend([0xA5; RECORD_BYTES - PATH_BYTES]);
    }
    bytes.extend(*b"raw-tail");
    Ok(bytes)
}

fn archive(directory: &Path, image: &[u8], directional: &[u8]) -> TestResult<Archive> {
    let path = directory.join("Data.p4k");
    let clip = TestAnimation::new("demo.caf", 30).caf();
    ArchiveBuilder::new()
        .entry(TestEntry::stored(IMAGE, image).encrypted())
        .entry(TestEntry::stored(DIRECTIONAL, directional))
        .entry(TestEntry::stored("Other/Animations.img", directional))
        .entry(TestEntry::stored("Data/Animations/Gear/demo.caf", clip))
        .write_to(&path)?;
    Ok(Archive::open_with(
        path,
        OpenOptions::new().key(Some(test_key())),
    )?)
}

fn options() -> ExportOptions {
    ExportOptions {
        kinds: vec![MediaKind::Animation],
        ..Default::default()
    }
}

fn validate_rows(rows: &[MediaRecord]) -> TestResult {
    let schema: Value = serde_json::from_str(include_str!(
        "../../../../schemas/media-builtin-record.schema.json"
    ))?;
    let validator = jsonschema::validator_for(&schema)?;
    for row in rows {
        let row = serde_json::to_value(row)?;
        assert!(
            validator.is_valid(&row),
            "media row does not match its schema: {row}"
        );
    }
    Ok(())
}

fn check_layouts(rows: &[MediaRecord]) -> TestResult {
    let layouts: Vec<_> = rows
        .iter()
        .filter(|row| row.subresource.namespace == "animation-image-index")
        .collect();
    let [caf, directional] = layouts.as_slice() else {
        return Err("both global images are present".into());
    };
    assert_eq!(
        caf.data.pointer("/payload/layout/status"),
        Some(&json!("caf-records")),
        "the exact CAF image is recognized"
    );
    assert_eq!(
        caf.data.pointer("/payload/layout/record_count"),
        Some(&json!(4)),
        "every declared physical CAF occurrence is retained"
    );
    assert_eq!(
        directional.data.pointer("/payload/layout/status"),
        Some(&json!("directional-opaque")),
        "the directional layout remains explicit and opaque"
    );
    Ok(())
}

fn check_paths(records: &[(usize, &MediaRecord)]) -> TestResult {
    for (ordinal, (_, row)) in records.iter().enumerate() {
        assert_eq!(
            row.subresource.indices,
            vec![0, ordinal as u64],
            "original occurrence ordinal"
        );
    }
    let fields: Vec<_> = records.iter().map(|(_, row)| &row.data).collect();
    let [first, second, missing, invalid] = fields.as_slice() else {
        return Err("CAF image occurrence count differs".into());
    };
    for field in [first, second] {
        assert_eq!(
            field.pointer("/payload/path/loose_entry/resolution/status"),
            Some(&json!("resolved")),
            "normalized paths resolve to the same loose entry"
        );
    }
    assert_eq!(
        missing.pointer("/payload/path/loose_entry/resolution/status"),
        Some(&json!("missing")),
        "the directory proves no loose entry"
    );
    assert_eq!(
        invalid.pointer("/payload/path/status"),
        Some(&json!("invalid-encoding")),
        "invalid text retains its physical row"
    );
    Ok(())
}

fn check_origin(base: &Path, ordinal: usize) -> TestResult {
    let text = fs::read_to_string(base.join("field-origins.jsonl"))?;
    let ordinal = ordinal.to_string();
    let mut encrypted = false;
    let mut selected_from_archive = false;
    for row in text.lines().map(serde_json::from_str::<Value>) {
        let row = row?;
        if row.pointer("/scope/stream/kind").and_then(Value::as_str) != Some("animation")
            || row.pointer("/scope/ordinal").and_then(Value::as_str) != Some(ordinal.as_str())
        {
            continue;
        }
        match row.get("field").and_then(Value::as_str) {
            Some("/data/payload/path/authored") => {
                encrypted =
                    row.get("containsEncryptedSource").and_then(Value::as_bool) == Some(true);
            }
            Some("/data/payload/path/loose_entry/resolution/target/source/entry/path") => {
                selected_from_archive = row.get("containsEncryptedSource").and_then(Value::as_bool)
                    == Some(true)
                    && row.get("sourceSetComplete").and_then(Value::as_bool) == Some(true)
                    && row
                        .get("sourceUsesRefs")
                        .and_then(Value::as_array)
                        .is_some_and(|refs| refs.len() > 1);
            }
            _ => {}
        }
    }
    assert!(encrypted, "the CAF path comes from encrypted payload");
    assert!(
        selected_from_archive,
        "target identity uses the encrypted selector and archive metadata"
    );
    Ok(())
}
