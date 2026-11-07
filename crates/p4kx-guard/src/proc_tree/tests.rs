// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;
use std::path::PathBuf;

use super::scan;

#[test]
fn a_partial_process_list_is_unknown_without_a_match() {
    let folders = [
        Ok(PathBuf::from("1")),
        Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "entry unavailable",
        )),
    ];
    let result = scan(folders, |_, _, bytes| {
        bytes.clear();
        bytes.extend_from_slice(b"explorer");
        Ok(true)
    });
    assert!(
        matches!(result, Err(ref error) if error.kind() == io::ErrorKind::PermissionDenied),
        "a partial enumeration does not establish absence"
    );
}

#[test]
fn an_unreadable_process_file_makes_absence_unknown() {
    let result = scan([Ok(PathBuf::from("1"))], |path, _, bytes| {
        bytes.clear();
        if path.ends_with("comm") {
            Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "comm denied",
            ))
        } else {
            bytes.extend_from_slice(b"other\0");
            Ok(true)
        }
    });
    assert!(
        matches!(result, Err(ref error) if error.kind() == io::ErrorKind::PermissionDenied),
        "one unreadable identity source leaves absence unknown"
    );
}

#[test]
fn a_partial_file_read_is_not_used_as_a_match() {
    let result = scan([Ok(PathBuf::from("1"))], |_, _, bytes| {
        bytes.clear();
        bytes.extend_from_slice(b"StarCitizen.exe");
        Err(io::Error::new(io::ErrorKind::UnexpectedEof, "partial read"))
    });
    assert!(
        matches!(result, Err(ref error) if error.kind() == io::ErrorKind::UnexpectedEof),
        "a failed read cannot produce a conclusive process name"
    );
}

#[test]
fn a_disappearing_process_does_not_make_absence_unknown() -> io::Result<()> {
    let result = scan([Ok(PathBuf::from("1"))], |_, _, _| {
        Err(io::Error::new(io::ErrorKind::NotFound, "process ended"))
    })?;
    assert!(!result, "processes ending during enumeration are skipped");
    Ok(())
}

#[test]
fn a_positive_match_is_conclusive_despite_other_unreadable_entries() -> io::Result<()> {
    let folders = [
        Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "entry unavailable",
        )),
        Ok(PathBuf::from("2")),
    ];
    let result = scan(folders, |_, _, bytes| {
        bytes.clear();
        bytes.extend_from_slice(b"StarCitizen.exe");
        Ok(true)
    })?;
    assert!(
        result,
        "an observed game establishes presence despite a partial list"
    );
    Ok(())
}
