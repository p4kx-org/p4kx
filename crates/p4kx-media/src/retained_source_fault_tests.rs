// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::{self, File};
use std::io;
use std::path::Path;

use p4kx_extraction::{Exclusion, OwnedOutputCleanupError, WriteProtectionError};
use p4kx_testkit::TestEntry;

use crate::MediaError;
use crate::required_input_fault_support::{self as support, Inventory, TestResult};

#[derive(Clone, Copy)]
enum Callback {
    Success,
    IoFailure,
}

#[test]
fn private_cleanup_failure_prevents_a_successful_callback_result() -> TestResult {
    cleanup_after(Callback::Success)
}

#[test]
fn private_cleanup_failure_keeps_the_actual_callback_io_failure() -> TestResult {
    cleanup_after(Callback::IoFailure)
}

fn cleanup_after(callback: Callback) -> TestResult {
    support::with_fixture(
        [
            TestEntry::stored("owner.dds", b"H"),
            TestEntry::stored("owner.dds.1", b"needed"),
        ],
        Inventory::Intact,
        &Exclusion::default(),
        |context, writer, _checkpoint, folder| {
            let archive = context.directory.index.archive;
            let entry = archive.entries().get(1).ok_or("actual input")?;
            let parked = folder.join("parked-private");
            let mut original_root = None;
            let mut original_io = None;
            let result =
                super::with_private(archive, entry, writer, |_digest, payload, _writer| {
                    let root = payload
                        .parent()
                        .ok_or_else(|| MediaError::Contract("private parent".into()))?;
                    fs::rename(root, &parked).map_err(|source| crate::error::io(root, source))?;
                    fs::create_dir(root).map_err(|source| crate::error::io(root, source))?;
                    fs::write(root.join("sentinel"), b"replacement namespace")
                        .map_err(|source| crate::error::io(root, source))?;
                    original_root = Some(root.to_path_buf());
                    match callback {
                        Callback::Success => Ok(23),
                        Callback::IoFailure => {
                            let missing = folder.join("missing-callback-input");
                            let cause = File::open(&missing).err().ok_or_else(|| {
                                MediaError::Contract("missing callback file".into())
                            })?;
                            original_io = Some((cause.kind(), cause.raw_os_error()));
                            Err(crate::error::io(&missing, cause))
                        }
                    }
                });
            let root = original_root.ok_or("private materialisation callback never ran")?;
            let failure = result.err().ok_or("cleanup failure became success")?;
            let cleanup = cleanup_cause(callback, failure, original_io)?;
            assert!(
                matches!(
                    cleanup,
                    MediaError::Cleanup(OwnedOutputCleanupError::Protection(
                        WriteProtectionError::TargetIdentity { .. }
                    ))
                ),
                "actual changed namespace identity prevents cleanup"
            );
            restore_namespace(&root, &parked)
        },
    )
}

fn cleanup_cause(
    callback: Callback,
    failure: MediaError,
    original_io: Option<(io::ErrorKind, Option<i32>)>,
) -> Result<MediaError, Box<dyn std::error::Error + Send + Sync>> {
    use std::error::Error as _;

    match callback {
        Callback::Success => Ok(failure),
        Callback::IoFailure => {
            let expected = original_io.ok_or("callback I/O failure never observed")?;
            assert!(
                matches!(failure.source().and_then(|cause| cause.downcast_ref::<MediaError>()),
                    Some(MediaError::Io { source, .. }) if source.kind() == expected.0),
                "standard error traversal preserves the primary callback failure",
            );
            let MediaError::CleanupFailed { primary, cleanup } = failure else {
                return Err("original callback and cleanup were not both retained".into());
            };
            let MediaError::Io { source, .. } = *primary else {
                return Err("original callback I/O type disappeared".into());
            };
            assert_eq!((source.kind(), source.raw_os_error()), expected);
            assert_eq!(source.kind(), io::ErrorKind::NotFound);
            Ok(*cleanup)
        }
    }
}

fn restore_namespace(root: &Path, parked: &Path) -> TestResult {
    assert_eq!(
        fs::read(root.join("sentinel"))?,
        b"replacement namespace",
        "cleanup did not delete an unauthenticated replacement"
    );
    assert_eq!(
        fs::read(parked.join("payload"))?,
        b"needed",
        "original private bytes remain attributable"
    );
    fs::remove_file(root.join("sentinel"))?;
    fs::remove_dir(root)?;
    fs::rename(parked, root)?;
    Ok(())
}
