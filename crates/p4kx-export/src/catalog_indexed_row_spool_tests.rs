// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read as _, Seek as _, SeekFrom, Write as _};

use p4kx_extraction::CancelToken;
use serde_json::json;

use super::super::RowSpoolError;
use super::IndexedRowSpoolWriter;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn indexed_frames_replay_sorted_keys_and_preserve_equal_key_admission() -> TestResult {
    let directory = tempfile::tempdir()?;
    let mut writer = IndexedRowSpoolWriter::new(directory.path())?;
    for (key, row) in [(3, "late"), (1, "first"), (1, "second"), (2, "middle")] {
        writer.append(key, &row, None)?;
    }
    let spool = writer.finish(None)?;
    let rows = spool.serialize_rows(None);
    assert_eq!(
        serde_json::to_string(&rows)?,
        r#"["first","second","middle","late"]"#,
        "seeking physical frames retains sorted keys and equal-key insertion order"
    );
    assert_eq!(
        serde_json::to_string(&rows)?,
        r#"["first","second","middle","late"]"#,
        "size measurement and output replay the same rows"
    );
    let cancel = CancelToken::new();
    cancel.cancel();
    let cancelled = spool.serialize_rows(Some(&cancel));
    assert!(
        serde_json::to_string(&cancelled).is_err(),
        "replay checks cancellation before seeking"
    );
    assert!(
        matches!(cancelled.take_failure(), Some(RowSpoolError::Cancelled)),
        "the replay retains its typed cancellation"
    );
    Ok(())
}

#[test]
fn indexed_frames_count_and_replay_a_row_wider_than_the_encoder_buffers() -> TestResult {
    let directory = tempfile::tempdir()?;
    let mut writer = IndexedRowSpoolWriter::new(directory.path())?;
    let large = json!({"name": "引用\n\"".repeat(super::BUFFER_BYTES)});
    let short = json!({"name": "short"});
    writer.append(2, &large, None)?;
    writer.append(1, &short, None)?;
    let spool = writer.finish(None)?;
    let super::super::RowIndex::Indexed(frames) = &spool.index else {
        return Err("expected indexed row frames".into());
    };
    assert_eq!(frames[0].original, serde_json::to_vec(&short)?.len());
    assert_eq!(frames[1].original, serde_json::to_vec(&large)?.len());
    assert_eq!(frames[0].offset, u64::try_from(frames[1].packed)?);
    let expected = serde_json::to_vec(&[short, large])?;
    let rows = spool.serialize_rows(None);
    assert_eq!(serde_json::to_vec(&rows)?, expected);
    assert_eq!(serde_json::to_vec(&rows)?, expected);
    Ok(())
}

#[test]
fn indexed_frames_reject_same_length_corruption() -> TestResult {
    let directory = tempfile::tempdir()?;
    let mut writer = IndexedRowSpoolWriter::new(directory.path())?;
    writer.append(0, &["row contents"], None)?;
    let spool = writer.finish(None)?;
    {
        let mut file = spool
            .backing
            .file
            .lock()
            .map_err(|_| "poisoned fixture spool")?;
        let mut byte = [0];
        file.seek(SeekFrom::End(-1))?;
        file.read_exact(&mut byte)?;
        file.seek(SeekFrom::End(-1))?;
        file.write_all(&[byte[0] ^ 1])?;
    }
    let rows = spool.serialize_rows(None);
    assert!(
        serde_json::to_string(&rows).is_err(),
        "the frame checksum rejects modified row data"
    );
    assert!(
        rows.take_failure().is_some(),
        "the typed spool error survives serialization"
    );
    Ok(())
}
