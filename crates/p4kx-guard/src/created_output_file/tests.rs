// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;
use std::sync::atomic::AtomicBool;

use crate::{OwnedFileToken, OwnedOutputError};

#[test]
fn writer_duplication_failure_uses_the_already_owned_descriptor_for_cleanup() {
    let root = tempfile::tempdir().expect("root");
    let path = root.path().join("output");
    let result = super::create_with(
        &path,
        None,
        &AtomicBool::new(false),
        OwnedFileToken::capture,
        |_| {
            Err(io::Error::new(
                io::ErrorKind::StorageFull,
                "synthetic descriptor duplication failure",
            ))
        },
    );
    let OwnedOutputError::Io { source, .. } = result.expect_err("duplication failed") else {
        panic!("original failure type changed");
    };
    assert_eq!(source.kind(), io::ErrorKind::StorageFull);
    assert!(!path.exists(), "verified temporary cleanup completed");
}

#[test]
fn identity_capture_failure_reports_the_explicitly_retained_creation() {
    let root = tempfile::tempdir().expect("root");
    let path = root.path().join("output");
    let result = super::create_with(
        &path,
        None,
        &AtomicBool::new(false),
        |path, _host, file, _protection| {
            drop(file);
            Err(OwnedOutputError::RetainedCreation {
                path: path.to_path_buf(),
                source: io::Error::new(
                    io::ErrorKind::Unsupported,
                    "synthetic identity unavailable",
                ),
            })
        },
        OwnedFileToken::writer,
    );
    let error = result.expect_err("identity unavailable");
    assert!(error.to_string().contains("retained"));
    let OwnedOutputError::RetainedCreation { source, .. } = error else {
        panic!("retention was hidden");
    };
    assert_eq!(source.kind(), io::ErrorKind::Unsupported);
    assert!(path.exists(), "unproved name was not blindly unlinked");
    assert_eq!(std::fs::metadata(path).expect("retained creation").len(), 0);
}

#[test]
fn a_factory_cannot_retry_an_identity_retention_as_a_name_collision() {
    let root = tempfile::tempdir().expect("root");
    let mut attempts = 0;
    let result = tempfile::Builder::new()
        .prefix("owned-")
        .make_in(root.path(), |path| {
            attempts += 1;
            super::create_with(
                path,
                None,
                &AtomicBool::new(false),
                |path, _host, file, _protection| {
                    drop(file);
                    Err(OwnedOutputError::RetainedCreation {
                        path: path.to_path_buf(),
                        source: io::Error::new(
                            io::ErrorKind::AlreadyExists,
                            "synthetic identity failure",
                        ),
                    })
                },
                OwnedFileToken::writer,
            )
            .map_err(OwnedOutputError::into_io_error)
        });
    let error = result.expect_err("retention is terminal");
    assert_eq!(attempts, 1);
    assert_eq!(error.kind(), io::ErrorKind::Other);
    let Some(OwnedOutputError::RetainedCreation { source, .. }) = error
        .get_ref()
        .and_then(|inner| inner.downcast_ref::<OwnedOutputError>())
    else {
        panic!("typed retained cause lost");
    };
    assert_eq!(source.kind(), io::ErrorKind::AlreadyExists);
    assert_eq!(std::fs::read_dir(root.path()).expect("outputs").count(), 1);
}
