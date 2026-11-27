// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Empty};

use p4kx_testkit::{SplitDds, TestTextureFormat};
use p4kx_texture::{DdsAlphaAssociation, DdsHeaderField, DdsRawHeader, SiblingReader, read_mip};

type Check = Result<(), Box<dyn std::error::Error>>;

const LEGACY_FOUR_CC: usize = 84;
const DX10_FLAGS2: usize = 144;

struct NoParts;

impl SiblingReader for NoParts {
    type Part<'a> = Empty;

    fn part_len(&self, _: u32) -> io::Result<Option<u64>> {
        Ok(None)
    }

    fn open_part(&self, _: u32) -> io::Result<Self::Part<'_>> {
        Err(io::ErrorKind::NotFound.into())
    }
}

#[test]
fn legacy_association_uses_the_authored_four_cc() -> Check {
    use DdsAlphaAssociation::{Premultiplied, Straight, Unknown};
    let fixture = SplitDds::new(TestTextureFormat::LegacyDxt5, 4, 4);
    for (four_cc, expected) in [
        (b"DXT1", Unknown),
        (b"DXT2", Premultiplied),
        (b"DXT3", Straight),
        (b"DXT4", Premultiplied),
        (b"DXT5", Straight),
        (b"AAAA", Unknown),
    ] {
        let mut bytes = fixture.header_bytes();
        bytes
            .get_mut(LEGACY_FOUR_CC..LEGACY_FOUR_CC + 4)
            .ok_or("FourCC")?
            .copy_from_slice(four_cc);
        let raw = DdsRawHeader::parse(&bytes)?;
        assert_eq!(raw.alpha_association(), expected, "{four_cc:?}");
        assert_eq!(
            raw.legacy().pixel_format.four_cc,
            *four_cc,
            "exact original FourCC"
        );
        let words: Vec<_> = raw
            .words_for(DdsHeaderField::AlphaAssociation)
            .map(|word| word.offset)
            .collect();
        assert_eq!(words, [80, 84], "legacy flags and FourCC origins");
    }
    Ok(())
}

#[test]
fn dx10_association_retains_reserved_bits_and_original_word() -> Check {
    use DdsAlphaAssociation::{Custom, Opaque, Premultiplied, Reserved, Straight, Unknown};
    for dxgi_format in [74, 77] {
        let fixture = SplitDds::new(
            TestTextureFormat::Dx10Block {
                dxgi_format,
                block_bytes: 16,
            },
            4,
            4,
        );
        for (mode, expected) in [
            Unknown,
            Straight,
            Premultiplied,
            Opaque,
            Custom,
            Reserved,
            Reserved,
            Reserved,
        ]
        .into_iter()
        .enumerate()
        {
            let original = 0x8000_0000_u32 | u32::try_from(mode)?;
            let mut bytes = fixture.header_bytes();
            bytes
                .get_mut(DX10_FLAGS2..DX10_FLAGS2 + 4)
                .ok_or("miscFlags2")?
                .copy_from_slice(&original.to_le_bytes());
            for (header, magic) in [(bytes.as_slice(), 4), (&bytes[4..], 0)] {
                let raw = DdsRawHeader::parse(header)?;
                assert_eq!(raw.alpha_association(), expected, "DX10 mode {mode}");
                assert_eq!(
                    raw.dx10().ok_or("DX10")?.misc_flags2,
                    original,
                    "entire authored word"
                );
                let words: Vec<_> = raw
                    .words_for(DdsHeaderField::AlphaAssociation)
                    .map(|word| word.offset)
                    .collect();
                assert_eq!(
                    words,
                    [80 + magic, 140 + magic],
                    "DX10 FourCC and miscFlags2"
                );
            }
        }
    }
    Ok(())
}

#[test]
fn selected_bc1_counts_only_in_bounds_transparent_selectors() -> Check {
    let fixture = SplitDds::new(TestTextureFormat::LegacyDxt1, 6, 4).mips(2);
    let one_transparent = 3_u32;
    let padded_transparent = one_transparent << 6;
    for (selectors, expected) in [
        (one_transparent, (1, 0, 5)),
        (padded_transparent, (0, 0, 6)),
    ] {
        let block = [0_u32.to_le_bytes(), selectors.to_le_bytes()].concat();
        verify(&fixture, 1, &block, expected)?;
    }
    Ok(())
}

#[test]
fn selected_bc2_and_bc3_share_alpha_decoding_and_ignore_edge_padding() -> Check {
    let bc2 = SplitDds::new(
        TestTextureFormat::Dx10Block {
            dxgi_format: 74,
            block_bytes: 16,
        },
        3,
        2,
    )
    .mips(1);
    let bc3 = SplitDds::new(TestTextureFormat::LegacyDxt5, 3, 2).mips(1);
    let bc2_alpha = [0x80, 0x0f, 0xff, 0x0f, 0, 0, 0, 0];
    let bc3_alpha = [0, 255, 0xA8, 0, 0, 0, 0, 0];
    verify(&bc2, 0, &[bc2_alpha, [0; 8]].concat(), (1, 1, 4))?;
    verify(&bc3, 0, &[bc3_alpha, [0; 8]].concat(), (4, 2, 0))?;
    Ok(())
}

#[test]
fn alpha_coverage_does_not_merge_array_surfaces() -> Check {
    let source = SplitDds::new(
        TestTextureFormat::Dx10Block {
            dxgi_format: 71,
            block_bytes: 8,
        },
        3,
        2,
    )
    .mips(1)
    .array(2);
    let mut bytes = source.header_bytes();
    let header = source.header_len();
    let payload = bytes.get_mut(header..).ok_or("array payload")?;
    let transparent = [0_u32.to_le_bytes(), u32::MAX.to_le_bytes()].concat();
    let opaque = [1_u32.to_le_bytes(), u32::MAX.to_le_bytes()].concat();
    payload.copy_from_slice(&[transparent, opaque].concat());
    let mip = read_mip(bytes.as_slice(), bytes.len() as u64, &NoParts, 0)?;
    let first = mip.alpha_coverage(0).ok_or("first layer")?;
    let second = mip.alpha_coverage(1).ok_or("second layer")?;
    assert_eq!(
        (first.zero, first.full),
        (6, 0),
        "three-colour block has transparent code"
    );
    assert_eq!(
        (second.zero, second.full),
        (0, 6),
        "four-colour block treats selector three as opaque"
    );
    assert_eq!(
        mip.alpha_coverage(2),
        None,
        "only the two original layers are present"
    );
    Ok(())
}

fn verify(fixture: &SplitDds, level: u32, block: &[u8], expected: (u64, u64, u64)) -> Check {
    let mut bytes = fixture.header_bytes();
    let payload = bytes.len() - block.len();
    bytes
        .get_mut(payload..)
        .ok_or("fixture payload")?
        .copy_from_slice(block);
    let mip = read_mip(bytes.as_slice(), bytes.len() as u64, &NoParts, level)?;
    let counted = mip.alpha_coverage(0).ok_or("selected image coverage")?;
    assert_eq!(
        (counted.zero, counted.partial, counted.full),
        expected,
        "in-bounds alpha counts"
    );
    assert_eq!(mip.alpha_coverage(1), None, "there is only one image");
    let decoded = mip.decode()?;
    let count = |accept: fn(u8) -> bool| -> u64 {
        decoded
            .pixels()
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|pixel| accept(pixel[3]))
            .count() as u64
    };
    assert_eq!(
        (
            count(|value| value == 0),
            count(|value| value > 0 && value < 255),
            count(|value| value == 255)
        ),
        expected,
        "matches the real RGBA decoder"
    );
    Ok(())
}
