// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_texture::{DdsHeader, DdsRawHeader, TextureError, TextureFormat};
use std::error::Error;

type TestResult = Result<(), Box<dyn Error>>;
const FOUR_CC_WORD: usize = 20;
const WIDTH_WORD: usize = 3;

#[test]
fn every_legacy_word_survives_without_defaulting_unknown_bits() -> TestResult {
    let expected = legacy_words();
    let bytes = encode(&expected, &[], true);
    let raw = DdsRawHeader::parse(&bytes)?;
    assert_eq!(
        project_words(&raw),
        expected,
        "all thirty-one original words"
    );
    assert!(raw.magic_present(), "explicit magic is retained");
    assert!(
        raw.dx10().is_none(),
        "legacy header has no invented extension"
    );
    let normalized = DdsHeader::from_raw(&raw)?;
    assert_eq!(raw.legacy().depth, 0, "authored depth is still zero");
    assert_eq!(normalized.depth(), 1, "existing reader depth default");
    assert_eq!(
        raw.legacy().mip_map_count,
        0,
        "authored count is still zero"
    );
    assert_eq!(normalized.mip_count(), 1, "existing reader mip default");
    assert_eq!(
        DdsHeader::parse(&bytes)?,
        normalized,
        "one word reader serves both APIs"
    );
    Ok(())
}

#[test]
fn all_dx10_words_and_zero_array_size_remain_authored() -> TestResult {
    let mut words = legacy_words();
    *words.get_mut(FOUR_CC_WORD).ok_or("FourCC word")? = u32::from_le_bytes(*b"DX10");
    let extension = [u32::MAX, 3, 0x8000_0004, 0, 0xface_0007];
    let bytes = encode(&words, &extension, true);
    let raw = DdsRawHeader::parse(&bytes)?;
    let dx10 = raw.dx10().ok_or("declared extension")?;
    assert_eq!(
        [
            dx10.dxgi_format,
            dx10.resource_dimension,
            dx10.misc_flag,
            dx10.array_size,
            dx10.misc_flags2
        ],
        extension,
        "every extension word is retained"
    );
    let normalized = DdsHeader::from_raw(&raw)?;
    assert_eq!(
        normalized.array_size(),
        1,
        "zero array size retains reader default"
    );
    assert_eq!(
        normalized.faces(),
        6,
        "known cube bit keeps its reader meaning"
    );
    assert!(
        normalized.format().layout().is_none(),
        "unknown format is explicit"
    );
    assert_eq!(raw.header_len(), 148, "actual magic plus extension length");
    let magicless = DdsRawHeader::parse(bytes.get(4..).ok_or("magic boundary")?)?;
    assert!(!magicless.magic_present(), "magicless DX10 is preserved");
    assert_eq!(
        magicless.header_len(),
        144,
        "magicless extended header length"
    );
    assert_eq!(
        magicless.dx10(),
        raw.dx10(),
        "all extension words are independent of magic"
    );
    Ok(())
}

#[test]
fn absent_magic_and_nontext_four_cc_keep_exact_bytes() -> TestResult {
    let mut words = legacy_words();
    let four_cc = [0xff, 0, 0x81, 0x7f];
    *words.get_mut(FOUR_CC_WORD).ok_or("FourCC word")? = u32::from_le_bytes(four_cc);
    let bytes = encode(&words, &[], false);
    let raw = DdsRawHeader::parse(&bytes)?;
    assert!(!raw.magic_present(), "no magic is fabricated");
    assert_eq!(raw.header_len(), 124, "magicless header length");
    assert_eq!(
        raw.legacy().pixel_format.four_cc,
        four_cc,
        "raw bytes are not UTF8 repaired"
    );
    assert!(
        matches!(DdsHeader::from_raw(&raw)?.format(), TextureFormat::UnknownFourCc(value) if value == four_cc),
        "unknown source remains available without a decoder"
    );
    Ok(())
}

#[test]
fn structurally_complete_raw_fields_survive_invalid_topology() -> TestResult {
    let mut words = legacy_words();
    *words.get_mut(WIDTH_WORD).ok_or("width word")? = 0;
    let raw = DdsRawHeader::parse(&encode(&words, &[], true))?;
    assert_eq!(
        raw.legacy().width,
        0,
        "invalid authored width remains observable"
    );
    assert!(
        matches!(
            DdsHeader::from_raw(&raw),
            Err(TextureError::InvalidHeader(_))
        ),
        "topology refusal stays separate from raw capture"
    );
    Ok(())
}

#[test]
fn truncated_extension_and_bad_size_remain_real_reader_errors() -> TestResult {
    let mut words = legacy_words();
    *words.get_mut(FOUR_CC_WORD).ok_or("FourCC word")? = u32::from_le_bytes(*b"DX10");
    let bytes = encode(&words, &[99, 3, 0, 1, 7], true);
    for size in [4, 127, 128, 147] {
        let prefix = bytes.get(..size).ok_or("prefix boundary")?;
        assert!(
            matches!(
                DdsRawHeader::parse(prefix),
                Err(TextureError::TruncatedHeader { .. })
            ),
            "incomplete bytes never become complete metadata"
        );
    }
    *words.first_mut().ok_or("size word")? = 120;
    assert!(
        matches!(
            DdsRawHeader::parse(&encode(&words, &[], true)),
            Err(TextureError::InvalidHeader(_))
        ),
        "unsupported header size is not guessed"
    );
    Ok(())
}

const fn legacy_words() -> [u32; 31] {
    [
        124,
        0x4000_1007,
        8,
        16,
        0xfefd_fcfb,
        0,
        0,
        1,
        2,
        3,
        4,
        5,
        6,
        7,
        8,
        9,
        10,
        u32::MAX,
        32,
        0x8000_0004,
        u32::from_le_bytes(*b"DXT1"),
        u32::MAX,
        0x1234_5678,
        0x2345_6789,
        0x3456_789a,
        0x4567_89ab,
        u32::MAX,
        0x8000_0000,
        0x5678_9abc,
        0x6789_abcd,
        0x789a_bcde,
    ]
}

fn encode(legacy: &[u32], dx10: &[u32], magic: bool) -> Vec<u8> {
    let mut bytes = if magic { b"DDS ".to_vec() } else { Vec::new() };
    for word in legacy.iter().chain(dx10) {
        bytes.extend_from_slice(&word.to_le_bytes());
    }
    bytes
}

fn project_words(raw: &DdsRawHeader) -> Vec<u32> {
    let value = raw.legacy();
    let pixel = value.pixel_format;
    let mut words = vec![
        value.size,
        value.flags,
        value.height,
        value.width,
        value.pitch_or_linear_size,
        value.depth,
        value.mip_map_count,
    ];
    words.extend_from_slice(&value.reserved1);
    words.extend_from_slice(&[
        pixel.size,
        pixel.flags,
        u32::from_le_bytes(pixel.four_cc),
        pixel.rgb_bit_count,
        pixel.r_bit_mask,
        pixel.g_bit_mask,
        pixel.b_bit_mask,
        pixel.a_bit_mask,
        value.caps,
        value.caps2,
        value.caps3,
        value.caps4,
        value.reserved2,
    ]);
    words
}
