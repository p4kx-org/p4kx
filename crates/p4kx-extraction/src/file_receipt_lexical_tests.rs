// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::io;
use std::path::Path;

use super::{FileReceipt, PAYLOAD, TestResult, publish_directory, publish_file, write_bytes};

#[cfg(windows)]
const ERROR_PRIVILEGE_NOT_HELD: i32 = 1314;

fn link(referent: &Path, output: &Path) -> io::Result<bool> {
    #[cfg(unix)]
    std::os::unix::fs::symlink(referent, output)?;
    #[cfg(windows)]
    if let Err(error) = std::os::windows::fs::symlink_file(referent, output) {
        if error.raw_os_error() == Some(ERROR_PRIVILEGE_NOT_HELD) {
            return Ok(false);
        }
        return Err(error);
    }
    Ok(true)
}

#[test]
fn an_ordinary_writer_replaces_only_the_link_in_a_fresh_destination() -> TestResult {
    let dir = tempfile::tempdir()?;
    let referent = dir.path().join("referent.bin");
    let output = dir.path().join("output.bin");
    fs::write(&referent, b"referent")?;
    if !link(&referent, &output)? {
        return Ok(());
    }
    write_bytes(&output)?;
    assert_eq!(fs::read(&referent)?, b"referent");
    assert_eq!(fs::read(&output)?, PAYLOAD);
    assert!(!fs::symlink_metadata(&output)?.file_type().is_symlink());
    assert_eq!(
        fs::read_dir(dir.path())?.count(),
        2,
        "only the two payloads"
    );
    Ok(())
}

#[test]
fn replacing_a_link_retires_only_its_own_direct_companion() -> TestResult {
    let dir = tempfile::tempdir()?;
    let referent = dir.path().join("referent.bin");
    let output = dir.path().join("output.bin");
    let named = FileReceipt::new(&output, None, None)?;
    let kept = FileReceipt::new(&referent, None, None)?;
    publish_file(&named)?;
    publish_file(&kept)?;
    let referent_receipt = fs::read(kept.path())?;
    fs::remove_file(&output)?;
    if !link(&referent, &output)? {
        return Ok(());
    }
    write_bytes(&output)?;
    assert_eq!(fs::read(&referent)?, PAYLOAD);
    assert_eq!(fs::read(kept.path())?, referent_receipt);
    assert!(
        !named.path().exists(),
        "the exact replaced name lost its proof"
    );
    assert!(!fs::symlink_metadata(&output)?.file_type().is_symlink());
    Ok(())
}

#[test]
fn a_directory_receipt_for_the_referent_survives_lexical_replacement() -> TestResult {
    let dir = tempfile::tempdir()?;
    let referent = dir.path().join("referent.bin");
    let output = dir.path().join("output.bin");
    let aggregate = dir.path().join(".p4kx-origin.jsonl");
    publish_directory(dir.path(), "referent.bin")?;
    let before = fs::read(&aggregate)?;
    if !link(&referent, &output)? {
        return Ok(());
    }
    write_bytes(&output)?;
    assert_eq!(fs::read(&referent)?, PAYLOAD);
    assert_eq!(fs::read(&aggregate)?, before);
    assert!(!fs::symlink_metadata(&output)?.file_type().is_symlink());
    Ok(())
}

#[test]
fn a_directory_receipt_covering_the_replaced_link_name_is_retired() -> TestResult {
    let dir = tempfile::tempdir()?;
    let referent = dir.path().join("referent.bin");
    let output = dir.path().join("output.bin");
    let aggregate = dir.path().join(".p4kx-origin.jsonl");
    publish_directory(dir.path(), "output.bin")?;
    assert!(aggregate.is_file());
    fs::write(&referent, b"referent")?;
    fs::remove_file(&output)?;
    if !link(&referent, &output)? {
        return Ok(());
    }
    write_bytes(&output)?;
    assert_eq!(fs::read(&referent)?, b"referent");
    assert!(
        !aggregate.exists(),
        "the receipt named the replaced directory entry"
    );
    Ok(())
}
