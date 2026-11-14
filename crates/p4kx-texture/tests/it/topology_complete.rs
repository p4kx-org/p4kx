// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::io;

use p4kx_testkit::{SplitDds, TestTextureFormat};
use p4kx_texture::{self as texture, DdsHeader, DdsRawHeader, SiblingReader, TextureError};

type TestResult = Result<(), Box<dyn Error>>;
const IMAGES: u32 = 2049;
const DX10_ARRAY_SIZE_OFFSET: usize = 140;

struct NoSiblings;

impl SiblingReader for NoSiblings {
    type Part<'part> = &'part [u8];

    fn part_len(&self, _: u32) -> io::Result<Option<u64>> {
        Ok(None)
    }

    fn open_part(&self, _: u32) -> io::Result<Self::Part<'_>> {
        Err(io::ErrorKind::NotFound.into())
    }
}

#[test]
fn every_readable_array_element_beyond_a_device_limit_is_retained() -> TestResult {
    complete(
        &SplitDds::new(TestTextureFormat::R8, 1, 1)
            .array(IMAGES)
            .mips(1),
        3,
    )
}

#[test]
fn every_readable_volume_slice_beyond_a_device_limit_is_retained() -> TestResult {
    complete(
        &SplitDds::new(TestTextureFormat::R8, 1, 1)
            .volume(IMAGES)
            .mips(1),
        4,
    )
}

fn complete(source: &SplitDds, dimension: u32) -> TestResult {
    let bytes = source.header_bytes();
    let raw = DdsRawHeader::parse(&bytes)?;
    let extended = raw.dx10().ok_or("the actual original DX10 declaration")?;
    assert_eq!(
        extended.resource_dimension, dimension,
        "the authored topology kind"
    );
    assert_eq!(raw.legacy().width, 1, "the actual one-pixel width");
    assert_eq!(raw.legacy().height, 1, "the actual one-pixel height");
    assert_eq!(raw.legacy().mip_map_count, 1, "one physical source mip");
    if dimension == 4 {
        assert_eq!(raw.legacy().depth, IMAGES, "the original volume count");
        assert_eq!(extended.array_size, 1, "a volume is not an array");
    } else {
        assert_eq!(extended.array_size, IMAGES, "the original array count");
    }
    let expected = source.mip(0);
    assert_eq!(
        expected.len(),
        usize::try_from(IMAGES)?,
        "one actual byte per original image"
    );
    assert_eq!(
        bytes.get(raw.header_len()..),
        Some(expected.as_slice()),
        "all declared payload bytes exist"
    );
    let mip = texture::read_mip(
        bytes.as_slice(),
        u64::try_from(bytes.len())?,
        &NoSiblings,
        0,
    )?;
    assert_eq!(
        u128::from(mip.surfaces()),
        u128::from(IMAGES),
        "the full original cardinality"
    );
    assert_eq!(
        mip.data(),
        expected,
        "every physical byte remains in image order"
    );
    assert_eq!(
        mip.surface(2048),
        expected.get(2048..2049),
        "the final original image is addressable"
    );
    Ok(())
}

#[test]
fn cube_array_cardinality_is_exact_before_payload_admission() -> TestResult {
    const CUBES: u32 = u32::MAX / 6 + 1;
    let mut bytes = SplitDds::new(TestTextureFormat::R8, 1, 1)
        .cubemap()
        .mips(1)
        .header_bytes();
    let array_word = bytes
        .get_mut(DX10_ARRAY_SIZE_OFFSET..DX10_ARRAY_SIZE_OFFSET + size_of::<u32>())
        .ok_or("the actual array count word")?;
    array_word.copy_from_slice(&CUBES.to_le_bytes());
    let raw = DdsRawHeader::parse(&bytes)?;
    let extended = raw.dx10().ok_or("the original DX10 cube declaration")?;
    assert_eq!(extended.array_size, CUBES, "the actual authored cube count");
    assert_eq!(extended.resource_dimension, 3, "cubes are 2D textures");
    assert_eq!(extended.misc_flag & 4, 4, "the original cube flag");
    assert_eq!(
        bytes.len(),
        raw.header_len() + 6,
        "only one small cube payload exists"
    );
    let header = DdsHeader::from_raw(&raw)?;
    let images = u128::from(CUBES) * 6;
    assert_eq!(
        u128::from(header.surfaces()),
        images,
        "no saturated surface count"
    );
    assert_eq!(
        u128::from(header.images(0)),
        images,
        "no saturated image count"
    );
    assert_eq!(
        header.mip_size(0).map(u128::from),
        Some(images),
        "exact original byte extent"
    );
    assert!(
        matches!(
            header.part_count(u64::try_from(bytes.len())?),
            Err(TextureError::TailMismatch { .. })
        ),
        "an incomplete body is refused independently of valid header metadata"
    );
    Ok(())
}
