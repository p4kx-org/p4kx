// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::io::{self, Write};
use std::path::Path;

use crate::file_receipt::lock::LOCK_NAME;
use crate::guard_cancellation;
use crate::origin_file_io::{self, StageDisposition};
use crate::write;
use crate::{
    CancelToken, ConflictPolicy, FileReceipt, OriginArtifactBinding, OriginArtifactName,
    OriginDigestWriter, SourceUses, write_origin_companion,
};

use super::StagedReceiptPublication;

const OLD: &[u8] = b"previous DDS";
const NEW: &[u8] = b"new merged DDS";

#[test]
fn stage_cancellation_before_replacement_preserves_artifact_and_receipt() -> io::Result<()> {
    let folder = tempfile::tempdir()?;
    let output = folder.path().join("fixture.dds");
    let cancel = CancelToken::new();
    let receipt = FileReceipt::new_with_cancel(&output, None, None, cancel.clone())?;
    write_prior(&output, &receipt)?;
    let original_receipt = fs::read(receipt.path())?;

    let placed = stage(
        &output,
        &receipt,
        || {
            cancel.cancel();
            Ok(())
        },
        |_, _| panic!("no origin is described for an uncommitted artifact"),
    )?;
    assert!(
        matches!(placed, StagedReceiptPublication::Uncommitted(_)),
        "archive cancellation must precede replacement: {placed:?}"
    );
    assert_eq!(fs::read(&output)?, OLD, "the old bytes remain");
    assert_eq!(
        fs::read(receipt.path())?,
        original_receipt,
        "the old byte binding remains"
    );
    assert!(
        !folder.path().join(LOCK_NAME).exists(),
        "the ephemeral lock is retired"
    );
    assert_eq!(fs::read_dir(folder.path())?.count(), 2, "only the old pair");
    Ok(())
}

#[test]
fn receipt_encoder_failure_keeps_new_artifact_without_old_binding() -> io::Result<()> {
    let folder = tempfile::tempdir()?;
    let output = folder.path().join("fixture.dds");
    let receipt = FileReceipt::new(&output, None, None)?;
    write_prior(&output, &receipt)?;

    let placed = stage(
        &output,
        &receipt,
        || Ok(()),
        |writer, _| {
            writer.write_all(b"an unpublished prefix")?;
            Err(io::Error::other("injected receipt failure"))
        },
    )?;
    assert!(
        matches!(
            placed,
            StagedReceiptPublication::Committed {
                receipt: Err(ref error),
                retirement: Ok(()),
            }
                if error.to_string() == "injected receipt failure"
        ),
        "a receipt error must retain the committed outcome: {placed:?}"
    );
    assert_eq!(fs::read(&output)?, NEW, "the installed DDS remains");
    assert!(!receipt.path().exists(), "no stale or partial binding");
    assert_eq!(fs::read_dir(folder.path())?.count(), 1, "only the new DDS");
    Ok(())
}

#[test]
fn stage_check_at_receipt_commit_may_cancel_after_artifact_replacement() -> io::Result<()> {
    let folder = tempfile::tempdir()?;
    let output = folder.path().join("fixture.dds");
    let cancel = CancelToken::new();
    let receipt = FileReceipt::new_with_cancel(&output, None, None, cancel.clone())?;
    write_prior(&output, &receipt)?;

    let placed = stage(
        &output,
        &receipt,
        || Ok(()),
        |writer, binding| {
            describe(writer, binding)?;
            cancel.cancel();
            Ok(())
        },
    )?;
    assert!(
        matches!(
            placed,
            StagedReceiptPublication::Committed {
                receipt: Err(_),
                retirement: Ok(()),
            }
        ),
        "the cancelled companion cannot undo the rename: {placed:?}"
    );
    assert_eq!(fs::read(&output)?, NEW, "the installed DDS remains");
    assert!(
        !receipt.path().exists(),
        "the cancelled companion is absent"
    );
    assert_eq!(fs::read_dir(folder.path())?.count(), 1, "only the new DDS");
    Ok(())
}

#[test]
fn skip_keeps_the_artifact_and_receipt_installed_during_staging() -> io::Result<()> {
    let folder = tempfile::tempdir()?;
    let output = folder.path().join("fixture.wav");
    let receipt = FileReceipt::for_staged_replacement(&output, None, CancelToken::new())?;
    let publication = origin_file_io::write_staged(
        &output,
        None,
        None,
        |writer| {
            let mut measured = OriginDigestWriter::new(writer);
            measured.write_all(NEW)?;
            measured.finish()
        },
        |digest, staged| {
            write_prior(&output, &receipt)?;
            let before = fs::read(receipt.path())?;
            let result = receipt.publish_staged_with_policy(
                staged,
                digest,
                ConflictPolicy::Skip,
                || Ok(()),
                |_, _| panic!("the competing artifact owns its receipt"),
            );

            assert!(result.is_none(), "{result:?}");
            assert_eq!(
                fs::read(receipt.path())?,
                before,
                "the competing artifact keeps its proof"
            );
            Ok(StageDisposition::Cleanup(()))
        },
    );
    publication?;
    assert_eq!(
        fs::read(&output)?,
        OLD,
        "the competing artifact keeps its bytes"
    );
    assert_eq!(
        fs::read_dir(folder.path())?.count(),
        2,
        "only the existing pair"
    );
    Ok(())
}

fn stage(
    output: &Path,
    receipt: &FileReceipt<'_>,
    check: impl FnMut() -> io::Result<()>,
    describe: impl FnOnce(&mut dyn Write, &OriginArtifactBinding) -> io::Result<()>,
) -> io::Result<StagedReceiptPublication> {
    origin_file_io::write_staged(
        output,
        None,
        None,
        |writer| {
            let mut measured = OriginDigestWriter::new(writer);
            measured.write_all(NEW)?;
            measured.finish()
        },
        |digest, staged| match receipt.publish_staged(staged, digest, check, describe) {
            result @ StagedReceiptPublication::Uncommitted(_) => {
                Ok(StageDisposition::Cleanup(result))
            }
            result @ StagedReceiptPublication::Committed { .. } => {
                Ok(StageDisposition::Moved(result))
            }
        },
    )
}

fn write_prior(output: &Path, receipt: &FileReceipt<'_>) -> io::Result<()> {
    let mut measured = OriginDigestWriter::new(fs::File::create(output)?);
    measured.write_all(OLD)?;
    let binding = OriginArtifactBinding::File {
        path: OriginArtifactName::from_basename(
            output
                .file_name()
                .ok_or_else(|| io::Error::other("test output has no basename"))?,
        )?,
        digest: measured.finish()?,
    };
    describe(&mut fs::File::create(receipt.path())?, &binding)
}

fn describe(writer: &mut dyn Write, binding: &OriginArtifactBinding) -> io::Result<()> {
    let inventory = SourceUses::new().freeze().map_err(io::Error::other)?;
    write_origin_companion(writer, binding, &inventory, [])
}

#[test]
fn a_committed_artifact_receives_the_digest_of_its_staged_bytes() -> io::Result<()> {
    let folder = tempfile::tempdir()?;
    let output = folder.path().join("fixture.dds");
    let receipt = FileReceipt::new(&output, None, None)?;
    write_prior(&output, &receipt)?;
    let prior = fs::read(receipt.path())?;

    let placed = stage(&output, &receipt, || Ok(()), describe)?;
    assert!(
        matches!(
            placed,
            StagedReceiptPublication::Committed {
                receipt: Ok(()),
                retirement: Ok(()),
            }
        ),
        "both files were published and the lock retired: {placed:?}"
    );
    assert_eq!(fs::read(&output)?, NEW, "the new bytes are installed");
    let mut measured = OriginDigestWriter::new(io::sink());
    measured.write_all(NEW)?;
    let current = fs::read_to_string(receipt.path())?;
    assert!(
        current.contains(measured.finish()?.sha256()),
        "the binding hashes the published bytes"
    );
    assert_ne!(current.as_bytes(), prior, "the old binding was replaced");
    assert_eq!(fs::read_dir(folder.path())?.count(), 2, "only the new pair");
    Ok(())
}

#[test]
fn a_foreign_companion_refuses_replacement_of_the_prior_artifact() -> io::Result<()> {
    let folder = tempfile::tempdir()?;
    let output = folder.path().join("fixture.dds");
    let receipt = FileReceipt::new(&output, None, None)?;
    fs::write(&output, OLD)?;
    fs::write(receipt.path(), b"an unrelated file")?;

    let placed = stage(&output, &receipt, || Ok(()), describe)?;
    assert!(
        matches!(placed, StagedReceiptPublication::Uncommitted(_)),
        "a foreign target blocks companion publication before DDS replacement: {placed:?}"
    );
    assert_eq!(fs::read(&output)?, OLD, "the DDS was not replaced");
    assert_eq!(
        fs::read(receipt.path())?,
        b"an unrelated file",
        "the foreign path is untouched"
    );
    assert_eq!(
        fs::read_dir(folder.path())?.count(),
        2,
        "no staged file or lock"
    );
    Ok(())
}

#[test]
fn cancellation_keeps_a_typed_lock_retirement_failure() -> io::Result<()> {
    let folder = tempfile::tempdir()?;
    let output = folder.path().join("fixture.dds");
    let lock = folder.path().join(LOCK_NAME);
    let moved = folder.path().join("displaced-lock");
    let cancel = CancelToken::new();
    let receipt = FileReceipt::new_with_cancel(&output, None, None, cancel.clone())?;
    write_prior(&output, &receipt)?;

    let placed = stage(
        &output,
        &receipt,
        || {
            fs::rename(&lock, &moved)?;
            fs::write(&lock, b"foreign lock")?;
            cancel.cancel();
            Ok(())
        },
        |_, _| panic!("no origin is described for an uncommitted artifact"),
    )?;
    let StagedReceiptPublication::Uncommitted(error) = placed else {
        return Err(io::Error::other("the replaced lock must block publication"));
    };
    assert_eq!(fs::read(&output)?, OLD, "the prior artifact remains");
    assert_eq!(
        fs::read(&lock)?,
        b"foreign lock",
        "the foreign lock is preserved"
    );
    let failure = write::write_error(output.clone(), io::Error::other(error));
    assert!(
        guard_cancellation::retained_output(&failure),
        "nested lock retirement failure is reported even when cancelled"
    );

    let interruption = write::write_error(output, io::ErrorKind::Interrupted.into());
    assert!(
        !guard_cancellation::retained_output(&interruption),
        "an ordinary cancellation has no retained output failure"
    );
    Ok(())
}
