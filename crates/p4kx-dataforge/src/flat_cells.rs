// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};

use crate::Value;
use crate::xml_text::FloatText;

/// Writes a value that never needs quoting: a number, boolean or GUID.
pub(crate) fn write_plain(out: &mut impl Write, value: &Value<'_>) -> io::Result<()> {
    match value {
        Value::Bool(value) => write!(out, "{value}"),
        Value::Int8(value) => write!(out, "{value}"),
        Value::Int16(value) => write!(out, "{value}"),
        Value::Int32(value) => write!(out, "{value}"),
        Value::Int64(value) => write!(out, "{value}"),
        Value::UInt8(value) => write!(out, "{value}"),
        Value::UInt16(value) => write!(out, "{value}"),
        Value::UInt32(value) => write!(out, "{value}"),
        Value::UInt64(value) => write!(out, "{value}"),
        Value::Single(value) => write!(out, "{}", FloatText(*value)),
        Value::Double(value) => write!(out, "{}", FloatText(*value)),
        Value::Guid(guid) => write!(out, "{guid}"),
        Value::String(_)
        | Value::Locale(_)
        | Value::Enum(_)
        | Value::Struct(_)
        | Value::Array(_)
        | Value::Reference(_)
        | Value::Null
        | Value::Error(_) => Ok(()),
    }
}

/// The UTF-8 byte order mark a default CSV file starts with, so that a spreadsheet reads the
/// encoding right.
pub const CSV_BYTE_ORDER_MARK: &[u8] = b"\xEF\xBB\xBF";

/// Writes `text` as one CSV cell, quoted when it holds a comma, quote, CR or LF.
///
/// With `neutralise`, a text a spreadsheet would run as a formula (it starts with `=`, `+`, `-`,
/// `@`, a tab or a carriage return) gets a leading `'`. Pass `false` for a number and for a raw
/// file.
///
/// # Errors
///
/// Returns the [`io::Error`] of writing to `out`.
pub fn write_csv_cell(out: &mut impl Write, text: &str, neutralise: bool) -> io::Result<()> {
    let mode = if neutralise {
        CsvCellMode::Spreadsheet
    } else {
        CsvCellMode::Raw
    };
    write_csv_cell_with_mode(out, text, mode)
}

/// How textual CSV cells are validated and formatted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum CsvCellMode {
    /// Quotes the field and protects text that a spreadsheet would interpret as a formula.
    Spreadsheet,
    /// Quotes the field and preserves every original UTF-8 character.
    Raw,
    /// Quotes the field and requires the ASCII field grammar of RFC 4180 Section 2.
    Strict,
}

impl CsvCellMode {
    /// Keeps the selected validation for a structural or numeric column.
    pub(crate) const fn verbatim(self) -> Self {
        match self {
            Self::Spreadsheet => Self::Raw,
            Self::Raw | Self::Strict => self,
        }
    }
}

/// Writes one CSV field using the selected text mode, without a byte order mark.
/// Empty strict fields are quoted.
///
/// # Errors
///
/// Returns a write error or an `InvalidData` error before writing any field bytes
/// if strict mode cannot represent the input.
pub fn write_csv_cell_with_mode(
    out: &mut impl Write,
    text: &str,
    mode: CsvCellMode,
) -> io::Result<()> {
    let quoted = if mode == CsvCellMode::Strict {
        strict_quoting(text)?
    } else {
        text.bytes().any(needs_quoting)
    };
    let prefix: &[u8] =
        if mode == CsvCellMode::Spreadsheet && text.starts_with(['=', '+', '-', '@', '\t', '\r']) {
            b"'"
        } else {
            b""
        };
    if !quoted {
        out.write_all(prefix)?;
        return out.write_all(text.as_bytes());
    }
    out.write_all(b"\"")?;
    out.write_all(prefix)?;
    let bytes = text.as_bytes();
    let mut start = 0;
    for (index, &byte) in bytes.iter().enumerate() {
        if byte == b'"' {
            out.write_all(bytes.get(start..=index).unwrap_or_default())?;
            out.write_all(b"\"")?;
            start = index.saturating_add(1);
        }
    }
    out.write_all(bytes.get(start..).unwrap_or_default())?;
    out.write_all(b"\"")
}

const fn needs_quoting(byte: u8) -> bool {
    matches!(byte, b',' | b'"' | b'\r' | b'\n')
}

fn strict_quoting(text: &str) -> io::Result<bool> {
    let mut quoted = text.is_empty();
    for byte in text.bytes() {
        if needs_quoting(byte) {
            quoted = true;
        } else if !(b' '..=b'~').contains(&byte) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "strict CSV requires ASCII field data allowed by RFC 4180. Use JSON, NDJSON, or lossless CSV.",
            ));
        }
    }
    Ok(quoted)
}

#[cfg(test)]
mod tests {
    use std::io;

    use super::{CsvCellMode, write_csv_cell, write_csv_cell_with_mode};

    #[test]
    fn strict_admits_only_rfc_field_bytes() -> io::Result<()> {
        for byte in 0_u8..=0x7f {
            let text = char::from(byte).to_string();
            let mut output = Vec::new();
            let result = write_csv_cell_with_mode(&mut output, &text, CsvCellMode::Strict);
            let allowed = matches!(byte, b'\n' | b'\r' | b' '..=b'~');
            assert_eq!(result.is_ok(), allowed, "ASCII byte {byte:#04x}");
            if allowed {
                result?;
                assert!(!output.is_empty(), "an accepted field writes bytes");
            } else {
                assert!(
                    output.is_empty(),
                    "a rejected ASCII byte writes no field bytes"
                );
            }
        }
        Ok(())
    }

    #[test]
    fn strict_rejects_invalid_quoted_fields_before_writing() {
        for text in ["a,\té", "a,\té\0", "a,\u{feff}", "a,\u{7f}"] {
            let mut output = b"earlier,".to_vec();
            let error = write_csv_cell_with_mode(&mut output, text, CsvCellMode::Strict)
                .expect_err("strict CSV rejects controls and non-ASCII in a quoted field");
            assert_eq!(error.kind(), io::ErrorKind::InvalidData, "invalid field");
            assert_eq!(
                output, b"earlier,",
                "no part of the rejected field was written"
            );
            assert_eq!(
                error.to_string(),
                "strict CSV requires ASCII field data allowed by RFC 4180. Use JSON, NDJSON, or lossless CSV.",
                "the error provides alternatives without returning field contents"
            );
        }
    }

    #[test]
    fn all_modes_keep_their_exact_escaping() -> io::Result<()> {
        let text = "=a,\"b\"\r\nc";
        let mut output = Vec::new();
        write_csv_cell_with_mode(&mut output, text, CsvCellMode::Strict)?;
        assert_eq!(
            output, b"\"=a,\"\"b\"\"\r\nc\"",
            "strict CSV uses doubled quotes and preserves the formula character"
        );
        output.clear();
        write_csv_cell_with_mode(&mut output, "", CsvCellMode::Strict)?;
        assert_eq!(output, b"\"\"", "strict CSV quotes an empty field");
        for (neutralise, mode, expected) in [
            (true, CsvCellMode::Spreadsheet, "\"'=a,\tcé\""),
            (false, CsvCellMode::Raw, "\"=a,\tcé\""),
        ] {
            let mut legacy = Vec::new();
            let mut typed = Vec::new();
            write_csv_cell(&mut legacy, "=a,\tcé", neutralise)?;
            write_csv_cell_with_mode(&mut typed, "=a,\tcé", mode)?;
            assert_eq!(
                legacy,
                expected.as_bytes(),
                "the public boolean API is stable"
            );
            assert_eq!(
                typed, legacy,
                "the typed legacy mode preserves the same bytes"
            );
        }
        Ok(())
    }
}
