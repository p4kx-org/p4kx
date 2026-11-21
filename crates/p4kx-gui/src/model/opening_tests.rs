// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;
use std::path::PathBuf;
use std::sync::mpsc;

use p4kx_extraction::ArchiveError;

use crate::discovery_test_support::{self, TestResult};
use crate::protected_open::ProtectedResult;
use crate::{Language, Model};

#[test]
fn model_poll_adopts_worker_notices_even_when_the_archive_open_fails() -> TestResult {
    let path = PathBuf::from("synthetic-archive.p4k");
    let (sender, result) = mpsc::channel();
    let completion = ProtectedResult {
        result: Err(ArchiveError::Io {
            path: path.clone(),
            source: io::Error::new(io::ErrorKind::PermissionDenied, "primary open sentinel"),
        }),
        issues: discovery_test_support::issues()?,
    };
    assert!(
        sender.send(completion).is_ok(),
        "the same production receiver receives the worker result"
    );
    let mut model = Model::new(Language::BritishEnglish);
    model.opening = Some(super::Opening {
        path: path.clone(),
        result,
    });
    assert!(
        model.poll(),
        "the production model consumes the actual completed opening"
    );
    let failure = model.failure().ok_or("opening failure")?;
    assert_eq!(
        failure.path, path,
        "discovery notices do not substitute the primary archive path"
    );
    assert!(
        failure
            .reason
            .as_deref()
            .is_some_and(|reason| reason.contains("primary open sentinel")),
        "the primary error is still shown separately"
    );
    assert!(
        !model.channels().discovery_issues().is_empty(),
        "the banner can access typed worker notices"
    );
    Ok(())
}
