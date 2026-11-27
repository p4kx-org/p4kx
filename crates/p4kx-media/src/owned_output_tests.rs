// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;
use std::path::Path;
use std::sync::atomic::AtomicBool;

use p4kx_extraction::{ArchiveWriteProtection, OwnedOutputCleanupError, OwnedOutputError};

use crate::{MediaError, error};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn cleanup_cancellation_keeps_both_errors_instead_of_becoming_plain_cancellation() -> TestResult {
    let cancelled = AtomicBool::new(true);
    let cause = ArchiveWriteProtection::new(Path::new("synthetic-source.p4k"), &cancelled)
        .err()
        .ok_or("cancelled source identity unexpectedly succeeded")?;
    assert!(cause.is_cancelled());
    let cleanup = MediaError::from(OwnedOutputCleanupError::Protection(cause));
    assert!(matches!(cleanup, MediaError::Cleanup(_)));
    let combined: Result<(), _> = error::finish(Err(MediaError::Cancelled), Err(cleanup));
    let Err(MediaError::CleanupFailed { primary, cleanup }) = combined else {
        return Err("cleanup failure was reduced to cancellation".into());
    };
    assert!(matches!(*primary, MediaError::Cancelled));
    assert!(matches!(*cleanup, MediaError::Cleanup(_)));
    Ok(())
}

#[test]
fn retained_creation_stays_a_failure_and_index_validation_keeps_its_typed_error() {
    let retained = MediaError::from(OwnedOutputError::RetainedCreation {
        path: "retained.part".into(),
        source: io::Error::other(MediaError::Cancelled),
    });
    assert!(matches!(retained, MediaError::OwnedOutput(_)));
    let domain = MediaError::from(OwnedOutputError::Io {
        path: "media".into(),
        source: io::Error::other(MediaError::Destination("invalid index".into())),
    });
    assert!(matches!(domain, MediaError::Destination(reason) if reason == "invalid index"));
}
