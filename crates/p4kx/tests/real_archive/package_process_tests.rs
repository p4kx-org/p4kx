// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;
use std::process::{ExitStatus, Output};

#[cfg(unix)]
use std::os::unix::process::ExitStatusExt;
#[cfg(windows)]
use std::os::windows::process::ExitStatusExt;

#[test]
fn failed_subprocess_output_is_retained_and_never_formatted() -> crate::package_fault::Result<()> {
    let output = Output {
        status: ExitStatus::from_raw(2),
        stdout: b"PRIVATE_SENTINEL_STDOUT".to_vec(),
        stderr: b"PRIVATE_SENTINEL_STDERR".to_vec(),
    };
    let captured = super::FailedProcess(output.clone());
    assert_eq!(
        captured.0.stdout, output.stdout,
        "original stdout is retained internally"
    );
    assert_eq!(
        captured.0.stderr, output.stderr,
        "original stderr is retained internally"
    );
    assert!(
        !format!("{captured:?} {captured}").contains("PRIVATE_SENTINEL"),
        "subprocess formatting is technical only"
    );
    let error = super::decode(output, Path::new("PRIVATE_SENTINEL_PATH"))
        .err()
        .ok_or("failed process accepted")?;
    assert!(
        !format!("{error:?} {error}").contains("PRIVATE_SENTINEL"),
        "the error boundary excludes private process data"
    );
    assert!(
        error.source().is_none(),
        "generic source-chain formatting cannot expose process output"
    );
    Ok(())
}
