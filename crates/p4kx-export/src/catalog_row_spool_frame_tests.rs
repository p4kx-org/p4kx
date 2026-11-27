// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read as _, Seek as _, SeekFrom, Write as _};

use p4kx_extraction::CancelToken;
use serde_json::json;

use super::super::{BUFFER_BYTES, RowSpool, RowSpoolError, RowSpoolWriter};
use super::TestResult;
use crate::CatalogError;

#[test]
fn an_in_place_checksum_corruption_stops_replay() -> TestResult {
    let scratch = tempfile::tempdir()?;
    let mut writer = RowSpoolWriter::new(scratch.path())?;
    writer.append(&json!({"value": 42}), None)?;
    let spool = writer.finish(None)?;
    flip_last_checksum_byte(&spool)?;
    let view = spool.serialize_rows(None);
    assert!(
        serde_json::to_vec(&view).is_err(),
        "frame checksum rejects changed bytes"
    );
    assert!(
        view.take_failure().is_some(),
        "spool replay error remains available"
    );
    Ok(())
}

#[test]
fn a_changed_suffix_checksum_keeps_the_original_frame_readable() -> TestResult {
    let scratch = tempfile::tempdir()?;
    let mut writer = RowSpoolWriter::new(scratch.path())?;
    writer.append(&10, None)?;
    let mut original = writer.finish(None)?;
    let ((), suffix) = original.append_suffix(None, |writer| {
        writer.append(&20, None).map_err(CatalogError::from)
    })?;
    let suffix = suffix.ok_or("missing appended frame")?;
    flip_last_checksum_byte(&suffix)?;

    assert_eq!(
        serde_json::to_string(&original.serialize_rows(None))?,
        "[10]"
    );
    let view = suffix.serialize_rows(None);
    assert!(
        serde_json::to_vec(&view).is_err(),
        "checksum rejects the suffix"
    );
    assert!(
        matches!(view.take_failure(), Some(RowSpoolError::Io(_))),
        "the changed checksum surfaces as a decode failure"
    );
    Ok(())
}

fn flip_last_checksum_byte(spool: &RowSpool) -> TestResult {
    let mut file = spool
        .backing
        .file
        .lock()
        .map_err(|_| "private spool lock")?;
    let length = file.metadata()?.len();
    file.seek(SeekFrom::End(-1))?;
    let mut byte = [0];
    file.read_exact(&mut byte)?;
    file.seek(SeekFrom::End(-1))?;
    file.write_all(&byte.map(|last| last ^ 0xff))?;
    assert_eq!(file.metadata()?.len(), length);
    Ok(())
}

#[test]
fn cancellation_after_a_staged_suffix_row_preserves_the_base_and_allows_retry() -> TestResult {
    let scratch = tempfile::tempdir()?;
    let mut writer = RowSpoolWriter::new(scratch.path())?;
    writer.append(&10, None)?;
    let mut original = writer.finish(None)?;
    let base_bytes = original.stored_bytes();
    let backing_file = original
        .backing
        .file
        .lock()
        .map_err(|_| "private spool lock")?
        .try_clone()?;
    let wide_row = wide_suffix_row();
    let cancel = CancelToken::new();
    let result = original.append_suffix(Some(&cancel), |writer| {
        writer.append(&wide_row, Some(&cancel))?;
        assert!(
            backing_file.metadata().map_err(RowSpoolError::Io)?.len() > base_bytes,
            "the suffix reaches the backing file before cancellation"
        );
        cancel.cancel();
        Ok(())
    });
    assert!(
        matches!(result, Err(CatalogError::Cancelled)),
        "a successful row append cannot publish a cancelled suffix"
    );
    assert_eq!(backing_file.metadata()?.len(), base_bytes);
    assert_eq!(
        serde_json::to_string(&original.serialize_rows(None))?,
        "[10]"
    );
    let ((), retry) = original.append_suffix(None, |writer| {
        writer.append(&20, None).map_err(CatalogError::from)
    })?;
    let retry = retry.ok_or("missing retried frame")?;
    assert_eq!(serde_json::to_string(&retry.serialize_rows(None))?, "[20]");
    drop(backing_file);
    drop(retry);
    drop(original);
    assert_eq!(
        scratch.path().read_dir()?.count(),
        0,
        "shared scratch is removed"
    );
    Ok(())
}

fn wide_suffix_row() -> String {
    let digits = b"0123456789abcdef";
    let mut seed = 1_u32;
    let mut row = String::with_capacity(8 * BUFFER_BYTES);
    for _ in 0..8 * BUFFER_BYTES {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        row.push(char::from(
            digits[usize::from(seed.to_le_bytes()[0] & 0x0f)],
        ));
    }
    row
}
