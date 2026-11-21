// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::chunks::{ChunkFile, ivo};
use p4kx_extraction::{Archive, OpenOptions};
use p4kx_media::{ExportOptions, MediaKind, MediaRecord, Status};
use p4kx_testkit::{ArchiveBuilder, TestChunkFile, TestEntry, test_key};
use serde_json::{Value, json};

use super::model_source_provider::field_uses;
use super::support::{self, TestResult};

const LOD_DISTANCE: u32 = ivo::LOD_DISTANCES.id();

#[test]
fn model_lod_distance_words_preserve_bits_ranges_and_unsupported_original_chunks() -> TestResult {
    let temp = tempfile::tempdir()?;
    let file = fixture();
    let original = ChunkFile::parse(file.as_slice())?;
    let word = original
        .chunks()
        .first()
        .ok_or("first original chunk")?
        .offset()
        + 12;
    let archive_path = temp.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("synthetic.cga", file).encrypted())
        .write_to(&archive_path)?;
    let archive = Archive::open_with(archive_path, OpenOptions::new().key(Some(test_key())))?;
    let output = temp.path().join("out");
    let options = ExportOptions {
        kinds: vec![MediaKind::Model],
        ..ExportOptions::default()
    };
    let index = support::published(p4kx_media::export(&archive, &output, &options)?)?;
    let rows = support::records(&output, &index, "model.jsonl")?;
    let lod_rows = rows
        .iter()
        .enumerate()
        .filter(|(_, row)| row.subresource.namespace == "model-lod-distance-words")
        .collect::<Vec<_>>();
    let [
        (first_ordinal, first),
        (_, wrong_version),
        (_, different_extent),
    ] = lod_rows.as_slice()
    else {
        return Err("expected one LOD projection per physical chunk".into());
    };
    check_schema(&lod_rows)?;
    check_values(first, wrong_version, different_extent);
    let uses = field_uses(
        &output.join(&index.artifact_base),
        *first_ordinal,
        "/data/payload/words/3/bits",
        0,
        true,
    )?;
    assert!(
        uses.iter().any(|usage| {
            usage.pointer("/location/indices")
                == Some(&json!([word.to_string(), (word + 4).to_string()]))
                && usage.pointer("/location/field").and_then(Value::as_str)
                    == Some("lodDistanceWord")
        }),
        "field receipt identifies the original encrypted word"
    );
    Ok(())
}

fn fixture() -> Vec<u8> {
    let words = [
        8.0_f32.to_bits(),
        4.0_f32.to_bits(),
        0x8000_0000,
        0x7FC0_0420,
        1,
        2,
        3,
        4,
        0,
        0,
        0,
        0,
    ];
    let bytes = words
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect::<Vec<_>>();
    TestChunkFile::ivo()
        .chunk(LOD_DISTANCE, 0x900, bytes)
        .chunk(LOD_DISTANCE, 0x901, [0; 48])
        .chunk(LOD_DISTANCE, 0x900, [0; 16])
        .bytes()
}

fn check_schema(rows: &[(usize, &MediaRecord)]) -> TestResult {
    let schema = jsonschema::validator_for(&p4kx_media::builtin_record_schema()?)?;
    for (_, row) in rows {
        assert!(
            schema.is_valid(&serde_json::to_value(row)?),
            "LOD row has its closed schema"
        );
        assert_eq!(
            row.status,
            Status::Complete,
            "unsupported layout retains raw coverage"
        );
        assert!(row.source.raw.is_some(), "original bytes remain available");
    }
    Ok(())
}

fn check_values(first: &MediaRecord, wrong_version: &MediaRecord, different_extent: &MediaRecord) {
    assert_eq!(
        first.data.pointer("/payload/words/3"),
        Some(&json!({"bits":0x7FC0_0420_u32, "finite_binary32":null})),
        "the source NaN payload is retained as original bits",
    );
    assert_eq!(
        first.data.pointer("/payload/words/2/bits"),
        Some(&json!(0x8000_0000_u32)),
        "negative zero keeps its original bit pattern",
    );
    for (row, disposition) in [
        (wrong_version, "unsupported-version"),
        (different_extent, "unmeasured-extent"),
    ] {
        assert_eq!(
            row.data.pointer("/payload/disposition"),
            Some(&json!(disposition)),
            "unproved layouts have separate dispositions"
        );
        assert!(
            row.data.pointer("/payload/words").is_none(),
            "unknown layouts stay opaque"
        );
    }
}
