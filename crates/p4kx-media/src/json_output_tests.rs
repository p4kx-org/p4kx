// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::Cell;
use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::Path;

use p4kx_extraction::{CancelToken, Exclusion};
use serde::{Serialize, Serializer};

use crate::checkpoint::IO_BLOCK_BYTES;
use crate::required_input_fault_support::{self as support, Inventory, TestResult};
use crate::{MediaError, MediaWriter, Status, Subresource};

#[derive(Clone, Copy)]
enum Mode {
    Data,
    DataAndChanged,
    DataAndCancelled,
    Changed,
    Cancelled,
}

struct Controlled<'a> {
    path: &'a Path,
    cancel: &'a CancelToken,
    mode: Mode,
}

impl Serialize for Controlled<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::Error as _;
        match self.mode {
            Mode::Data => {}
            Mode::Cancelled | Mode::DataAndCancelled => self.cancel.cancel(),
            Mode::DataAndChanged | Mode::Changed => {
                OpenOptions::new()
                    .append(true)
                    .open(self.path)
                    .and_then(|mut file| file.write_all(b"actual serializer source change"))
                    .map_err(S::Error::custom)?;
            }
        }
        match self.mode {
            Mode::Data | Mode::DataAndChanged | Mode::DataAndCancelled => {
                Err(S::Error::custom("synthetic original serialisation failure"))
            }
            Mode::Changed => serializer.serialize_str(&"x".repeat(IO_BLOCK_BYTES * 2 + 17)),
            Mode::Cancelled => serializer.serialize_u8(0),
        }
    }
}

#[test]
fn actual_record_data_failure_and_later_changed_keep_the_original_json_error() -> TestResult {
    support::with_fixture(
        support::entries(),
        Inventory::Intact,
        &Exclusion::default(),
        |context, writer, checkpoint, _| {
            let data = Controlled {
                path: context.directory.index.archive.path(),
                cancel: checkpoint.cancel,
                mode: Mode::DataAndChanged,
            };
            let failure = record(writer, &data)
                .err()
                .ok_or("two serializer failures published")?;
            let MediaError::OutputCheckFailed {
                operation,
                checkpoint,
            } = failure
            else {
                return Err(
                    "independent serialization and source check were not both retained".into(),
                );
            };
            assert_data_error(*operation)?;
            assert!(matches!(*checkpoint, MediaError::Changed(_)));
            Ok(())
        },
    )
}

#[test]
fn an_actual_guarded_record_writer_change_stays_a_single_typed_changed() -> TestResult {
    support::with_fixture(
        support::entries(),
        Inventory::Intact,
        &Exclusion::default(),
        |context, writer, checkpoint, _| {
            let data = Controlled {
                path: context.directory.index.archive.path(),
                cancel: checkpoint.cancel,
                mode: Mode::Changed,
            };
            let failure = record(writer, &data)
                .err()
                .ok_or("changed serialization published")?;
            assert!(
                matches!(failure, MediaError::Changed(_)),
                "same guarded writer stop is not a second independent JSON failure: {failure:?}"
            );
            assert!(checkpoint.watch.is_changed());
            Ok(())
        },
    )
}

#[test]
fn actual_guarded_record_writer_cancellation_keeps_its_original_typed_stop() {
    let reached = Cell::new(false);
    let result = support::with_fixture(
        support::entries(),
        Inventory::Intact,
        &Exclusion::default(),
        |context, writer, checkpoint, _| {
            let data = Controlled {
                path: context.directory.index.archive.path(),
                cancel: checkpoint.cancel,
                mode: Mode::Cancelled,
            };
            let failure = record(writer, &data)
                .err()
                .ok_or("cancelled serialization published")?;
            assert!(
                matches!(failure, MediaError::Cancelled),
                "same actual cancellation stays one typed stop: {failure:?}"
            );
            reached.set(true);
            Ok(())
        },
    );
    finish_cancelled_case(result, reached.get());
}

#[test]
fn an_actual_record_data_error_keeps_its_original_json_category_and_message() -> TestResult {
    support::with_fixture(
        support::entries(),
        Inventory::Intact,
        &Exclusion::default(),
        |context, writer, checkpoint, _| {
            let data = Controlled {
                path: context.directory.index.archive.path(),
                cancel: checkpoint.cancel,
                mode: Mode::Data,
            };
            assert_data_error(
                record(writer, &data)
                    .err()
                    .ok_or("invalid data published")?,
            )
        },
    )
}

#[test]
fn an_unpositioned_writer_io_keeps_the_actual_original_os_error() -> TestResult {
    support::with_fixture(
        support::entries(),
        Inventory::Intact,
        &Exclusion::default(),
        |context, _writer, checkpoint, _| {
            let cause = failed_write(context.payload)?;
            let original: io::Error = failed_write(context.payload)?.into();
            let expected = (original.kind(), original.raw_os_error());
            let failure = super::finish(Err(cause), checkpoint)
                .err()
                .ok_or("read-only JSON write succeeded")?;
            let MediaError::Io { source, .. } = failure else {
                return Err("original writer I/O category".into());
            };
            assert_eq!((source.kind(), source.raw_os_error()), expected);
            assert!(source.raw_os_error().is_some());
            Ok(())
        },
    )
}

#[test]
fn actual_json_writer_io_and_a_later_change_keep_both_typed_causes() -> TestResult {
    support::with_fixture(
        support::entries(),
        Inventory::Intact,
        &Exclusion::default(),
        |context, _writer, checkpoint, _| {
            let cause = failed_write(context.payload)?;
            OpenOptions::new()
                .append(true)
                .open(context.directory.index.archive.path())?
                .write_all(b"later actual source change")?;
            let failure = super::finish(Err(cause), checkpoint)
                .err()
                .ok_or("two failures became success")?;
            let MediaError::OutputCheckFailed {
                operation,
                checkpoint,
            } = failure
            else {
                return Err("writer I/O and independent source change retained".into());
            };
            assert!(
                matches!(*operation, MediaError::Io { source, .. } if source.raw_os_error().is_some())
            );
            assert!(matches!(*checkpoint, MediaError::Changed(_)));
            Ok(())
        },
    )
}

#[test]
fn the_general_public_json_from_contract_remains_json_for_io_errors() -> TestResult {
    support::with_fixture(
        support::entries(),
        Inventory::Intact,
        &Exclusion::default(),
        |context, _writer, _checkpoint, _| {
            let cause = failed_write(context.payload)?;
            let expected = (cause.line(), cause.column(), cause.io_error_kind());
            let MediaError::Json(cause) = MediaError::from(cause) else {
                return Err("general From was changed".into());
            };
            assert_eq!(
                (cause.line(), cause.column(), cause.io_error_kind()),
                expected
            );
            Ok(())
        },
    )
}

fn failed_write(
    path: &Path,
) -> Result<serde_json::Error, Box<dyn std::error::Error + Send + Sync>> {
    let error = serde_json::to_writer(File::open(path)?, &0_u8)
        .err()
        .ok_or("read-only output unexpectedly accepted a byte")?;
    assert_eq!(error.classify(), serde_json::error::Category::Io);
    assert_eq!(
        (error.line(), error.column()),
        (0, 0),
        "only the owned writer's unpositioned I/O seam is normalized"
    );
    Ok(error)
}

fn record(writer: &mut MediaWriter<'_>, data: &Controlled<'_>) -> Result<(), MediaError> {
    writer.record(
        &Subresource::new("synthetic.json-output", vec![0]),
        Status::Complete,
        data,
        &[],
    )
}

fn assert_data_error(failure: MediaError) -> TestResult {
    let MediaError::Json(cause) = failure else {
        return Err("original JSON data error was replaced".into());
    };
    assert_eq!(cause.classify(), serde_json::error::Category::Data);
    assert_eq!(
        cause.to_string(),
        "synthetic original serialisation failure"
    );
    Ok(())
}

#[test]
fn independent_record_data_failure_and_actual_cancellation_keep_both_causes() {
    let reached = Cell::new(false);
    let result = support::with_fixture(
        support::entries(),
        Inventory::Intact,
        &Exclusion::default(),
        |context, writer, checkpoint, _| {
            let data = Controlled {
                path: context.directory.index.archive.path(),
                cancel: checkpoint.cancel,
                mode: Mode::DataAndCancelled,
            };
            let failure = record(writer, &data)
                .err()
                .ok_or("two cancellation failures published")?;
            let MediaError::OutputCheckFailed {
                operation,
                checkpoint,
            } = failure
            else {
                return Err("independent JSON and actual cancellation were not retained".into());
            };
            assert_data_error(*operation)?;
            assert!(matches!(*checkpoint, MediaError::Cancelled));
            reached.set(true);
            Ok(())
        },
    );
    finish_cancelled_case(result, reached.get());
}

fn finish_cancelled_case(result: TestResult, reached: bool) {
    assert!(reached, "actual serialization cancellation assertions ran");
    if let Err(error) = result {
        assert!(
            matches!(
                error.downcast_ref::<MediaError>(),
                Some(MediaError::Cleanup(_) | MediaError::CleanupFailed { .. })
            ),
            "only typed protected cleanup refusal may follow: {error}"
        );
    }
}

#[test]
fn an_actual_guarded_record_budget_keeps_the_exact_typed_limit() -> TestResult {
    const LIMIT: u64 = 2 << 10;
    support::with_fixture(
        support::entries(),
        Inventory::Intact,
        &Exclusion::default(),
        |_context, writer, _, _| {
            writer.limits.record_bytes = LIMIT;
            let data = "x".repeat(usize::try_from(LIMIT)? * 2);
            let failure = writer
                .record(
                    &Subresource::new("synthetic.json-limit", vec![0]),
                    Status::Complete,
                    &data,
                    &[],
                )
                .expect_err("oversized record must not be admitted");
            assert!(
                matches!(
                    failure,
                    MediaError::Limit {
                        budget: "record_bytes",
                        limit: LIMIT
                    }
                ),
                "the actual bounded writer retains its exact typed budget: {failure:?}"
            );
            Ok(())
        },
    )
}
