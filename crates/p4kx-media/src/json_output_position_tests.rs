// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::File;
use std::io::{self, Cursor, Read};

use p4kx_extraction::Exclusion;

use crate::MediaError;
use crate::required_input_fault_support::{self as support, Inventory, TestResult};

#[test]
fn foreign_parser_data_syntax_and_eof_positions_remain_exact() -> TestResult {
    support::with_fixture(
        support::entries(),
        Inventory::Intact,
        &Exclusion::default(),
        |_context, _writer, checkpoint, _| {
            let cases = [
                (
                    serde_json::from_slice::<serde_json::Value>(b"{bad}")
                        .err()
                        .ok_or("syntax fixture accepted")?,
                    serde_json::error::Category::Syntax,
                ),
                (
                    serde_json::from_slice::<serde_json::Value>(b"{")
                        .err()
                        .ok_or("EOF fixture accepted")?,
                    serde_json::error::Category::Eof,
                ),
                (
                    serde_json::from_slice::<u32>(b"\"wrong-type\"")
                        .err()
                        .ok_or("data fixture accepted")?,
                    serde_json::error::Category::Data,
                ),
            ];
            for (cause, category) in cases {
                assert_eq!(cause.classify(), category);
                let expected = (
                    cause.classify(),
                    cause.line(),
                    cause.column(),
                    cause.to_string(),
                );
                let failure = super::finish(Err(cause), checkpoint)
                    .err()
                    .ok_or("parser failure accepted")?;
                let MediaError::Json(cause) = failure else {
                    return Err("foreign parser error category changed".into());
                };
                assert_eq!(
                    (
                        cause.classify(),
                        cause.line(),
                        cause.column(),
                        cause.to_string()
                    ),
                    expected
                );
            }
            Ok(())
        },
    )
}

#[test]
fn a_positioned_foreign_parser_io_error_is_not_unwrapped_as_writer_io() -> TestResult {
    support::with_fixture(
        support::entries(),
        Inventory::Intact,
        &Exclusion::default(),
        |_context, _writer, checkpoint, folder| {
            let original = File::open(folder.join("missing-foreign-reader"))
                .err()
                .ok_or("missing foreign reader opened")?;
            let reader = FailedReader {
                prefix: Cursor::new(b"{\n \"item\": ".as_slice()),
                failure: Some(original),
            };
            let cause = serde_json::from_reader::<_, serde_json::Value>(reader)
                .err()
                .ok_or("foreign reader unexpectedly succeeded")?;
            assert_eq!(cause.classify(), serde_json::error::Category::Io);
            assert!(
                cause.line() != 0 || cause.column() != 0,
                "fixture must have a real parser position"
            );
            let expected = (
                cause.line(),
                cause.column(),
                cause.io_error_kind(),
                cause.to_string(),
            );
            let failure = super::finish(Err(cause), checkpoint)
                .err()
                .ok_or("foreign reader error vanished")?;
            let MediaError::Json(cause) = failure else {
                return Err("foreign parser I/O position was discarded".into());
            };
            assert_eq!(
                (
                    cause.line(),
                    cause.column(),
                    cause.io_error_kind(),
                    cause.to_string()
                ),
                expected
            );
            Ok(())
        },
    )
}

struct FailedReader {
    prefix: Cursor<&'static [u8]>,
    failure: Option<io::Error>,
}

impl Read for FailedReader {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        let read = self.prefix.read(output)?;
        if read > 0 {
            return Ok(read);
        }
        Err(self
            .failure
            .take()
            .unwrap_or_else(|| io::Error::other("foreign reader already failed")))
    }
}
