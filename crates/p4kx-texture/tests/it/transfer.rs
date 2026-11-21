// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_testkit::{SplitDds, TestTextureFormat};
use p4kx_texture::{DdsHeaderField, DdsRawHeader, DdsTransfer};

const FLAGS: usize = 36;
const MARKER: usize = 124;
const DXGI: usize = 128;
const CRY_MARKER: u32 = u32::from_le_bytes(*b"FYRC");

type TestResult = Result<(), Box<dyn Error>>;

fn set(bytes: &mut [u8], offset: usize, value: u32) -> TestResult {
    bytes
        .get_mut(offset..offset + 4)
        .ok_or("header word")?
        .copy_from_slice(&value.to_le_bytes());
    Ok(())
}

#[test]
fn legacy_transfer_requires_the_exact_cry_marker() -> TestResult {
    let mut bytes = SplitDds::new(TestTextureFormat::LegacyDxt1, 8, 8)
        .mips(1)
        .header_bytes();
    set(&mut bytes, FLAGS, 0x800 | 0x20_0000)?;
    let unmarked = DdsRawHeader::parse(&bytes)?.transfer();
    assert_eq!(
        unmarked.cry,
        DdsTransfer::Unknown,
        "without the marker the flag word is uninterpreted"
    );
    assert_eq!(
        unmarked.effective,
        DdsTransfer::Unknown,
        "legacy FourCC does not establish transfer"
    );
    assert_eq!(
        unmarked.cry_image_flags, 0x20_0800,
        "uninterpreted bits survive"
    );
    set(&mut bytes, MARKER, CRY_MARKER)?;
    let marked = DdsRawHeader::parse(&bytes)?.transfer();
    assert_eq!(marked.cry_marker, CRY_MARKER, "original marker word");
    assert_eq!(marked.effective, DdsTransfer::Srgb, "valid marker and bit");
    set(&mut bytes, FLAGS, 0x20_0000)?;
    let linear = DdsRawHeader::parse(&bytes)?.transfer();
    assert_eq!(
        linear.effective,
        DdsTransfer::Linear,
        "valid marker with the bit clear"
    );
    assert!(!linear.conflict, "legacy supplies only one declaration");
    Ok(())
}

#[test]
fn explicit_dxgi_and_cry_declarations_stay_independent() -> TestResult {
    use DdsTransfer::{Linear, Srgb, Unknown};

    for (format, marker, flags, dxgi, cry, effective, conflict) in [
        (99, 0, 0, Srgb, Unknown, Srgb, false),
        (98, 0, 0, Linear, Unknown, Linear, false),
        (99, CRY_MARKER, 0, Srgb, Linear, Unknown, true),
        (98, CRY_MARKER, 0x800, Linear, Srgb, Unknown, true),
        (98, CRY_MARKER, 0, Linear, Linear, Linear, false),
        (99, CRY_MARKER, 0x800, Srgb, Srgb, Srgb, false),
        (97, 0, 0, Unknown, Unknown, Unknown, false),
        (u32::MAX, 0, 0x800, Unknown, Unknown, Unknown, false),
        (57, 0, 0, Unknown, Unknown, Unknown, false),
        (54, 0, 0, Linear, Unknown, Linear, false),
    ] {
        let mut bytes = SplitDds::new(TestTextureFormat::BC7_SRGB, 8, 8)
            .mips(1)
            .header_bytes();
        set(&mut bytes, DXGI, format)?;
        set(&mut bytes, FLAGS, flags)?;
        set(&mut bytes, MARKER, marker)?;
        let transfer = DdsRawHeader::parse(&bytes)?.transfer();
        assert_eq!(
            (
                transfer.dxgi,
                transfer.cry,
                transfer.effective,
                transfer.conflict
            ),
            (dxgi, cry, effective, conflict),
            "format {format}, marker {marker}, flags {flags}",
        );
    }
    Ok(())
}

#[test]
fn transfer_words_follow_both_dds_header_profiles() -> TestResult {
    let mut bytes = SplitDds::new(TestTextureFormat::BC7_SRGB, 8, 8)
        .mips(1)
        .header_bytes();
    for (prefix, shift) in [(0, 0), (4, 4)] {
        let raw = DdsRawHeader::parse(bytes.get(prefix..).ok_or("DDS prefix")?)?;
        for (field, expected) in [
            (DdsHeaderField::DxgiTransfer, vec![84 - shift, 128 - shift]),
            (DdsHeaderField::CryImageMarker, vec![124 - shift]),
            (DdsHeaderField::CryImageFlags, vec![36 - shift]),
            (DdsHeaderField::CryTransfer, vec![124 - shift]),
        ] {
            assert_eq!(
                offsets(&raw, field),
                expected,
                "{field:?} at magic shift {shift}"
            );
        }
    }
    set(&mut bytes, MARKER, CRY_MARKER)?;
    assert_eq!(
        offsets(&DdsRawHeader::parse(&bytes)?, DdsHeaderField::CryTransfer),
        [124, 36],
        "a recognised marker makes the flags relevant",
    );
    Ok(())
}

fn offsets(raw: &DdsRawHeader, field: DdsHeaderField) -> Vec<u64> {
    raw.words_for(field).map(|word| word.offset).collect()
}
