// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs::{self, File};
use std::io;

use crate::{ArchiveWriteProtection, CancelToken, WriteProtectionError};

use super::{Contention, LEASE_NAME, LOCK_NAME, ReceiptLock};

#[test]
fn a_foreign_or_abandoned_lease_refuses_without_removing_the_name() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let lease = directory.path().join(LEASE_NAME);
    let existing = File::create_new(&lease)?;
    let identity = same_file::Handle::from_file(existing)?;
    let cancel = CancelToken::new();
    for contention in [Contention::Refuse, Contention::Wait] {
        let refused = ReceiptLock::acquire_with(directory.path(), None, &cancel, contention)
            .err()
            .ok_or("a second writer must refuse the existing lease")?;
        assert_eq!(refused.kind(), io::ErrorKind::WouldBlock);
        assert_eq!(same_file::Handle::from_path(&lease)?, identity);
    }
    Ok(())
}

#[test]
fn a_legacy_lock_aliasing_the_input_archive_remains_protected() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let source = directory.path().join("archive.p4k");
    let lock = directory.path().join(LOCK_NAME);
    let cancel = CancelToken::new();
    File::create_new(&source)?;
    fs::hard_link(&source, &lock)?;
    let protection = ArchiveWriteProtection::new(&source, cancel.flag())?;

    let prohibited = ReceiptLock::acquire(directory.path(), Some(&protection), &cancel)
        .err()
        .ok_or("a lock aliasing the source must refuse")?;
    assert!(
        crate::owned_output_failure::io_error_chain(&prohibited)
            .find_map(|cause| cause.downcast_ref::<WriteProtectionError>())
            .is_some_and(|cause| matches!(cause, WriteProtectionError::SourceOverwrite { .. })),
        "the original alias guard must refuse a legacy marker pointing to the source: {prohibited:?}"
    );
    assert_eq!(fs::metadata(&source)?.len(), 0);
    assert!(!directory.path().join(LEASE_NAME).exists());
    Ok(())
}

#[test]
fn a_waiting_writer_refuses_to_wait_on_its_own_lease() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let cancel = CancelToken::new();
    let held = ReceiptLock::acquire_with(directory.path(), None, &cancel, Contention::Wait)?;
    let refused = ReceiptLock::acquire_with(directory.path(), None, &cancel, Contention::Wait)
        .err()
        .ok_or("a recursive writer must refuse its own lease")?;
    assert_eq!(refused.kind(), io::ErrorKind::WouldBlock);
    held.retire()?;
    let retry = ReceiptLock::acquire(directory.path(), None, &cancel)?;
    retry.retire()?;
    assert!(!directory.path().join(LEASE_NAME).exists());
    Ok(())
}

#[test]
fn session_drop_retires_its_lease_and_preserves_the_existing_marker() -> io::Result<()> {
    let directory = tempfile::tempdir()?;
    let marker = directory.path().join(LOCK_NAME);
    let legacy = same_file::Handle::from_file(File::create_new(&marker)?)?;
    let cancel = CancelToken::new();
    let held = ReceiptLock::acquire(directory.path(), None, &cancel)?;
    assert!(
        directory.path().join(LEASE_NAME).is_file(),
        "lease is visible"
    );
    drop(held);
    assert_eq!(
        same_file::Handle::from_path(&marker)?,
        legacy,
        "the existing marker retains its identity"
    );
    assert_eq!(
        fs::read_dir(directory.path())?.count(),
        1,
        "only the marker remains"
    );
    Ok(())
}
