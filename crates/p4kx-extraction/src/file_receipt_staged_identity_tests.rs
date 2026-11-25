// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::io;

use crate::file_receipt::lock::LEASE_NAME;
use crate::{CancelToken, FileReceipt};

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
