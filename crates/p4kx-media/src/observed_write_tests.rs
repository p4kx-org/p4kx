// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};

use p4kx_extraction::CancelToken;
use serde_json::value;

use super::ObservedWrite;
use crate::MediaError;
use crate::checkpoint::IO_BLOCK_BYTES;
use crate::origin_index_test_support::{self, TestResult};

#[test]
fn raw_json_fragment_polls_between_bounded_writes() -> TestResult {
    origin_index_test_support::run(|_, checkpoint| {
        let fragment = value::to_raw_value(&"x".repeat(IO_BLOCK_BYTES * 2))?;
        let mut sink = CancelAfterWrite {
            cancel: checkpoint.cancel,
            bytes: 0,
        };
        let result = serde_json::to_writer(ObservedWrite::new(&mut sink, &checkpoint), &fragment);
        let actual = crate::json_output::finish(result, checkpoint);
        assert!(
            matches!(actual, Err(MediaError::Cancelled)),
            "the raw JSON write returns the original cancellation"
        );
        assert_eq!(
            sink.bytes, IO_BLOCK_BYTES,
            "one raw JSON fragment cannot skip the next writer checkpoint"
        );
        Ok(())
    })
}

struct CancelAfterWrite<'a> {
    cancel: &'a CancelToken,
    bytes: usize,
}

impl Write for CancelAfterWrite<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.bytes += bytes.len();
        self.cancel.cancel();
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
