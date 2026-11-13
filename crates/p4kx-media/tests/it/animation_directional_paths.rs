// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::TestChunkFile;
use serde_json::{Value, json};

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
