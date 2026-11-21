// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::io;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use p4kx_guard::{ArchiveWriteProtection, OwnedOutputCleanupError, OwnedOutputError};

use crate::package_paths::sibling_name;
use crate::package_replacement::Replacement;
use crate::stage::Staging;
use crate::{PackageError, PackageWriteContext, error};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn actual_cancel_cleans_owned_staging_and_only_empty_created_ancestors() -> TestResult {
    let folder = tempfile::tempdir()?;
    let out = folder.path().join("created").join("nested").join("package");
    let cancel = AtomicBool::new(false);
    let context = PackageWriteContext::standalone(&|| false, &cancel);
    let staging = Staging::create(&out, context)?;
    let stage = staging.path().to_path_buf();
    fs::write(stage.join("owned.bin"), b"owned")?;
    cancel.store(true, Ordering::Relaxed);
    let result: Result<(), _> = staging.finish(Err(PackageError::Cancelled));
    assert!(matches!(result, Err(PackageError::Cancelled)));
    assert!(!stage.exists());
    assert!(!folder.path().join("created").exists());
    Ok(())
}

#[test]
fn a_retargeted_stage_preserves_both_trees_and_both_failure_causes() -> TestResult {
    let folder = tempfile::tempdir()?;
    let out = folder.path().join("package");
    let cancel = AtomicBool::new(false);
    let context = PackageWriteContext::standalone(&|| false, &cancel);
    let staging = Staging::create(&out, context)?;
    let stage = staging.path().to_path_buf();
    fs::write(stage.join("owned.bin"), b"owned")?;
    let moved = folder.path().join("retained-stage");
    fs::rename(&stage, &moved)?;
    fs::create_dir(&stage)?;
    fs::write(stage.join("foreign.bin"), b"foreign")?;
    cancel.store(true, Ordering::Relaxed);
    let result: Result<(), _> = staging.finish(Err(PackageError::Cancelled));
    let Err(PackageError::CleanupFailed { primary, cleanup }) = result else {
        return Err("cleanup failure was reduced to cancellation".into());
    };
    assert!(matches!(*primary, PackageError::Cancelled));
    assert!(matches!(*cleanup, PackageError::Cleanup(_)));
    assert_eq!(fs::read(stage.join("foreign.bin"))?, b"foreign");
    assert_eq!(fs::read(moved.join("owned.bin"))?, b"owned");
    Ok(())
}

#[test]
fn cleanup_cancel_and_retained_creation_are_failures_and_domain_validation_stays_typed()
-> TestResult {
    let cancel = AtomicBool::new(true);
    let cause = ArchiveWriteProtection::new(Path::new("synthetic-source.p4k"), &cancel)
        .err()
        .ok_or("cancelled source identity unexpectedly succeeded")?;
    assert!(cause.is_cancelled());
    let cleanup = PackageError::from(OwnedOutputCleanupError::Protection(cause));
    assert!(matches!(cleanup, PackageError::Cleanup(_)));
    let combined: Result<(), _> = error::finish(Err(PackageError::Cancelled), Err(cleanup));
    assert!(matches!(combined, Err(PackageError::CleanupFailed { .. })));
    let retained = PackageError::from(OwnedOutputError::RetainedCreation {
        path: "retained.part".into(),
        source: io::Error::other(PackageError::Cancelled),
    });
    assert!(matches!(retained, PackageError::OwnedOutput(_)));
    let domain = PackageError::from(OwnedOutputError::Io {
        path: "package".into(),
        source: io::Error::other(PackageError::NotAPackage {
            path: "package".into(),
        }),
    });
    assert!(matches!(domain, PackageError::NotAPackage { .. }));
    Ok(())
}

#[test]
fn a_foreign_retained_backup_is_not_adopted_or_deleted_by_its_suffix() -> TestResult {
    let folder = tempfile::tempdir()?;
    let out = folder.path().join("package");
    fs::create_dir(&out)?;
    let name = out.file_name().ok_or("package name")?.to_os_string();
    let old = folder.path().join(sibling_name(&name, "old"));
    fs::create_dir(&old)?;
    fs::write(old.join("foreign.bin"), b"foreign")?;
    let cancel = AtomicBool::new(false);
    let context = PackageWriteContext::standalone(&|| false, &cancel);
    let staging = Staging::create(&out, context)?;
    let stage = staging.path().to_path_buf();
    assert!(matches!(
        staging.complete(Ok(())),
        Err(PackageError::BackupExists { .. })
    ));
    assert_eq!(fs::read(old.join("foreign.bin"))?, b"foreign");
    assert!(out.is_dir());
    assert!(!stage.exists());
    Ok(())
}

#[test]
fn failed_restore_keeps_the_replacement_error_and_the_filesystem_cause() -> TestResult {
    let folder = tempfile::tempdir()?;
    let out = folder.path().join("package");
    fs::create_dir(&out)?;
    let cancel = AtomicBool::new(false);
    let context = PackageWriteContext::standalone(&|| false, &cancel);
    let previous = Replacement::move_aside(&out, context)?.ok_or("existing package")?;
    let old = previous.path().to_path_buf();
    fs::write(&out, b"foreign replacement")?;
    let primary = PackageError::write(&out, io::Error::from(io::ErrorKind::PermissionDenied));
    let error = previous.restore(primary);
    let PackageError::CleanupFailed { primary, cleanup } = error else {
        return Err(format!("restore failure lost the replacement failure: {error:?}").into());
    };
    assert!(
        matches!(*primary, PackageError::Write { source, .. } if source.kind() == io::ErrorKind::PermissionDenied)
    );
    assert!(matches!(*cleanup, PackageError::NotRestored { source, .. }
        if source.kind() == io::ErrorKind::AlreadyExists));
    assert_eq!(fs::read(&out)?, b"foreign replacement");
    assert!(old.is_dir());
    Ok(())
}

#[test]
fn an_existing_backup_file_is_not_replaced_by_the_package_directory() -> TestResult {
    let folder = tempfile::tempdir()?;
    let out = folder.path().join("package");
    fs::create_dir(&out)?;
    let name = out.file_name().ok_or("package name")?.to_os_string();
    let old = folder.path().join(sibling_name(&name, "old"));
    fs::write(&old, b"foreign backup file")?;
    let cancel = AtomicBool::new(false);
    let context = PackageWriteContext::standalone(&|| false, &cancel);
    let staging = Staging::create(&out, context)?;
    assert!(
        matches!(staging.complete(Ok(())), Err(PackageError::BackupExists { path }) if path == old)
    );
    assert_eq!(fs::read(&old)?, b"foreign backup file");
    assert!(out.is_dir());
    Ok(())
}
