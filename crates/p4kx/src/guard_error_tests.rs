// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;
use std::path::Path;
use std::sync::atomic::AtomicBool;

use p4kx_extraction::{
    ArchiveWriteProtection, OwnedOutputCleanupError, OwnedOutputError, WriteProtectionError,
};

fn cancelled() -> WriteProtectionError {
    ArchiveWriteProtection::new(Path::new("unopened.p4k"), &AtomicBool::new(true))
        .expect_err("cancel before observation")
}

#[test]
fn a_pure_factory_source_refusal_keeps_the_invalid_input_classification() {
    let error = OwnedOutputError::Protection(WriteProtectionError::SourceOverwrite {
        archive: "source.p4k".into(),
        target: "alias.p4k".into(),
    })
    .into_io_error();
    assert!(super::is_refusal(&anyhow::Error::new(error)));
}

#[test]
fn a_cancelled_factory_is_not_an_invalid_input_refusal() {
    let error = OwnedOutputError::Protection(cancelled()).into_io_error();
    assert!(!super::is_refusal(&anyhow::Error::new(error)));
}

#[test]
fn cleanup_failure_with_a_cancelled_primary_remains_a_partial_failure() {
    let error = OwnedOutputError::Cleanup {
        primary: Box::new(OwnedOutputError::Io {
            path: "output.part".into(),
            source: io::Error::new(io::ErrorKind::PermissionDenied, cancelled()),
        }),
        cleanup: OwnedOutputCleanupError::UnexpectedType {
            path: "output.part".into(),
        },
    }
    .into_io_error();
    assert!(!super::is_refusal(&anyhow::Error::new(error)));
}
