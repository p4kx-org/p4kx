// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::io;
use std::sync::atomic::AtomicBool;

use super::Staging;
use crate::{PackageError, PackageWriteContext};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn a_foreign_file_occupying_the_destination_before_place_is_preserved() -> TestResult {
    let folder = tempfile::tempdir()?;
    let out = folder.path().join("package");
    let cancel = AtomicBool::new(false);
    let context = PackageWriteContext::standalone(&|| false, &cancel);
    let staging = Staging::create(&out, context)?;
    let temporary = staging.path().to_path_buf();
    fs::write(&out, b"foreign bytes")?;
    let result = staging.place();
    let error = staging
        .finish(result)
        .err()
        .ok_or("place replaced an occupied destination")?;
    assert!(matches!(error, PackageError::Write { source, .. }
        if source.kind() == io::ErrorKind::AlreadyExists));
    assert_eq!(fs::read(&out)?, b"foreign bytes");
    assert!(!temporary.exists());
    Ok(())
}
