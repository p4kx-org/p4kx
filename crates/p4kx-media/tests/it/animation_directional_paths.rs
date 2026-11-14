// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::TestChunkFile;
use serde_json::{Value, json};

#[path = "animation_directional_pose_blocks.rs"]
mod directional_pose_tests;

use super::{
    CAF_CHUNK, DIRECTIONAL_CHUNK, TestResult, archive, field, image, options, support,
    validate_rows,
};

#[test]
fn directional_paths_require_count_agreement_and_keep_original_spans() -> TestResult {
    let temporary = tempfile::tempdir()?;
    let caf = TestChunkFile::ivo()
        .chunk(CAF_CHUNK, 0x902, image(&[])?)
        .bytes();
    let mut source = 2_u32.to_le_bytes().to_vec();
    source.extend([0xEE; 8]);
    source.extend(field(b"Animations/fixture/first.aim"));
    source.extend([0x40; 8]);
    source.extend(field(b"Animations/fixture/second.aim"));
    let mut incomplete = source.clone();
    incomplete
        .get_mut(..4)
        .ok_or("count word")?
        .copy_from_slice(&3_u32.to_le_bytes());
    let directional = TestChunkFile::ivo()
        .chunk(DIRECTIONAL_CHUNK, 0x901, source)
        .chunk(DIRECTIONAL_CHUNK, 0x901, incomplete)
        .bytes();
    let archive = archive(temporary.path(), &caf, &directional)?;
    let root = temporary.path().join("out");
    let index = support::published(p4kx_media::export(&archive, &root, &options())?)?;
    let rows = support::records(&root, &index, "animation.jsonl")?;
    validate_rows(&rows)?;
    let paths: Vec<_> = rows
        .iter()
        .filter(|row| row.subresource.namespace == "animation-image-directional-path")
        .collect();
    let [first, second] = paths.as_slice() else {
        return Err("two counted paths required".into());
    };
    assert_eq!(first.subresource.indices, [0, 0], "first counted field");
    assert_eq!(second.subresource.indices, [0, 1], "second counted field");
    assert_eq!(
        first.data.pointer("/payload/path/authored"),
        Some(&json!("Animations/fixture/first.aim")),
        "original path spelling"
    );
    let end = first
        .data
        .pointer("/payload/path_field/end_exclusive")
        .and_then(Value::as_u64)
        .ok_or("first path end")?;
    assert_eq!(
        second.data.pointer("/payload/path_field/start"),
        Some(&json!(end + 8)),
        "opaque segment remains between observations"
    );
    assert!(
        rows.iter()
            .any(|row| row.subresource.namespace == "animation-image-index"
                && row.subresource.indices == [1]
                && row.data.pointer("/payload/layout/status")
                    == Some(&json!("directional-opaque"))),
        "unreconciled chunk remains opaque",
    );
    Ok(())
}

#[test]
fn directional_records_follow_original_lengths_and_preserve_opaque_bytes() -> TestResult {
    let temporary = tempfile::tempdir()?;
    let caf = TestChunkFile::ivo()
        .chunk(CAF_CHUNK, 0x902, image(&[])?)
        .bytes();
    let first = record(b"Animations/fixture/first.aim", 9, &[7; 12])?;
    let bad_lexeme = record(&[0xff; 256], 12, &[9; 8])?;
    let last = record(b"Animations/fixture/last.aim", 23, &[5; 4])?;
    let framed = concatenate(3, [&first, &bad_lexeme, &last]);
    let truncated = framed
        .get(..framed.len() - last.len() - size_of::<u32>())
        .ok_or("truncated synthetic record table")?;
    let empty = 0_u32.to_le_bytes();
    let overflow = concatenate(u32::MAX, [&first, &bad_lexeme, &last]);
    let directional = TestChunkFile::ivo()
        .chunk(DIRECTIONAL_CHUNK, 0x901, framed.clone())
        .chunk(DIRECTIONAL_CHUNK, 0x901, truncated)
        .chunk(DIRECTIONAL_CHUNK, 0x901, overflow)
        .chunk(DIRECTIONAL_CHUNK, 0x901, empty)
        .bytes();
    let archive = archive(temporary.path(), &caf, &directional)?;
    let root = temporary.path().join("out");
    let index = support::published(p4kx_media::export(&archive, &root, &options())?)?;
    let rows = support::records(&root, &index, "animation.jsonl")?;
    validate_rows(&rows)?;
    let index_row = rows
        .iter()
        .find(|row| {
            row.subresource.namespace == "animation-image-index"
                && row.data.pointer("/payload/layout/record_count") == Some(&json!(3))
        })
        .ok_or("directional record index")?;
    let start = position(&index_row.data, "/payload/layout/header/end_exclusive")?;
    verify_directional_rows(&rows, start, [first, bad_lexeme, last])?;
    let paths: Vec<_> = rows
        .iter()
        .filter(|row| row.subresource.namespace == "animation-image-directional-path")
        .map(|row| row.subresource.indices.clone())
        .collect();
    assert_eq!(
        paths,
        [vec![0, 0], vec![0, 2]],
        "an unreadable path leaves its record and does not renumber later paths"
    );
    assert!(
        rows.iter().any(|row| row.subresource.indices == [3]
            && row.data.pointer("/payload/layout/record_count") == Some(&json!(0))),
        "an exact empty record table stays distinguishable"
    );
    Ok(())
}

fn verify_directional_rows(
    rows: &[p4kx_media::MediaRecord],
    start: u64,
    expected: [Vec<u8>; 3],
) -> TestResult {
    let records: Vec<_> = rows
        .iter()
        .filter(|row| row.subresource.namespace == "animation-image-directional-record")
        .collect();
    assert_eq!(
        records.len(),
        3,
        "only the completely framed three-record chunk has record rows"
    );
    let mut offset = start;
    for (ordinal, (row, raw)) in records.into_iter().zip(expected).enumerate() {
        assert_eq!(
            row.subresource.indices,
            [0, ordinal as u64],
            "physical record position"
        );
        let header_word =
            u32::from_le_bytes(raw.get(4..8).ok_or("original header word")?.try_into()?);
        let crc = u32::from_le_bytes(raw.get(264..268).ok_or("original CRC word")?.try_into()?);
        let end = offset + raw.len() as u64;
        for (field, want) in [
            ("range/start", offset),
            ("byte_length", raw.len() as u64),
            ("header_word", u64::from(header_word)),
            ("path_field/start", offset + 8),
            ("path_field/end_exclusive", offset + 264),
            ("aim_path_crc32", u64::from(crc)),
            ("opaque_tail/start", offset + 268),
            ("opaque_tail/end_exclusive", end),
            ("range/end_exclusive", end),
        ] {
            assert_eq!(
                position(&row.data, &format!("/payload/{field}"))?,
                want,
                "original {field} at record {ordinal}"
            );
        }
        offset = end;
    }
    Ok(())
}

fn concatenate<const N: usize>(count: u32, records: [&[u8]; N]) -> Vec<u8> {
    let mut bytes = count.to_le_bytes().to_vec();
    for record in records {
        bytes.extend_from_slice(record);
    }
    bytes
}

fn record(path: &[u8], header_word: u32, tail: &[u8]) -> TestResult<Vec<u8>> {
    let mut bytes = 0_u32.to_le_bytes().to_vec();
    bytes.extend(header_word.to_le_bytes());
    bytes.extend(if path.len() == 256 {
        path.to_vec()
    } else {
        field(path).to_vec()
    });
    bytes.extend(crc32fast::hash(&path.to_ascii_lowercase()).to_le_bytes());
    bytes.extend_from_slice(tail);
    let len = u32::try_from(bytes.len())?;
    bytes
        .get_mut(..4)
        .ok_or("synthetic length word")?
        .copy_from_slice(&len.to_le_bytes());
    Ok(bytes)
}

fn position(data: &Value, address: &str) -> TestResult<u64> {
    data.pointer(address)
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("missing directional field {address}").into())
}
