// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::io::Write as _;

use super::{FRAME, RAW, Sizes, reconcile, support::TestResult};

const CATALOG: &str = concat!(
    r#"{"schemaVersion":3,"texts":{"example":"München"},"provenance":{"sources":[],"uses":[]}}"#,
    "\n",
);

#[test]
fn completed_frame_is_copied_exactly_and_existing_raw_is_preserved() -> TestResult {
    let directory = tempfile::tempdir()?;
    let frame = encoded()?;
    fs::write(directory.path().join(FRAME), &frame)?;
    let census = reconcile(directory.path(), &sizes(&frame, CATALOG.len() as u64))?;
    assert_eq!(census.encoded_bytes, frame.len() as u64, "encoded length");
    let raw = directory.path().join(RAW);
    assert_eq!(fs::read(&raw)?, CATALOG.as_bytes(), "exact UTF-8 output");

    fs::write(&raw, b"previous publication")?;
    assert!(
        reconcile(directory.path(), &sizes(&frame, CATALOG.len() as u64)).is_err(),
        "the offline census refuses to replace a raw file"
    );
    assert_eq!(
        fs::read(raw)?,
        b"previous publication",
        "older bytes survive"
    );
    Ok(())
}

#[test]
fn corrupt_frames_and_inconsistent_receipts_leave_no_raw_catalog() -> TestResult {
    let directory = tempfile::tempdir()?;
    let frame = encoded()?;
    let input = directory.path().join(FRAME);
    fs::write(&input, &frame)?;
    let wrong_size = sizes(&frame, CATALOG.len() as u64 + 1);
    assert!(
        reconcile(directory.path(), &wrong_size).is_err(),
        "decoded length must agree with the producer"
    );
    let mut truncated = frame;
    let _last_byte = truncated.pop().ok_or("frame is empty")?;
    fs::write(&input, &truncated)?;
    assert!(
        reconcile(directory.path(), &sizes(&truncated, CATALOG.len() as u64)).is_err(),
        "a truncated checksum invalidates even a matching encoded size"
    );
    assert_eq!(
        fs::read_dir(directory.path())?.count(),
        1,
        "only the original input remains"
    );
    Ok(())
}

fn encoded() -> TestResult<Vec<u8>> {
    let mut encoder = zstd::Encoder::new(Vec::new(), 1)?;
    encoder.include_checksum(true)?;
    encoder.write_all(CATALOG.as_bytes())?;
    Ok(encoder.finish()?)
}

const fn sizes(frame: &[u8], decoded_bytes: u64) -> Sizes {
    Sizes {
        encoded: frame.len() as u64,
        decoded: decoded_bytes,
        file: frame.len() as u64,
    }
}
