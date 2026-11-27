// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::io::{self, Write as _};

use crate::file_receipt::lock::LEASE_NAME;
use crate::output_file::StagedOutput;
use crate::{CancelToken, FileReceipt, MAX_PART_BYTES, OriginDigestWriter};

use super::{NEW, StagedReceiptPublication, describe, stage};

#[test]
fn a_different_regular_file_cannot_receive_the_committed_artifacts_proof() -> io::Result<()> {
    let folder = tempfile::tempdir()?;
    let output = folder.path().join("fixture.dds");
    let moved = folder.path().join("displaced.dds");
    let receipt = FileReceipt::for_native_staged_replacement(&output, None, CancelToken::new())?;

    let placed = stage(
        &output,
        &receipt,
        || Ok(()),
        |writer, binding| {
            describe(writer, binding)?;
            fs::rename(&output, &moved)?;
            fs::write(&output, b"another regular file")
        },
    )?;
    assert!(
        matches!(
            placed,
            StagedReceiptPublication::Committed {
                receipt: Some(Err(_)),
                retirement: Ok(()),
            }
        ),
        "the receipt must refuse a different descriptor: {placed:?}"
    );
    assert_eq!(fs::read(&output)?, b"another regular file");
    assert_eq!(fs::read(&moved)?, NEW);
    assert!(
        !receipt.path().exists(),
        "no proof may name the wrong bytes"
    );
    assert!(
        !folder.path().join(LEASE_NAME).exists(),
        "retire the owned lease"
    );
    Ok(())
}

#[test]
fn an_overlong_native_receipt_name_keeps_its_committed_artifact() -> io::Result<()> {
    let folder = tempfile::tempdir()?;
    let output = folder.path().join("a".repeat(MAX_PART_BYTES));
    let temporary = folder.path().join("staged.part");
    let cancel = CancelToken::new();
    let receipt = FileReceipt::for_native_staged_replacement(&output, None, cancel.clone())?;
    let (file, owner) = crate::create_output_file(&temporary, None, cancel.flag())
        .map_err(crate::OwnedOutputError::into_io_error)?
        .into_parts();
    let mut measured = OriginDigestWriter::new(file);
    measured.write_all(NEW)?;
    let (digest, file) = measured.finish_with_sink()?;
    file.sync_all()?;
    drop(file);
    let staged = StagedOutput::new(&temporary, &output, &owner, None, Some(&cancel));
    let placed = receipt.publish_staged(&staged, digest, || Ok(()), describe);
    assert!(
        matches!(
            placed,
            StagedReceiptPublication::Committed {
                receipt: Some(Err(_)),
                retirement: Ok(()),
            }
        ),
        "only the overlong companion failed: {placed:?}"
    );
    assert_eq!(fs::read(&output)?, NEW, "the native artifact still commits");
    assert_eq!(
        fs::read_dir(folder.path())?.count(),
        1,
        "no stage or lock remains"
    );
    Ok(())
}
