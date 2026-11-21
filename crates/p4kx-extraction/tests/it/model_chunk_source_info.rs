// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_extraction::chunks::ChunkFile;
use p4kx_extraction::model::{
    CompanionError, CompanionPath, LodSelection, MaterialReading, Model, PositionBox,
};
use p4kx_extraction::{ChunkInfo, ChunkJson, DecimalU64, ModelChunkSourceInfo};
use p4kx_testkit::TestChunkFile;
use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn Error>>;
const UNKNOWN_IVO: u32 = 0xDEAD_BEEF;
const UNKNOWN_CRCH: u32 = 0xABCD;

fn read(file: &ChunkFile<Vec<u8>>) -> Result<Model, Box<dyn Error>> {
    let mut companions =
        |_: CompanionPath<'_>| -> Result<Option<Vec<u8>>, CompanionError> { Ok(None) };
    Ok(Model::read_with_materials(
        "source.cgf",
        file,
        &mut companions,
        LodSelection::All,
        PositionBox::default(),
        MaterialReading::Skipped,
    )?)
}

fn unknown_ivo() -> Result<Model, Box<dyn Error>> {
    let file = ChunkFile::parse(
        TestChunkFile::ivo()
            .chunk(UNKNOWN_IVO, u32::MAX, b"abc".to_vec())
            .bytes(),
    )?;
    read(&file)
}

fn projected_ivo() -> Result<Value, Box<dyn Error>> {
    let model = unknown_ivo()?;
    let chunk = model.chunks.first().ok_or("source chunk")?;
    Ok(serde_json::to_value(ModelChunkSourceInfo::of(chunk))?)
}

#[test]
fn source_projection_keeps_unknown_facts_after_the_parser_input_is_dropped() -> TestResult {
    let value = projected_ivo()?;
    assert_eq!(
        value,
        json!({
            "schema_version": 1,
            "chunk": {"file": "model", "index": 0, "type": null,
                "type_id": UNKNOWN_IVO, "usage": "unknown"},
            "layout": {"container": "#ivo", "version": u32::MAX,
                "id": null, "byte_order": "little", "offset": "32",
                "span": {"basis": "derived-ivo-next-boundary-including-padding", "bytes": "3"}}
        }),
        "actual retained facts, without a semantic payload decoder"
    );
    let schema = schemars::schema_for!(ModelChunkSourceInfo).to_value();
    assert!(
        jsonschema::validator_for(&schema)?.is_valid(&value),
        "actual source DTO validates"
    );
    Ok(())
}

#[test]
fn crch_source_projection_separates_version_flag_and_declared_span() -> TestResult {
    let file = ChunkFile::parse(
        TestChunkFile::crch()
            .big_endian_chunk(UNKNOWN_CRCH, 0x1234, b"abc".to_vec())
            .bytes(),
    )?;
    let model = read(&file)?;
    let info = ModelChunkSourceInfo::of(model.chunks.first().ok_or("CrCh chunk")?);
    let value = serde_json::to_value(info)?;
    assert_eq!(
        value.pointer("/layout"),
        Some(&json!({
            "container": "CrCh", "version": 0x1234, "id": 1, "byte_order": "big",
            "offset": "32", "span": {"basis": "declared-crch-size", "bytes": "3"}
        })),
        "declared table fields retain their exact separate meanings"
    );
    let schema = schemars::schema_for!(ModelChunkSourceInfo).to_value();
    let validator = jsonschema::validator_for(&schema)?;
    assert!(validator.is_valid(&value), "actual CrCh DTO validates");
    for (path, replacement) in [
        ("/layout/id", Value::Null),
        ("/layout/version", json!(0x8000)),
        (
            "/layout/span/basis",
            json!("derived-ivo-next-boundary-including-padding"),
        ),
    ] {
        assert!(
            !validator.is_valid(&replace(&value, path, replacement)?),
            "CrCh source profile rejects contradictory {path}"
        );
    }
    Ok(())
}

#[test]
fn source_schema_requires_nullable_keys_exact_widths_and_profile_basis() -> TestResult {
    let value = projected_ivo()?;
    let schema = schemars::schema_for!(ModelChunkSourceInfo).to_value();
    let validator = jsonschema::validator_for(&schema)?;
    for (path, replacement) in [
        ("/schema_version", json!(2)),
        ("/layout/version", json!(u64::from(u32::MAX) + 1)),
        ("/layout/id", json!(1)),
        ("/layout/byte_order", json!("big")),
        ("/layout/offset", json!(32)),
        ("/layout/offset", json!("01")),
        ("/layout/span/bytes", json!("18446744073709551616")),
        ("/layout/span/basis", json!("declared-crch-size")),
        ("/chunk/index", json!(u64::from(u32::MAX) + 1)),
        ("/chunk/type_id", json!(u64::from(u32::MAX) + 1)),
        ("/chunk/usage", json!("unsupported")),
    ] {
        assert!(
            !validator.is_valid(&replace(&value, path, replacement)?),
            "source schema rejects invalid {path}"
        );
    }
    for (object, key) in [("/chunk", "type"), ("/layout", "id")] {
        let mut changed = value.clone();
        changed
            .pointer_mut(object)
            .and_then(Value::as_object_mut)
            .ok_or("nullable object")?
            .remove(key);
        assert!(
            !validator.is_valid(&changed),
            "nullable {key} must be present"
        );
    }
    Ok(())
}

fn replace(value: &Value, path: &str, replacement: Value) -> Result<Value, Box<dyn Error>> {
    let mut changed = value.clone();
    *changed.pointer_mut(path).ok_or("source mutation path")? = replacement;
    Ok(changed)
}

#[test]
fn source_u64_wire_values_keep_the_full_decimal_range() -> TestResult {
    let model = unknown_ivo()?;
    let mut info = ModelChunkSourceInfo::of(model.chunks.first().ok_or("source chunk")?);
    info.layout.offset = DecimalU64::new(u64::MAX);
    let value = serde_json::to_value(info)?;
    assert_eq!(
        value.pointer("/layout/offset"),
        Some(&json!("18446744073709551615")),
        "the wire projection never rounds source u64 words"
    );
    let schema = schemars::schema_for!(ModelChunkSourceInfo).to_value();
    assert!(
        jsonschema::validator_for(&schema)?.is_valid(&value),
        "wire-boundary fixture, without claiming it is a usable physical range"
    );
    Ok(())
}

#[test]
fn existing_model_chunk_summary_and_single_file_chunk_json_keep_their_exact_bytes() -> TestResult {
    let file = ChunkFile::parse(
        TestChunkFile::ivo()
            .chunk(UNKNOWN_IVO, u32::MAX, b"abc".to_vec())
            .bytes(),
    )?;
    let model = read(&file)?;
    let chunk = model.chunks.first().ok_or("source chunk")?;
    assert_eq!(
        serde_json::to_string(&ChunkInfo::of(chunk))?,
        r#"{"file":"model","index":0,"type":null,"type_id":3735928559,"usage":"unknown"}"#,
        "the existing model summary has no new or changed wire fields"
    );
    let physical = file.chunks().first().ok_or("physical chunk")?;
    assert_eq!(
        serde_json::to_string(&ChunkJson::of(0, physical))?,
        r#"{"index":0,"type":null,"type_id":3735928559,"known":false,"version":4294967295,"id":null,"byte_order":"little","offset":32,"size":3}"#,
        "the existing single-file JSON keeps numeric offsets and lengths"
    );
    let legacy = serde_json::to_value(ChunkInfo::of(chunk))?;
    let source = serde_json::to_value(ModelChunkSourceInfo::of(chunk))?;
    assert_eq!(
        source.get("chunk"),
        Some(&legacy),
        "one canonical summary mapping"
    );
    assert!(
        source.get("raw").is_none(),
        "source observation is not a RawReference"
    );
    Ok(())
}
