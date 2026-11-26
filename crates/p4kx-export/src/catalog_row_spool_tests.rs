// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::io::{self, Read as _, Seek as _, SeekFrom, Write as _};

use p4kx_extraction::CancelToken;
use serde::ser::SerializeSeq as _;
use serde::{Serialize, Serializer};
use serde_json::json;
use serde_json::value::RawValue;

use super::{RowSpoolError, RowSpoolWriter};

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn rows_replay_in_original_order_as_the_same_compact_json() -> TestResult {
    let scratch = tempfile::tempdir()?;
    let mut writer = RowSpoolWriter::new(scratch.path())?;
    let values = [
        json!({"slot": 9, "label": "same"}),
        json!({"slot": 1, "label": "same", "text": "a\nb"}),
        json!({"slot": 9, "label": "same"}),
        json!({"nested": {"bytes": "x".repeat(128 * 1024)}}),
    ];
    for row in &values {
        writer.append(row, None)?;
    }
    let spool = writer.finish(None)?;
    assert_eq!(
        spool.rows(),
        values.len(),
        "physical occurrences are preserved"
    );
    let expected = serde_json::to_vec(&values)?;
    assert_eq!(
        spool.stored_bytes(),
        spool
            .file
            .lock()
            .map_err(|_| "private spool lock")?
            .metadata()?
            .len(),
        "the stored size is the logical length of the actual open spool"
    );
    assert!(
        spool.stored_bytes() < u64::try_from(expected.len())?,
        "the scratch file retains only the compressed row stream"
    );
    let view = spool.serialize_rows(None);
    assert_eq!(serde_json::to_vec(&view)?, expected, "the measure replay");
    assert_eq!(serde_json::to_vec(&view)?, expected, "the output replay");
    drop(spool);
    assert_eq!(
        scratch.path().read_dir()?.count(),
        0,
        "scratch is released on drop"
    );
    Ok(())
}

#[test]
fn streamed_lengths_separate_scalars_and_a_row_larger_than_the_buffer() -> TestResult {
    let scratch = tempfile::tempdir()?;
    let mut writer = RowSpoolWriter::new(scratch.path())?;
    let large = "x".repeat(super::BUFFER_BYTES * 4 + 1);
    let rows = [json!(1), json!(23), json!(&large), json!(4)];
    for row in &rows {
        writer.append(row, None)?;
    }
    let spool = writer.finish(None)?;
    let super::RowIndex::Sequential(lengths) = &spool.index else {
        return Err("expected sequential row lengths".into());
    };
    assert_eq!(
        lengths
            .iter()
            .map(|length| length.get())
            .collect::<Vec<_>>(),
        [1, 2, large.len() + 2, 1],
        "the index counts serialized bytes accepted across buffer flushes"
    );
    let expected = serde_json::to_vec(&rows)?;
    for _ in 0..2 {
        assert_eq!(serde_json::to_vec(&spool.serialize_rows(None))?, expected);
    }
    Ok(())
}

#[test]
fn empty_and_multiline_raw_rows_have_unambiguous_framing() -> TestResult {
    let scratch = tempfile::tempdir()?;
    let empty = RowSpoolWriter::new(scratch.path())?.finish(None)?;
    assert_eq!(
        serde_json::to_string(&empty.serialize_rows(None))?,
        "[]",
        "an empty stream is an empty array"
    );
    let mut writer = RowSpoolWriter::new(scratch.path())?;
    let raw = RawValue::from_string("{\n \"source\": [1, 2]\n}".to_owned())?;
    writer.append(raw.as_ref(), None)?;
    let spool = writer.finish(None)?;
    assert_eq!(
        serde_json::to_string(&spool.serialize_rows(None))?,
        format!("[{}]", raw.get()),
        "raw row framing preserves interior whitespace and line breaks"
    );
    Ok(())
}

#[test]
fn a_cancelled_append_closes_the_spool_without_publishing_a_prefix() -> TestResult {
    let scratch = tempfile::tempdir()?;
    let mut writer = RowSpoolWriter::new(scratch.path())?;
    writer.append(&json!({"position": 0}), None)?;
    let cancel = CancelToken::new();
    cancel.cancel();
    assert!(
        matches!(
            writer.append(&json!({"position": 1}), Some(&cancel)),
            Err(RowSpoolError::Cancelled)
        ),
        "cancellation remains typed"
    );
    assert!(
        matches!(writer.finish(None), Err(RowSpoolError::Closed)),
        "a failed row cannot leave a publishable prefix"
    );
    assert_eq!(
        scratch.path().read_dir()?.count(),
        0,
        "failed scratch is removed"
    );
    Ok(())
}

#[test]
fn replay_cancellation_and_output_errors_are_distinct() -> TestResult {
    let scratch = tempfile::tempdir()?;
    let mut writer = RowSpoolWriter::new(scratch.path())?;
    writer.append(&true, None)?;
    let spool = writer.finish(None)?;
    let cancel = CancelToken::new();
    let cancelled = spool.serialize_rows(Some(&cancel));
    cancel.cancel();
    assert!(
        serde_json::to_vec(&cancelled).is_err(),
        "no cancelled array"
    );
    assert!(
        matches!(cancelled.take_failure(), Some(RowSpoolError::Cancelled)),
        "the caller can propagate the typed cancellation"
    );
    let output = spool.serialize_rows(None);
    assert!(
        serde_json::to_writer(FailedOutput, &output).is_err(),
        "destination failure is surfaced"
    );
    assert!(
        output.take_failure().is_none(),
        "the destination error keeps its own cause"
    );
    assert_eq!(
        serde_json::to_string(&output)?,
        "[true]",
        "a failed output did not consume the immutable row source"
    );
    Ok(())
}

#[test]
fn an_in_place_checksum_corruption_stops_replay() -> TestResult {
    let scratch = tempfile::tempdir()?;
    let mut writer = RowSpoolWriter::new(scratch.path())?;
    writer.append(&json!({"value": 42}), None)?;
    let spool = writer.finish(None)?;
    {
        let mut file = spool.file.lock().map_err(|_| "private spool lock")?;
        file.seek(SeekFrom::End(-1))?;
        let mut byte = [0];
        file.read_exact(&mut byte)?;
        file.seek(SeekFrom::End(-1))?;
        file.write_all(&byte.map(|last| last ^ 0xff))?;
    }
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
fn cancellation_during_a_streamed_row_closes_the_writer() -> TestResult {
    let scratch = tempfile::tempdir()?;
    let mut writer = RowSpoolWriter::new(scratch.path())?;
    writer.append(&1, None)?;
    let cancel = CancelToken::new();
    let partial = CancelDuringRow {
        prefix: "x".repeat(super::BUFFER_BYTES + 1),
        cancel: &cancel,
    };
    assert!(matches!(
        writer.append(&partial, Some(&cancel)),
        Err(RowSpoolError::Cancelled)
    ));
    assert!(matches!(writer.finish(None), Err(RowSpoolError::Closed)));
    assert_eq!(scratch.path().read_dir()?.count(), 0);
    Ok(())
}

struct CancelDuringRow<'a> {
    prefix: String,
    cancel: &'a CancelToken,
}

impl Serialize for CancelDuringRow<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut sequence = serializer.serialize_seq(Some(2))?;
        sequence.serialize_element(&self.prefix)?;
        self.cancel.cancel();
        sequence.serialize_element(&false)?;
        sequence.end()
    }
}

struct FailedOutput;

impl io::Write for FailedOutput {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        Err(io::Error::other("test output refused writes"))
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
