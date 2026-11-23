// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs::{self, File};
use std::io;

use crate::{ArchiveWriteProtection, CancelToken, WriteProtectionError};

use super::{Contention, LOCK_NAME, ReceiptLock};

#[test]
fn identity_admission_waits_until_the_generation_can_be_locked() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let source = directory.path().join("archive.p4k");
    let lock = directory.path().join(LOCK_NAME);
    let holder = File::create_new(&source)?;
    fs::hard_link(&source, &lock)?;
    let cancel = CancelToken::new();
    let protection = ArchiveWriteProtection::new(&source, cancel.flag())?;
    holder.try_lock().map_err(io::Error::from)?;

    let contended = ReceiptLock::acquire_with(
        directory.path(),
        Some(&protection),
        &cancel,
        Contention::Refuse,
    )
    .err()
    .ok_or("a contending writer must refuse")?;
    assert_eq!(
        contended.kind(),
        io::ErrorKind::WouldBlock,
        "identity admission must not reopen a generation held by another writer"
    );

    drop(holder);
    let prohibited = ReceiptLock::acquire_with(
        directory.path(),
        Some(&protection),
        &cancel,
        Contention::Refuse,
    )
    .err()
    .ok_or("a lock aliasing the source must refuse")?;
    assert!(
        prohibited
            .get_ref()
            .and_then(|cause| cause.downcast_ref::<WriteProtectionError>())
            .is_some_and(|cause| matches!(cause, WriteProtectionError::SourceOverwrite { .. })),
        "the acquired lock still requires the original source guard: {prohibited:?}"
    );
    assert_eq!(
        fs::metadata(&source)?.len(),
        0,
        "source bytes remain intact"
    );
    Ok(())
}
