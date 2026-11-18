// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::io::ErrorKind;

use p4kx_testkit::{TestContainer, TestEntry};

use crate::format_error::FormatError;
use crate::source::Source;

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn a_required_original_name_read_preserves_source_io_failure() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("source.zip");
    let bytes = TestContainer::new()
        .entry(TestEntry::stored("member", b"value"))
        .build();
    std::fs::write(&path, bytes)?;
    let source = Source::open(&path)?;
    std::fs::OpenOptions::new()
        .write(true)
        .open(&path)?
        .set_len(0)?;
    assert!(
        matches!(super::read(&source, 0, b"member"),
            Err(FormatError::Io(error)) if error.kind() == ErrorKind::UnexpectedEof),
        "an actual failed original read cannot prove that a matching local frame is absent"
    );
    Ok(())
}

#[test]
fn a_readable_name_mismatch_is_not_an_original_frame_binding() -> TestResult {
    let bytes = TestContainer::new()
        .entry(TestEntry::stored("member", b"value"))
        .build();
    let source = Source::memory(bytes);
    assert!(
        super::read(&source, 0, b"other!")?.is_none(),
        "a readable different name has no physical owner binding"
    );
    let matched = super::read(&source, 0, b"member")?.ok_or("matching original local name")?;
    assert_eq!(
        matched.data_start, 36,
        "the stored extent starts after the original name"
    );
    assert!(
        !matched.cig,
        "the ordinary ZIP signature is retained separately"
    );
    Ok(())
}
