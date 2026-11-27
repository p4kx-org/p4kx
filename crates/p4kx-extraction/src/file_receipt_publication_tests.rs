// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::io::{self, Write};

use crate::file_receipt::ordinary::LockFinishFailure;
use crate::origin_file_io::{self, StageDisposition};
use crate::{
    ArchiveWriteProtection, CancelToken, ConflictPolicy, OriginArtifactBinding,
    OwnedOutputCleanupError, OwnedOutputError, SourceUses, write_origin_companion,
};

use super::{FilePublication, FileReceipt};
use crate::origin_file_io::install_without_replacing;

const ORIGINAL: &[u8] = b"# sound\nsynthetic.wav\n";
const EDITED: &[u8] = b"# sound\nsynthetic.wav ";

fn describe(writer: &mut dyn Write, binding: &OriginArtifactBinding) -> io::Result<()> {
    let inventory = SourceUses::new().freeze().map_err(io::Error::other)?;
    write_origin_companion(writer, binding, &inventory, [])
}

#[test]
fn retaining_a_file_also_retains_its_existing_receipt() -> io::Result<()> {
    let directory = tempfile::tempdir()?;
    let artifact = directory.path().join("file.txtp");
    let receipt = FileReceipt::new(&artifact, None, None)?;
    assert_eq!(
        receipt.publish_bytes(ORIGINAL, ConflictPolicy::Skip, describe)?,
        FilePublication::Written {
            size: ORIGINAL.len() as u64,
        },
        "a missing file and its receipt are published under Skip"
    );
    let first = fs::read(receipt.path())?;
    for (policy, source, verified_content) in [
        (ConflictPolicy::Skip, EDITED, false),
        (ConflictPolicy::IfChanged, ORIGINAL, true),
    ] {
        let result = receipt.publish_bytes(source, policy, |_, _| {
            Err(io::Error::other(
                "a retained file must not generate source proof",
            ))
        })?;
        assert_eq!(
            result,
            FilePublication::Kept {
                size: ORIGINAL.len() as u64,
                verified_content,
            },
            "retention reports whether the bytes were checked"
        );
        assert_eq!(fs::read(&artifact)?, ORIGINAL, "kept artifact bytes");
        assert_eq!(fs::read(receipt.path())?, first, "kept companion bytes");
    }
    assert_eq!(EDITED.len(), ORIGINAL.len(), "only the final byte differs");
    assert!(
        matches!(
            receipt.publish_bytes(EDITED, ConflictPolicy::IfChanged, describe)?,
            FilePublication::Written { .. }
        ),
        "full comparison detects the last-byte difference"
    );
    assert_eq!(fs::read(&artifact)?, EDITED, "new artifact bytes");
    assert_ne!(fs::read(receipt.path())?, first, "new artifact digest");
    Ok(())
}

#[test]
fn a_concurrent_destination_wins_a_no_clobber_installation() -> io::Result<()> {
    const OTHER: &[u8] = b"external writer\n";
    let directory = tempfile::tempdir()?;
    let target = directory.path().join("collision.txtp");
    let installed = origin_file_io::write_staged(
        &target,
        None,
        None,
        |writer| writer.write_all(ORIGINAL),
        |(), staged| {
            fs::write(&target, OTHER)?;
            let installed = install_without_replacing(staged, &target)?;
            Ok(StageDisposition::Cleanup(installed))
        },
    )?;
    assert!(!installed, "a racing destination is retained atomically");
    assert_eq!(fs::read(&target)?, OTHER, "the external contents survive");
    assert_eq!(fs::read_dir(directory.path())?.count(), 1, "no staged file");
    Ok(())
}

#[test]
fn cancellation_before_publication_keeps_existing_receipt_and_payload() -> io::Result<()> {
    let directory = tempfile::tempdir()?;
    let target = directory.path().join("cancelled.txtp");
    let cancel = CancelToken::new();
    let receipt = FileReceipt::new_with_cancel(&target, None, None, cancel.clone())?;
    receipt.publish_bytes(ORIGINAL, ConflictPolicy::Overwrite, describe)?;
    let previous = fs::read(receipt.path())?;
    cancel.cancel();
    let failure = receipt
        .publish_bytes(EDITED, ConflictPolicy::Overwrite, describe)
        .expect_err("cancellation prevents commit");
    assert_eq!(failure.kind(), io::ErrorKind::Interrupted, "{failure}");
    assert_eq!(fs::read(&target)?, ORIGINAL, "the prior bytes survive");
    assert_eq!(
        fs::read(receipt.path())?,
        previous,
        "the prior receipt survives"
    );
    Ok(())
}

#[derive(Clone, Copy, Debug)]
enum Publication {
    Stream,
    Bytes(ConflictPolicy),
}

#[test]
fn a_different_regular_file_cannot_receive_a_direct_publication_receipt() -> io::Result<()> {
    for publication in [
        Publication::Stream,
        Publication::Bytes(ConflictPolicy::Overwrite),
        Publication::Bytes(ConflictPolicy::Skip),
    ] {
        refuse_displaced_output(publication)?;
    }
    Ok(())
}

fn refuse_displaced_output(publication: Publication) -> io::Result<()> {
    const OTHER: &[u8] = b"another regular file";
    let directory = tempfile::tempdir()?;
    let artifact = directory.path().join("measured.txtp");
    let displaced = directory.path().join("displaced.txtp");
    let receipt = FileReceipt::new(&artifact, None, None)?;
    let replace = |writer: &mut dyn Write, binding: &OriginArtifactBinding| {
        describe(writer, binding)?;
        fs::rename(&artifact, &displaced)?;
        fs::write(&artifact, OTHER)
    };
    let result = match publication {
        Publication::Stream => receipt.publish(
            |writer| writer.write_all(ORIGINAL),
            |writer, binding, ()| replace(writer, binding),
        ),
        Publication::Bytes(policy) => receipt.publish_bytes(ORIGINAL, policy, replace).map(|_| ()),
    };
    assert!(
        result.is_err(),
        "{publication:?} must refuse the changed file identity"
    );
    assert_eq!(fs::read(&artifact)?, OTHER);
    assert_eq!(fs::read(&displaced)?, ORIGINAL);
    assert!(
        !receipt.path().exists(),
        "{publication:?} cannot publish a stale receipt"
    );
    assert_eq!(
        fs::read_dir(directory.path())?.count(),
        2,
        "only the output names remain"
    );
    Ok(())
}

#[test]
fn the_commit_observer_disambiguates_a_cancelled_receipt_after_artifact_placement() -> io::Result<()>
{
    let directory = tempfile::tempdir()?;
    let target = directory.path().join("committed.glb");
    let cancel = CancelToken::new();
    let receipt = FileReceipt::new_with_cancel(&target, None, None, cancel.clone())?;
    let observed = std::cell::Cell::new(false);
    let failure = receipt.publish_observing_commit(
        |writer| writer.write_all(b"placed artifact"),
        || {
            observed.set(true);
            cancel.cancel();
        },
        |_, _, &()| Ok(()),
    );
    assert!(observed.get(), "the callback runs after artifact placement");
    assert!(
        failure.is_err(),
        "late cancellation reports incomplete publication"
    );
    assert_eq!(
        fs::read(target)?,
        b"placed artifact",
        "the GLB was already committed"
    );
    assert!(
        !receipt.path().exists(),
        "there is no mismatched origin companion"
    );
    Ok(())
}

#[test]
fn cancelled_owned_factory_failure_is_a_precommit_cancellation() -> io::Result<()> {
    let directory = tempfile::tempdir()?;
    let archive = directory.path().join("Data.p4k");
    let target = directory.path().join("asset.glb.part");
    fs::write(&archive, b"original input")?;
    let cancel = CancelToken::new();
    let protection =
        ArchiveWriteProtection::new(&archive, cancel.flag()).map_err(io::Error::other)?;
    cancel.cancel();
    let factory = crate::create_output_file(&target, Some(&protection), cancel.flag())
        .expect_err("the cancelled factory refuses the staged file");
    let error = factory.into_io_error();

    assert_eq!(
        error.kind(),
        io::ErrorKind::PermissionDenied,
        "the regular file factory preserves the protection error kind"
    );
    assert!(
        FileReceipt::is_precommit_cancellation(&error, &cancel),
        "the factory's typed cancellation survives the I/O wrapper"
    );
    assert!(!target.exists(), "cancellation left no staged output");
    assert_eq!(fs::read(archive)?, b"original input", "original preserved");
    Ok(())
}

#[test]
fn retained_output_and_failed_lock_retirement_are_not_precommit_cancellation() {
    let cancel = CancelToken::new();
    cancel.cancel();
    let missing = std::path::Path::new("unopened.p4k");
    let protection = ArchiveWriteProtection::new(missing, cancel.flag())
        .expect_err("the guard supplies a typed cancellation");
    assert!(
        protection.is_cancelled(),
        "the primary error really is cancellation"
    );
    let output = missing.with_extension("glb.part");
    let cleanup = OwnedOutputError::Cleanup {
        primary: Box::new(OwnedOutputError::Protection(protection.clone())),
        cleanup: OwnedOutputCleanupError::UnexpectedType {
            path: output.clone(),
        },
    }
    .into_io_error();
    let retained = OwnedOutputError::RetainedCreation {
        path: output,
        source: io::Error::other("the staged file identity was not captured"),
    }
    .into_io_error();
    let lease = io::Error::other(LockFinishFailure::Uncommitted {
        primary: OwnedOutputError::Protection(protection).into_io_error(),
        cleanup: io::Error::from(io::ErrorKind::PermissionDenied),
    });
    for error in [cleanup, retained, lease] {
        assert!(
            !FileReceipt::is_precommit_cancellation(&error, &cancel),
            "a retained path or failed cleanup remains observable: {error}"
        );
    }
}
