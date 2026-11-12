// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::path::Path;

use p4kx_extraction::{Archive, OpenOptions};
use p4kx_media::{ExportOptions, ExportOutcome, MediaKind, MediaRecord, Status};
use p4kx_testkit::{
    ArchiveBuilder, TestAnimation, TestChunkFile, TestDba, TestEntry, TestTimes, TestTrack,
    test_key,
};
use serde_json::{Value, json};

use super::support;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const STREAM: &str = "animation.jsonl";
const IDENTITY: [f32; 4] = [0.0, 0.0, 0.0, 1.0];
const FLAGS: u32 = 0x8000_0023;

#[test]
fn animation_exports_raw_and_decoded_data_in_the_shared_contract() -> TestResult {
    let temporary = tempfile::tempdir()?;
    let animation = TestAnimation::new("demo.caf", 30)
        .additive()
        .track(TestTrack::new(17))
        .track(
            TestTrack::new(17)
                .times(TestTimes::Bitmap)
                .rotations(&[(2, IDENTITY), (15, IDENTITY), (39, IDENTITY)])
                .positions(&[(3, [0.0, 2.0, 3.0]), (29, [1.0, 5.0, 9.0])]),
        );
    let mut metadata = animation.anim_info();
    metadata
        .get_mut(..4)
        .ok_or("flag field")?
        .copy_from_slice(&FLAGS.to_le_bytes());
    let caf = TestChunkFile::ivo()
        .chunk(0x4733_C6ED, 0x901, metadata.clone())
        .chunk(0xA949_6CB5, 0x900, animation.caf_data())
        .bytes();
    let dba = TestDba::new()
        .animation(animation)
        .animation(TestAnimation::new("another.caf", 60))
        .pool_after_metadata()
        .bytes();
    let aim = TestChunkFile::ivo()
        .chunk(0x0BAD_BEEF, 7, b"opaque")
        .bytes();
    let path = temporary.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("demo.caf", caf.as_slice()))
        .entry(TestEntry::zstd("demo.caf", caf.as_slice()).encrypted())
        .entry(TestEntry::stored("demo.dba", dba.as_slice()))
        .entry(TestEntry::stored("demo.aim", aim.as_slice()))
        .write_to(&path)?;
    let archive = Archive::open_with(path, OpenOptions::new().key(Some(test_key())))?;
    let output = temporary.path().join("out");
    let index = support::published(p4kx_media::export(&archive, &output, &options())?)?;
    let base = output.join(&index.artifact_base);
    let rows = support::records(&output, &index, STREAM)?;
    validate(&rows)?;
    check_inventory(&rows, &base, &metadata, [&caf, &caf, &dba, &aim])?;
    Ok(())
}

fn check_inventory(
    rows: &[MediaRecord],
    base: &Path,
    metadata: &[u8],
    originals: [&[u8]; 4],
) -> TestResult {
    for (owner, original) in originals.into_iter().enumerate() {
        let (_, entry) = find(rows, owner, "animation-entry", &[])?;
        let raw = entry.source.raw.as_ref().ok_or("retained source")?;
        assert_eq!(
            fs::read(base.join(&raw.path))?,
            original,
            "byte-perfect source {owner}"
        );
    }
    for owner in [0, 1] {
        let (_, clip) = find(rows, owner, "animation-clip", &[0])?;
        assert_value(&clip.data, "/payload/metadata_bytes", &json!(metadata));
        assert_value(&clip.data, "/payload/flags", &json!(FLAGS));
        assert_value(&clip.data, "/payload/timing/additive", &json!(true));
        let (_, empty) = find(rows, owner, "animation-controller", &[0, 0])?;
        assert_value(
            &empty.data,
            "/payload/rotation/samples/layout",
            &json!("empty"),
        );
        let (ordinal, controller) = find(rows, owner, "animation-controller", &[0, 1])?;
        check_samples(base, controller)?;
        check_sample_origin(base, ordinal, owner == 1)?;
    }
    let (_, clip) = find(rows, 2, "animation-clip", &[1])?;
    assert_value(
        &clip.data,
        "/payload/database_name/utf8",
        &json!("another.caf"),
    );
    let (_, aim) = find(rows, 3, "animation-entry", &[])?;
    assert_value(&aim.data, "/payload/layout", &json!("opaque-aim"));
    let (_, opaque) = find(rows, 3, "animation-chunk", &[0])?;
    assert_value(&opaque.data, "/payload/type_id", &json!(0x0BAD_BEEF));
    assert!(
        find(rows, 3, "animation-clip", &[0]).is_err(),
        "AIM remains opaque"
    );
    Ok(())
}

fn check_samples(base: &Path, record: &MediaRecord) -> TestResult {
    assert_value(&record.data, "/payload/controller_id", &json!(17));
    assert_value(
        &record.data,
        "/payload/rotation/packed_format",
        &json!(0x8242),
    );
    assert_value(
        &record.data,
        "/payload/translation/packed_format",
        &json!(0xC242),
    );
    for (side, frames) in [
        ("rotation", &[2_u16, 15, 39][..]),
        ("translation", &[3, 29][..]),
    ] {
        let prefix = format!("/payload/{side}/samples/frames");
        let path = record
            .data
            .pointer(&format!("{prefix}/artifact/path"))
            .and_then(Value::as_str)
            .ok_or("frame path")?;
        let bytes: Vec<_> = frames.iter().copied().flat_map(u16::to_le_bytes).collect();
        assert_eq!(
            fs::read(base.join(path))?,
            bytes,
            "original {side} frame numbers"
        );
    }
    let position = record
        .data
        .pointer("/payload/translation/samples/values/artifact/path")
        .and_then(Value::as_str)
        .ok_or("value path")?;
    let bytes = fs::read(base.join(position))?;
    let values = bytes
        .as_chunks::<4>()
        .0
        .iter()
        .copied()
        .map(f32::from_le_bytes);
    for (actual, expected) in values.zip([0.0, 2.0, 3.0, 1.0, 5.0, 9.0]) {
        assert!(
            (actual - expected).abs() < 1e-3,
            "decoded translation {actual} differs from {expected}"
        );
    }
    Ok(())
}

fn check_sample_origin(base: &Path, ordinal: usize, encrypted: bool) -> TestResult {
    let field = "/data/payload/rotation/samples/frames/artifact/digest/hex";
    let ordinal = ordinal.to_string();
    let data = fs::read_to_string(base.join("field-origins.jsonl"))?;
    let mut rows = data.lines().map(serde_json::from_str::<Value>);
    let receipt = rows
        .find(|row| {
            row.as_ref().is_ok_and(|row| {
                row.get("field").and_then(Value::as_str) == Some(field)
                    && row.pointer("/scope/stream/kind").and_then(Value::as_str)
                        == Some("animation")
                    && row.pointer("/scope/ordinal").and_then(Value::as_str)
                        == Some(ordinal.as_str())
            })
        })
        .ok_or("exact occurrence field receipt")??;
    assert_value(&receipt, "/containsEncryptedSource", &json!(encrypted));
    assert_value(&receipt, "/sourceSetComplete", &json!(true));
    let references = receipt
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("source uses")?;
    assert_eq!(
        references.len(),
        2,
        "encoded frame range and its descriptor"
    );
    Ok(())
}

#[test]
fn invalid_rotation_retains_decoded_translation_in_a_diagnostic_generation() -> TestResult {
    let temporary = tempfile::tempdir()?;
    let bad = TestAnimation::new("bad.caf", 30).track(
        TestTrack::new(9)
            .rotation_code(0x85)
            .rotations(&[(0, IDENTITY)])
            .positions(&[(2, [0.0; 3]), (5, [1.0, 2.0, 3.0])]),
    );
    let good = TestAnimation::new("good.caf", 60);
    let dba = TestDba::new().animation(bad).animation(good).bytes();
    let archive = support::archive_with(
        temporary.path(),
        [TestEntry::stored("source.dba", dba.as_slice())],
    )?;
    let output = temporary.path().join("out");
    let outcome = p4kx_media::export(
        &archive,
        &output,
        &ExportOptions {
            diagnostic: true,
            ..options()
        },
    )?;
    let ExportOutcome::Incomplete { diagnostic, .. } = outcome else {
        return Err("unsupported declared channel must not publish a complete generation".into());
    };
    let base = diagnostic.parent().ok_or("diagnostic generation base")?;
    let rows: Vec<_> = fs::read_to_string(base.join(STREAM))?
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()?;
    let (_, row) = find(&rows, 0, "animation-controller", &[0, 0])?;
    assert_eq!(
        row.status,
        Status::Partial,
        "declared rotation was not decoded"
    );
    assert_value(
        &row.data,
        "/payload/rotation/samples/layout",
        &json!("unsupported"),
    );
    assert_value(
        &row.data,
        "/payload/translation/samples/layout",
        &json!("decoded"),
    );
    assert!(
        find(&rows, 0, "animation-clip", &[1]).is_ok(),
        "the later clip survives"
    );
    validate(&rows)
}

fn options() -> ExportOptions {
    ExportOptions {
        kinds: vec![MediaKind::Animation],
        ..ExportOptions::default()
    }
}

fn find<'a>(
    rows: &'a [MediaRecord],
    owner: usize,
    namespace: &str,
    indices: &[u64],
) -> TestResult<(usize, &'a MediaRecord)> {
    rows.iter().enumerate().find(|(_, row)| {
        matches!(&row.source.identity, p4kx_media::Identity::Entry { entry, .. } if entry.entry_index == owner)
            && row.subresource.namespace == namespace
            && row.subresource.indices == indices
    }).ok_or_else(|| format!("missing {namespace} {owner} {indices:?}").into())
}

fn validate(rows: &[MediaRecord]) -> TestResult {
    let schema = jsonschema::validator_for(&p4kx_media::builtin_record_schema()?)?;
    for row in rows {
        schema
            .validate(&serde_json::to_value(row)?)
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn assert_value(value: &Value, path: &str, expected: &Value) {
    assert_eq!(
        value.pointer(path),
        Some(expected),
        "animation output at {path}"
    );
}

#[test]
fn a_configured_chunk_budget_never_publishes_an_incomplete_animation_index() -> TestResult {
    let temporary = tempfile::tempdir()?;
    let caf = TestAnimation::new("bounded.caf", 30)
        .track(TestTrack::new(5).rotations(&[(0, IDENTITY), (2, IDENTITY)]))
        .caf();
    let archive = support::archive_with(temporary.path(), [TestEntry::stored("bounded.caf", caf)])?;
    let output = temporary.path().join("out");
    let mut options = options();
    options.limits.chunk_bytes = 1;
    let Err(error) = p4kx_media::export(&archive, &output, &options) else {
        return Err("the declared animation chunk budget was exceeded".into());
    };
    assert!(
        super::required_input_support::has_limit(&error, "chunk_bytes"),
        "the source-bound error must retain the chunk limit: {error}"
    );
    assert!(
        !output.join("media-index.json").exists(),
        "no partial root index"
    );
    Ok(())
}
