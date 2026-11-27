// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;

use p4kx_testkit::{SplitDds, TestTextureFormat};
use p4kx_texture::{DdsRawHeader, SiblingReader, TextureError, read_mip_with_location};

type TestResult = Result<(), Box<dyn std::error::Error>>;

struct Parts<'a>(Option<&'a SplitDds>);

impl SiblingReader for Parts<'_> {
    type Part<'part>
        = io::Cursor<Vec<u8>>
    where
        Self: 'part;

    fn part_len(&self, index: u32) -> io::Result<Option<u64>> {
        Ok(self.0.map(|parts| parts.part(index).len() as u64))
    }

    fn open_part(&self, index: u32) -> io::Result<Self::Part<'_>> {
        let parts = self.0.ok_or(io::ErrorKind::NotFound)?;
        Ok(io::Cursor::new(parts.part(index)))
    }
}

const RGBA8: TestTextureFormat = TestTextureFormat::Dx10Pixel {
    dxgi_format: 29,
    pixel_bytes: 4,
};
const BC5: TestTextureFormat = TestTextureFormat::Dx10Block {
    dxgi_format: 83,
    block_bytes: 16,
};

#[test]
fn selected_dds_keeps_exact_odd_and_subblock_mips_for_all_tested_formats() -> TestResult {
    for (format, pitch) in [
        (TestTextureFormat::LegacyDxt1, [32, 8, 8]),
        (TestTextureFormat::LegacyDxt5, [64, 16, 16]),
        (BC5, [64, 16, 16]),
        (RGBA8, [28, 12, 4]),
    ] {
        let source = SplitDds::new(format, 7, 5).mips(3).split(2);
        for ((level, expected), size) in
            [0, 1, 2]
                .into_iter()
                .zip(pitch)
                .zip([(7, 5), (3, 2), (1, 1)])
        {
            check(&source, level, expected, size)?;
        }
    }
    Ok(())
}

#[test]
fn selected_dds_retains_every_cube_array_layer_and_volume_slice() -> TestResult {
    let cube = SplitDds::new(RGBA8, 8, 8)
        .mips(4)
        .array(2)
        .cubemap()
        .split(1);
    let volume = SplitDds::new(RGBA8, 8, 8).mips(4).volume(8).split(1);
    check(&cube, 0, 32, (8, 8))?;
    check(&cube, 1, 16, (4, 4))?;
    check(&volume, 1, 16, (4, 4))?;
    check(&volume, 3, 4, (1, 1))?;
    Ok(())
}

#[test]
fn a_missing_selected_part_cannot_create_an_asset() {
    let source = SplitDds::new(BC5, 7, 5).mips(3).split(2);
    let bytes = source.header_bytes();
    let error = read_mip_with_location(bytes.as_slice(), bytes.len() as u64, &Parts(None), 0)
        .expect_err("the required source part is absent");
    assert!(
        matches!(error, TextureError::MissingPart { index: 2 }),
        "selected input was missing"
    );
}

fn check(source: &SplitDds, level: u32, pitch: u32, dimensions: (u32, u32)) -> TestResult {
    let bytes = source.header_bytes();
    let original = read_mip_with_location(
        bytes.as_slice(),
        bytes.len() as u64,
        &Parts(Some(source)),
        level,
    )?;
    let mut encoded = Vec::new();
    original.write_dds(&mut encoded)?;
    assert!(encoded.starts_with(b"DDS "), "standard file signature");
    let raw = DdsRawHeader::parse(&encoded)?;
    assert_eq!(
        raw.legacy().pitch_or_linear_size,
        pitch,
        "output pitch or block byte length"
    );
    assert_eq!(
        raw.transfer(),
        original.raw_header.transfer(),
        "authored transfer survives"
    );
    assert_eq!(
        encoded.get(raw.header_len()..),
        Some(source.mip(level).as_slice()),
        "bit-identical selected bytes"
    );
    let roundtrip =
        read_mip_with_location(encoded.as_slice(), encoded.len() as u64, &Parts(None), 0)?;
    let mip = roundtrip.mip;
    assert_eq!(
        (mip.width(), mip.height()),
        dimensions,
        "selected mip dimensions"
    );
    assert_eq!(mip.header().mip_count(), 1, "single authored package level");
    assert_eq!(
        mip.surfaces(),
        original.mip.surfaces(),
        "full surface topology"
    );
    assert_eq!(
        mip.header().faces(),
        original.mip.header().faces(),
        "cube face identity"
    );
    assert_eq!(
        mip.header().array_size(),
        original.mip.header().array_size(),
        "array layer identity"
    );
    assert_eq!(
        mip.header().depth(),
        original.mip.header().slices(level),
        "selected depth"
    );
    Ok(())
}
