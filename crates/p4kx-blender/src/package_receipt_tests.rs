// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::sync::OnceLock;
use std::sync::atomic::AtomicBool;

use crate::finite::tests::manifest;
use crate::package_receipt::{RECEIPT_FILE, source_revision, write_metadata};
use crate::stage::Staging;
use crate::{MANIFEST_FILE, PackageError, PackageWriteContext};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn cancellation_before_receipt_creation_preserves_the_previous_package() -> TestResult {
    let folder = tempfile::tempdir()?;
    let destination = folder.path().join("package");
    let previous = br#"{"format":"p4kx-blender-package"}"#;
    fs::create_dir(&destination)?;
    fs::write(destination.join(MANIFEST_FILE), previous)?;
    let observed = AtomicBool::new(false);
    let stage_path = OnceLock::new();
    let cancel = || {
        stage_path
            .get()
            .is_some_and(|path: &std::path::PathBuf| path.join(MANIFEST_FILE).is_file())
    };
    let context = PackageWriteContext::standalone(&cancel, &observed);
    let staging = Staging::create(&destination, context)?;
    stage_path
        .set(staging.path().to_path_buf())
        .map_err(|_| "staging path was already set")?;

    let result = write_metadata(staging.path(), &manifest(Vec::new()), Vec::new(), &context);
    let error = staging
        .complete(result)
        .err()
        .ok_or("cancelled receipt was published")?;
    assert!(matches!(error, PackageError::Cancelled));
    assert_eq!(fs::read(destination.join(MANIFEST_FILE))?, previous);
    assert_eq!(fs::read_dir(folder.path())?.count(), 1);
    Ok(())
}

#[test]
fn receipt_write_conflict_preserves_the_previous_package() -> TestResult {
    let folder = tempfile::tempdir()?;
    let destination = folder.path().join("package");
    let previous = br#"{"format":"p4kx-blender-package"}"#;
    fs::create_dir(&destination)?;
    fs::write(destination.join(MANIFEST_FILE), previous)?;
    let observed = AtomicBool::new(false);
    let context = PackageWriteContext::standalone(&|| false, &observed);
    let staging = Staging::create(&destination, context)?;
    let occupied = staging.path().join(RECEIPT_FILE);
    fs::write(&occupied, b"occupied")?;

    let result = write_metadata(staging.path(), &manifest(Vec::new()), Vec::new(), &context);
    let error = staging
        .complete(result)
        .err()
        .ok_or("occupied receipt was overwritten")?;
    assert!(matches!(error, PackageError::Write { path, source }
        if path == occupied && source.kind() == std::io::ErrorKind::AlreadyExists));
    assert_eq!(fs::read(destination.join(MANIFEST_FILE))?, previous);
    assert_eq!(fs::read_dir(folder.path())?.count(), 1);
    Ok(())
}

#[test]
fn generator_source_revision_requires_a_full_lowercase_object_id() {
    const REVISION: &str = "1234567890abcdef1234567890abcdef12345678";
    assert_eq!(source_revision(Some(REVISION)), Some(REVISION));
    assert_eq!(source_revision(Some("DEADBEEF")), None);
    assert_eq!(source_revision(Some("not-a-revision")), None);
    assert_eq!(source_revision(None), None);
}
