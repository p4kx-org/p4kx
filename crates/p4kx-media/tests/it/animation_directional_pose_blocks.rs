// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;

use p4kx_extraction::{Archive, OpenOptions};
use p4kx_media::MediaRecord;
use p4kx_testkit::{ArchiveBuilder, TestChunkFile, TestEntry, test_key};
use serde_json::{Value, json};

use super::super::{
    DIRECTIONAL, DIRECTIONAL_CHUNK, TestResult, field, options, support, validate_rows,
};

const POSE_INDEX: &str = "animation-image-directional-pose-index";
const POSE_BLOCK: &str = "animation-image-directional-pose-block";
const RECORD: &str = "animation-image-directional-record";

#[test]
fn pose_blocks_keep_zero_counts_and_extension_bytes_with_encrypted_field_origins() -> TestResult {
    let temp = tempfile::tempdir()?;
    let entry = TestChunkFile::ivo()
        .chunk(DIRECTIONAL_CHUNK, 0x901, directional_body()?)
        .bytes();
    let archive_file = temp.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(DIRECTIONAL, entry).encrypted())
        .write_to(&archive_file)?;
    let archive = Archive::open_with(archive_file, OpenOptions::new().key(Some(test_key())))?;
    let output = temp.path().join("output");
    let index = support::published(p4kx_media::export(&archive, &output, &options())?)?;
    let rows = support::records(&output, &index, "animation.jsonl")?;
    validate_rows(&rows)?;
    let pose_indices: Vec<_> = rows
        .iter()
        .enumerate()
        .filter(|(_, row)| row.subresource.namespace == POSE_INDEX)
        .collect();
    let [(stream_ordinal, pose)] = pose_indices.as_slice() else {
        return Err("only the completely framed record has a pose index".into());
    };
    let start = verify_index(pose, &rows)?;
    let blocks: Vec<_> = rows
        .iter()
        .filter(|row| row.subresource.namespace == POSE_BLOCK)
        .collect();
    let [first, second] = blocks.as_slice() else {
        return Err("two physical pose blocks".into());
    };
    let next = verify_block(first, 0, start, [1, 2]);
    let end = verify_block(second, 1, next, [0, 1]);
    assert_eq!(
        pose.data.pointer("/payload/extensions/start"),
        Some(&json!(end)),
        "extensions start after the last framed pose"
    );
    assert_eq!(
        pose.data.pointer("/payload/extensions/end_exclusive"),
        Some(&json!(end + 552)),
        "one exact opaque extension"
    );
    let base = output.join(index.artifact_base);
    verify_origin(&base, *stream_ordinal)
}

fn directional_body() -> TestResult<Vec<u8>> {
    let path = b"Animations/fixture/pose.aim";
    let mut record = vec![0; 8];
    record.extend(field(path));
    record.extend(crc32fast::hash(&path.to_ascii_lowercase()).to_le_bytes());
    record.resize(8 + 2_156, 0xaa);
    for word in [2_u32, 1, 19] {
        record.extend(word.to_le_bytes());
    }
    for (lanes, count) in [(4, 1_u32), (3, 2), (4, 0), (3, 1)] {
        record.extend(count.to_le_bytes());
        let len = lanes * size_of::<f32>() * usize::try_from(count)?;
        record.resize(record.len() + len, 0x4d);
    }
    record.resize(record.len() + 552, 0x33);
    let len = u32::try_from(record.len())?;
    record
        .get_mut(..4)
        .ok_or("record length")?
        .copy_from_slice(&len.to_le_bytes());
    let mut truncated = record.clone();
    truncated
        .get_mut(8 + 2_156 + 12..8 + 2_156 + 16)
        .ok_or("first array count")?
        .copy_from_slice(&u32::MAX.to_le_bytes());
    let mut body = 2_u32.to_le_bytes().to_vec();
    body.extend(record);
    body.extend(truncated);
    Ok(body)
}

fn verify_index(pose: &MediaRecord, rows: &[MediaRecord]) -> TestResult<u64> {
    assert_eq!(
        pose.subresource.indices,
        [0, 0],
        "pose scope shares the physical record address"
    );
    let physical: Vec<_> = rows
        .iter()
        .filter(|row| row.subresource.namespace == RECORD)
        .collect();
    let [first, second] = physical.as_slice() else {
        return Err("both well-framed records survive regardless of pose coverage".into());
    };
    assert_eq!(
        second.subresource.indices,
        [0, 1],
        "bad inner count keeps its original outer position"
    );
    let path_at = first
        .data
        .pointer("/payload/path_field/start")
        .and_then(Value::as_u64)
        .ok_or("original record path start")?;
    let header_at = path_at + 2_156;
    for (field, value) in [
        ("pose_count", 2),
        ("extension_count", 1),
        ("header_word", 19),
        ("first_orientation/start", path_at + 288),
        ("first_orientation/end_exclusive", path_at + 304),
        ("second_orientation/start", path_at + 304),
        ("second_orientation/end_exclusive", path_at + 320),
        ("polar_grid/start", path_at + 320),
        ("polar_grid/end_exclusive", header_at),
        ("header/start", header_at),
        ("header/end_exclusive", header_at + 12),
    ] {
        assert_eq!(
            pose.data.pointer(&format!("/payload/{field}")),
            Some(&json!(value)),
            "the original pose header {field}"
        );
    }
    Ok(header_at + 12)
}

fn verify_block(row: &MediaRecord, ordinal: u64, start: u64, counts: [u64; 2]) -> u64 {
    assert_eq!(
        row.subresource.indices,
        [0, 0, ordinal],
        "physical pose ordinal"
    );
    let mut cursor = start;
    for ((field, width), count) in [("entries4", 16), ("entries3", 12)].into_iter().zip(counts) {
        let end = cursor + 4 + count * width;
        for (member, value) in [
            ("count", count),
            ("count_range/start", cursor),
            ("count_range/end_exclusive", cursor + 4),
            ("values/start", cursor + 4),
            ("values/end_exclusive", end),
        ] {
            assert_eq!(
                row.data.pointer(&format!("/payload/{field}/{member}")),
                Some(&json!(value)),
                "{field} original {member} at pose {ordinal}"
            );
        }
        cursor = end;
    }
    cursor
}

fn verify_origin(root: &std::path::Path, ordinal: usize) -> TestResult {
    let lines = fs::read_to_string(root.join("field-origins.jsonl"))?;
    let ordinal = ordinal.to_string();
    let selected = lines
        .lines()
        .map(serde_json::from_str::<Value>)
        .collect::<Result<Vec<_>, _>>()?;
    for address in [
        "/data/payload/pose_count",
        "/data/payload/extension_count",
        "/data/payload/header_word",
        "/data/payload/extensions/start",
    ] {
        let origin = selected
            .iter()
            .find(|row| {
                row.pointer("/scope/stream/kind").and_then(Value::as_str) == Some("animation")
                    && row.pointer("/scope/ordinal").and_then(Value::as_str)
                        == Some(ordinal.as_str())
                    && row.get("field").and_then(Value::as_str) == Some(address)
            })
            .ok_or_else(|| format!("missing origin for {address}"))?;
        assert_eq!(
            origin.get("containsEncryptedSource"),
            Some(&json!(true)),
            "{address} uses the encrypted pose header"
        );
        assert_eq!(
            origin.get("sourceSetComplete"),
            Some(&json!(true)),
            "{address} has original coverage"
        );
    }
    Ok(())
}
