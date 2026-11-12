// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::io::Write as _;

use same_file::Handle;

#[test]
fn precommit_context_describes_the_staged_descriptor_that_is_renamed() {
    let dir = tempfile::tempdir().expect("folder");
    let out = dir.path().join("artifact.bin");
    fs::write(&out, b"previous").expect("previous output");
    let ((), staged) = super::write_replacing_protected_before(
        &out,
        None,
        None,
        |writer| {
            writer
                .write_all(b"current")
                .map_err(|error| super::write_error(&out, error))
        },
        |staged| {
            let temporary = staged.temporary();
            assert_eq!(temporary.parent(), out.parent());
            assert_eq!(fs::read(&out).expect("previous output"), b"previous");
            assert_eq!(fs::read(temporary).expect("staged output"), b"current");
            Handle::from_path(temporary).map_err(|error| super::write_error(temporary, error))
        },
    )
    .expect("atomic replacement");
    assert_eq!(
        staged,
        Handle::from_path(&out).expect("committed descriptor")
    );
}

#[test]
fn counted_commit_revalidates_a_staged_file_after_lock_acquisition()
-> Result<(), Box<dyn std::error::Error>> {
    use crate::{FileReceipt, OriginOutputAddress, SourceUses, write_origin_companion};

    let dir = tempfile::tempdir()?;
    let out = dir.path().join("artifact.bin");
    let receipt = FileReceipt::new(&out, None, None)?;
    let registry = SourceUses::new();
    let inventory = registry.freeze()?;
    let origin = registry.origin(&[], true);
    receipt.publish(
        |writer| writer.write_all(b"previous"),
        |writer, binding, ()| {
            write_origin_companion(
                writer,
                binding,
                &inventory,
                [(&OriginOutputAddress::OpaquePayload, &origin)],
            )
        },
    )?;
    let previous_receipt = fs::read(receipt.path())?;
    let (temporary, mut file, ownership) = super::create_temporary(&out, None, None)?;
    file.write_all(b"current")?;
    let staged = super::StagedOutput::new(&temporary, &out, &ownership, None, None);
    let retained = dir.path().join("retained.bin");
    let error = super::place(&staged, file, false, 7, 0, || {
        let replacement = super::OrdinaryReplacement::new(&out, None, None)
            .map_err(|source| super::write_error(&out, source))?;
        fs::rename(&temporary, &retained).map_err(|source| super::write_error(&out, source))?;
        fs::write(&temporary, b"unowned").map_err(|source| super::write_error(&out, source))?;
        Ok(replacement)
    })
    .expect_err("the staged name no longer identifies the created file");
    assert!(
        matches!(error, crate::ConversionError::Installation(_)),
        "{error}"
    );
    assert_eq!(fs::read(&out)?, b"previous", "previous artifact");
    assert_eq!(
        fs::read(receipt.path())?,
        previous_receipt,
        "previous receipt"
    );
    assert_eq!(fs::read(&retained)?, b"current", "actual staging bytes");
    assert_eq!(fs::read(&temporary)?, b"unowned", "unowned file");
    Ok(())
}
