// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use super::{Output, Row, Rows, capture_error, wire, write_error};
use crate::soc_scene::SocSceneError;
use crate::{PackageError, PackageWriteContext, PlayerSourceError};

type Check = Result<(), Box<dyn std::error::Error>>;

#[test]
fn row_serialization_keeps_cancellation_typed_and_does_not_commit_a_partial_row() -> Check {
    let workspace = tempfile::tempdir()?;
    let path = workspace.path().join("bundle");
    let cancelled = AtomicBool::new(false);
    let counter = AtomicUsize::new(0);
    let interrupt = || counter.fetch_add(1, Ordering::Relaxed) >= 1;
    let mut output = Output::create(
        &path,
        PackageWriteContext::standalone(&|| false, &cancelled),
    )?;
    let mut rows = Rows {
        writer: output.file(wire::RECORDS)?,
        context: PackageWriteContext::standalone(&interrupt, &cancelled),
        count: 0,
    };
    let result = rows
        .push(&Row::Header {
            format: wire::FORMAT,
            schema_version: wire::VERSION,
        })
        .map_err(|error| write_error(&path, error));
    assert_eq!(counter.load(Ordering::Relaxed), 2);
    assert_eq!(rows.count, 0);
    drop(rows);
    assert!(matches!(
        output.finish(result),
        Err(PackageError::Cancelled)
    ));
    assert!(!path.exists(), "partial row was never published");
    Ok(())
}

#[test]
fn nested_sink_cancellation_is_distinct_from_write_errors() {
    let path = Path::new("records.jsonl");
    let callback = |error| PlayerSourceError::Xml(SocSceneError::DocumentSink(error));
    assert!(matches!(
        capture_error(path, callback(io::Error::other(PackageError::Cancelled))),
        PackageError::Cancelled
    ));
    assert!(matches!(
        capture_error(path, callback(io::Error::other("ordinary write failure"))),
        PackageError::Write { .. }
    ));
}
