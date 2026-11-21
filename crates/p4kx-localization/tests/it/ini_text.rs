// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Decoding the encodings a `global.ini` comes in, through the public API.

use std::error::Error;

use p4kx_localization::{IniEncoding, IniText, LocalizationError, LocalizationLimits};

type TestResult = Result<(), Box<dyn Error>>;

const SAMPLE: &str = "first=Erste\u{e4}\r\nsecond=\u{1f680}\r\n";

fn utf16(text: &str, big_endian: bool) -> Vec<u8> {
    text.encode_utf16()
        .flat_map(|unit| {
            if big_endian {
                unit.to_be_bytes()
            } else {
                unit.to_le_bytes()
            }
        })
        .collect()
}

fn with_bom(bom: &[u8], body: Vec<u8>) -> Vec<u8> {
    let mut bytes = bom.to_vec();
    bytes.extend(body);
    bytes
}

fn assert_sample(text: &IniText<'_>, encoding: IniEncoding) -> TestResult {
    assert_eq!(text.encoding(), encoding, "encoding");
    assert!(!text.is_lossy(), "nothing replaced");
    let ini = text.index()?;
    assert_eq!(ini.resolve("first"), Some("Erste\u{e4}"), "first");
    assert_eq!(ini.resolve("second"), Some("\u{1f680}"), "second");
    assert_eq!(ini.len(), 2, "two keys");
    Ok(())
}

#[test]
fn utf8_without_a_byte_order_mark_is_read_and_borrowed() -> TestResult {
    let bytes = SAMPLE.as_bytes();
    let text = IniText::decode(bytes)?;
    assert_sample(&text, IniEncoding::Utf8)?;
    assert!(
        std::ptr::eq(text.text().as_ptr(), bytes.as_ptr()),
        "no copy"
    );
    Ok(())
}

#[test]
fn utf8_with_a_byte_order_mark_is_read() -> TestResult {
    let bytes = with_bom(b"\xef\xbb\xbf", SAMPLE.as_bytes().to_vec());
    assert_sample(&IniText::decode(&bytes)?, IniEncoding::Utf8Bom)?;
    assert_sample(&IniText::decode_vec(bytes)?, IniEncoding::Utf8Bom)?;
    Ok(())
}

#[test]
fn utf16_little_endian_with_a_byte_order_mark_is_read() -> TestResult {
    let bytes = with_bom(b"\xff\xfe", utf16(SAMPLE, false));
    assert_sample(&IniText::decode(&bytes)?, IniEncoding::Utf16Le)?;
    assert_sample(&IniText::decode_vec(bytes)?, IniEncoding::Utf16Le)?;
    Ok(())
}

#[test]
fn utf16_big_endian_with_a_byte_order_mark_is_read() -> TestResult {
    let bytes = with_bom(b"\xfe\xff", utf16(SAMPLE, true));
    assert_sample(&IniText::decode(&bytes)?, IniEncoding::Utf16Be)?;
    Ok(())
}

#[test]
fn utf16_without_a_byte_order_mark_is_recognised_by_its_zero_bytes() -> TestResult {
    assert_sample(
        &IniText::decode(&utf16(SAMPLE, false))?,
        IniEncoding::Utf16Le,
    )?;
    assert_sample(
        &IniText::decode(&utf16(SAMPLE, true))?,
        IniEncoding::Utf16Be,
    )?;
    Ok(())
}

#[test]
fn utf16_without_a_byte_order_mark_and_a_blank_first_line_is_recognised() -> TestResult {
    let blank_first = format!("\r\n{SAMPLE}");
    assert_sample(
        &IniText::decode(&utf16(&blank_first, false))?,
        IniEncoding::Utf16Le,
    )?;
    assert_sample(
        &IniText::decode(&utf16(&blank_first, true))?,
        IniEncoding::Utf16Be,
    )?;
    assert_sample(
        &IniText::decode_vec(utf16(&blank_first, false))?,
        IniEncoding::Utf16Le,
    )?;
    Ok(())
}

#[test]
fn valid_utf8_stays_utf8_whatever_it_starts_with() -> TestResult {
    for text in [
        SAMPLE,
        "\u{e4}=1\n",
        "\u{1f680}=1\n",
        "\n\n\u{4e2d}=1\n",
        "a",
    ] {
        let borrowed = IniText::decode(text.as_bytes())?;
        assert_eq!(borrowed.encoding(), IniEncoding::Utf8, "{text:?}");
        assert!(!borrowed.is_lossy(), "{text:?}");
        let owned = IniText::decode_vec(text.as_bytes().to_vec())?;
        assert_eq!(owned.encoding(), IniEncoding::Utf8, "{text:?} owned");
        assert_eq!(owned.text(), text, "{text:?} text");
    }
    Ok(())
}

#[test]
fn broken_utf16_is_replaced_not_rejected() -> TestResult {
    let mut bytes = with_bom(b"\xff\xfe", utf16("a=1\nb=2\n", false));
    bytes.splice(6..8, [0x00, 0xd8]);
    bytes.push(0x41);
    let text = IniText::decode(&bytes)?;
    assert!(text.is_lossy(), "lone surrogate and odd length");
    let ini = text.index()?;
    assert_eq!(ini.resolve("b"), Some("2"), "the rest survives");
    Ok(())
}

#[test]
fn owned_invalid_utf8_is_decoded_lossily() -> TestResult {
    let text = IniText::decode_vec(b"\xef\xbb\xbfa=\xffb\n".to_vec())?;
    assert!(text.is_lossy(), "lossy");
    assert_eq!(text.text(), "a=\u{fffd}b\n", "replaced, BOM gone");
    Ok(())
}

#[test]
fn an_empty_file_has_no_keys() -> TestResult {
    let text = IniText::decode(b"")?;
    assert!(text.index()?.is_empty(), "empty");
    Ok(())
}

#[test]
fn source_and_decoded_budgets_are_independent() -> TestResult {
    let bytes = b"\xef\xbb\xbfa=1";
    let limits = LocalizationLimits::UNLIMITED
        .with_source_bytes(6)
        .with_decoded_bytes(3);
    assert_eq!(
        IniText::decode_with_limits(bytes, limits)?.text(),
        "a=1",
        "BOM belongs to the source size only"
    );
    assert!(
        matches!(
            IniText::decode_with_limits(bytes, limits.with_source_bytes(5)),
            Err(LocalizationError::TooLarge { size: 6, limit: 5 })
        ),
        "encoded size is validated first"
    );
    assert!(
        matches!(
            IniText::decode_vec_with_limits(bytes.to_vec(), limits.with_decoded_bytes(2)),
            Err(LocalizationError::DecodedTooLarge { size: 3, limit: 2 })
        ),
        "the decoded UTF-8 byte count is a separate error"
    );
    Ok(())
}

#[test]
fn transcoding_applies_the_utf8_budget_after_each_decoded_part() -> TestResult {
    let limits = LocalizationLimits::UNLIMITED.with_decoded_bytes(4);
    let bytes = utf16("a=日", false);
    assert!(
        matches!(
            IniText::decode_with_limits(&bytes, limits),
            Err(LocalizationError::DecodedTooLarge { size: 5, limit: 4 })
        ),
        "UTF-16 input expands in the decoded text"
    );
    assert!(
        matches!(
            IniText::decode_vec_with_limits(b"a=\xff".to_vec(), limits),
            Err(LocalizationError::DecodedTooLarge { size: 5, limit: 4 })
        ),
        "a UTF-8 replacement contributes all three bytes"
    );
    assert_eq!(
        IniText::decode_with_limits(&bytes, limits.with_decoded_bytes(5))?.text(),
        "a=日",
        "the exact decoded boundary is admitted"
    );
    Ok(())
}
