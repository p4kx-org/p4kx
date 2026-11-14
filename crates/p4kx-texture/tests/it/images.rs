// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Volume and array textures: one image per slice or element, the sheet they are previewed on,
//! the bounds of untrusted headers and the formats `R16G16_FLOAT` and `R32_UINT`.

use std::collections::BTreeMap;
use std::error::Error;
use std::io;

use p4kx_testkit::{SplitDds, TestTextureFormat};
use p4kx_texture::{
    DdsHeader, DdsRawHeader, DxgiFormat, Mip, RgbaImage, SiblingReader, TextureError,
    TextureFormat, decode, read_mip, sheet_columns, tile,
};

struct Parts(BTreeMap<u32, Vec<u8>>);

impl Parts {
    fn of(dds: &SplitDds) -> Self {
        Self(
            (1..=dds.part_count())
                .map(|index| (index, dds.part(index)))
                .collect(),
        )
    }
}

impl SiblingReader for Parts {
    type Part<'a> = &'a [u8];

    fn part_len(&self, index: u32) -> io::Result<Option<u64>> {
        Ok(self.0.get(&index).map(|data| data.len() as u64))
    }

    fn open_part(&self, index: u32) -> io::Result<Self::Part<'_>> {
        self.0
            .get(&index)
            .map(Vec::as_slice)
            .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))
    }
}

fn read(dds: &SplitDds, level: u32) -> Result<Mip, TextureError> {
    let header = dds.header_bytes();
    read_mip(
        header.as_slice(),
        header.len() as u64,
        &Parts::of(dds),
        level,
    )
}

fn header_of(dds: &SplitDds) -> Result<DdsHeader, TextureError> {
    DdsHeader::parse(&dds.header_bytes())
}

const fn dxgi(code: u32) -> TextureFormat {
    TextureFormat::Dxgi(DxgiFormat(code))
}

#[test]
fn a_volume_mip_holds_one_image_per_slice_and_the_slices_halve_per_level()
-> Result<(), Box<dyn Error>> {
    let dds = SplitDds::new(TestTextureFormat::LegacyBgra8, 8, 8)
        .volume(4)
        .split(2);

    for (level, slices) in [(0, 4), (1, 2), (2, 1), (3, 1)] {
        let mip = read(&dds, level)?;
        assert_eq!(mip.surfaces(), slices, "slices of mip {level}");
        assert_eq!(mip.data(), dds.mip(level).as_slice(), "data of mip {level}");
        let slice_len = mip.data().len() / usize::try_from(slices)?;
        for slice in 0..slices {
            let start = usize::try_from(slice)? * slice_len;
            assert_eq!(
                mip.surface(slice),
                mip.data().get(start..start + slice_len),
                "slice {slice} of mip {level}"
            );
        }
    }
    Ok(())
}

#[test]
fn every_slice_of_a_volume_decodes_to_an_image_of_the_mips_size() -> Result<(), TextureError> {
    let dds = SplitDds::new(TestTextureFormat::LegacyBgra8, 4, 4)
        .volume(3)
        .mips(1);
    let mip = read(&dds, 0)?;

    let slices = mip.decode_surfaces()?;

    assert_eq!(slices.len(), 3, "three slices");
    let data = dds.surface_mip(0, 0);
    for (index, image) in slices.iter().enumerate() {
        assert_eq!((image.width(), image.height()), (4, 4), "slice {index}");
        let first = data.get(index * 64..index * 64 + 4).unwrap_or_default();
        assert_eq!(
            image.pixels().get(..4),
            Some([first[2], first[1], first[0], first[3]].as_slice()),
            "the first pixel of slice {index}"
        );
    }
    assert!(
        matches!(
            mip.decode(),
            Err(TextureError::UnsupportedImage { images: 3 })
        ),
        "a volume is not one image"
    );
    Ok(())
}

#[test]
fn an_array_mip_holds_one_image_per_element_from_its_part_or_the_header() -> Result<(), TextureError>
{
    let dds = SplitDds::new(TestTextureFormat::R8, 8, 8).array(3).split(2);

    for level in 0..4 {
        let mip = read(&dds, level)?;
        assert_eq!(mip.surfaces(), 3, "three elements of mip {level}");
        for element in 0..3 {
            assert_eq!(
                mip.surface(u64::from(element)),
                Some(dds.surface_mip(element, level).as_slice()),
                "element {element} of mip {level}"
            );
        }
        let layers = mip.decode_surfaces()?;
        assert_eq!(layers.len(), 3, "three decoded layers of mip {level}");
    }
    Ok(())
}

#[test]
fn a_volume_array_and_cubemap_array_count_their_images() -> Result<(), TextureError> {
    let volume = header_of(&SplitDds::new(TestTextureFormat::BC7_SRGB, 16, 16).volume(8))?;
    let array = header_of(&SplitDds::new(TestTextureFormat::BC7_SRGB, 16, 16).array(5))?;
    let cubes = header_of(
        &SplitDds::new(TestTextureFormat::BC7_SRGB, 16, 16)
            .cubemap()
            .array(2),
    )?;

    assert_eq!(
        [volume.images(0), volume.images(1), volume.images(3)],
        [8, 4, 1],
        "volume"
    );
    assert_eq!(volume.slices(9), 1, "never below one slice");
    assert_eq!(array.images(2), 5, "array");
    assert_eq!(cubes.images(0), 12, "two cubemaps of six faces");
    Ok(())
}

#[test]
fn sheets_are_the_smallest_square_that_holds_the_images() {
    let columns: Vec<u64> = [1, 2, 3, 4, 5, 9, 10, 86, 2048]
        .into_iter()
        .map(sheet_columns)
        .collect();

    assert_eq!(columns, [1, 2, 2, 2, 3, 3, 4, 10, 46], "columns");
}

#[test]
fn the_preview_of_a_texture_is_its_mip_cross_or_sheet() -> Result<(), TextureError> {
    let plain = header_of(&SplitDds::new(TestTextureFormat::BC7_SRGB, 32, 16))?;
    let array = header_of(&SplitDds::new(TestTextureFormat::BC7_SRGB, 32, 16).array(5))?;
    let volume = header_of(&SplitDds::new(TestTextureFormat::BC7_SRGB, 32, 16).volume(8))?;
    let cube = header_of(&SplitDds::new(TestTextureFormat::BC7_SRGB, 32, 16).cubemap())?;
    let cubes = header_of(
        &SplitDds::new(TestTextureFormat::BC7_SRGB, 32, 16)
            .cubemap()
            .array(2),
    )?;

    assert_eq!(plain.preview_size(1), Some([16, 8]), "plain");
    assert_eq!(array.preview_size(0), Some([96, 32]), "3 × 2 of 32 × 16");
    assert_eq!(volume.preview_size(0), Some([96, 48]), "3 × 3 of 8 slices");
    assert_eq!(volume.preview_size(1), Some([32, 16]), "2 × 2 of 4 slices");
    assert_eq!(cube.preview_size(0), Some([128, 48]), "the cross");
    assert_eq!(cubes.preview_size(0), Some([256, 48]), "two crosses");
    assert_eq!(plain.preview_size(9), None, "no such level");
    assert_eq!(array.preview_units(0), 5, "five elements");
    assert_eq!(cubes.preview_units(0), 2, "two crosses");
    Ok(())
}

fn flat(width: u32, height: u32, colour: u8) -> Result<RgbaImage, TextureError> {
    RgbaImage::new(width, height, vec![colour; (width * height * 4) as usize])
}

#[test]
fn images_are_tiled_row_by_row_with_transparent_cells_left_over() -> Result<(), TextureError> {
    let images = [flat(2, 1, 10)?, flat(2, 1, 20)?, flat(2, 1, 30)?];

    let sheet = tile(&images, 2)?;

    assert_eq!(
        (sheet.width(), sheet.height()),
        (4, 2),
        "two columns, two rows"
    );
    let row = |y: usize| sheet.pixels().get(y * 16..(y + 1) * 16).unwrap_or_default();
    assert_eq!(
        row(0),
        [[10; 8], [20; 8]].concat(),
        "the first row holds images 0 and 1"
    );
    assert_eq!(
        row(1),
        [[30; 8], [0; 8]].concat(),
        "the second row holds image 2 and an empty cell"
    );
    Ok(())
}

#[test]
fn a_sheet_refuses_no_images_no_columns_and_images_of_two_sizes() -> Result<(), TextureError> {
    let mixed = [flat(2, 1, 1)?, flat(1, 2, 1)?];

    assert!(
        matches!(tile(&[], 1), Err(TextureError::SheetMismatch)),
        "no images"
    );
    assert!(
        matches!(tile(&mixed[..1], 0), Err(TextureError::SheetMismatch)),
        "no columns"
    );
    assert!(
        matches!(tile(&mixed, 2), Err(TextureError::SheetMismatch)),
        "two sizes"
    );
    Ok(())
}

#[test]
fn large_source_topologies_keep_exact_readable_mip_counts() -> Result<(), TextureError> {
    let volume = SplitDds::new(TestTextureFormat::LegacyBgra8, 1, 1)
        .volume(4096)
        .mips(1);
    let array = SplitDds::new(TestTextureFormat::R8, 1, 1)
        .array(4096)
        .mips(1);

    for (name, dds) in [("volume", volume), ("array", array)] {
        let bytes = dds.header_bytes();
        let raw = DdsRawHeader::parse(&bytes)?;
        let original = raw
            .dx10()
            .map_or_else(|| raw.legacy().depth, |extended| extended.array_size);
        assert_eq!(original, 4096, "{name} keeps the same actual source count");
        let header = header_of(&dds)?;
        assert_eq!(
            header.images(0),
            4096,
            "{name} has no device-derived source cap"
        );
        assert_eq!(
            header.mip_size(0),
            Some(dds.mip(0).len() as u64),
            "the exact physical byte extent"
        );
        let mip = read_mip(bytes.as_slice(), bytes.len() as u64, &Parts::of(&dds), 0)?;
        assert_eq!(mip.data(), dds.mip(0), "{name} retains every source byte");
    }
    Ok(())
}

#[test]
fn a_volume_that_is_also_a_cubemap_or_an_array_is_invalid() {
    let cube = SplitDds::new(TestTextureFormat::BC7_SRGB, 8, 8)
        .volume(4)
        .cubemap();
    let array = SplitDds::new(TestTextureFormat::BC7_SRGB, 8, 8)
        .volume(4)
        .array(2);

    for (name, dds) in [("cubemap", cube), ("array", array)] {
        let result = header_of(&dds);
        assert!(
            matches!(result, Err(TextureError::InvalidHeader(_))),
            "{name}: {result:?}"
        );
    }
}

#[test]
fn an_array_header_larger_than_its_data_is_a_mismatch_not_an_allocation() {
    let dds = SplitDds::new(TestTextureFormat::R8, 4, 4)
        .array(2000)
        .mips(1);
    let header = dds.header_bytes();
    let short = header.get(..header.len() - 3000).unwrap_or_default();

    let result = read_mip(short, short.len() as u64, &Parts::of(&dds), 0);

    assert!(
        matches!(result, Err(TextureError::TailMismatch { .. })),
        "{result:?}"
    );
}

#[test]
fn r16g16_float_decodes_to_red_and_green() -> Result<(), TextureError> {
    let half = |value: u16| value.to_le_bytes();
    let data = [half(0x3C00), half(0x3800), half(0x0000), half(0x4000)].concat();

    let image = decode(dxgi(34), 2, 1, &data)?;

    assert_eq!(
        image.pixels(),
        [188, 156, 0, 255, 0, 213, 0, 255],
        "1.0 and 0.5, then 0 and 2.0, tone-mapped"
    );
    Ok(())
}

#[test]
fn r32_uint_is_scaled_by_its_largest_value() -> Result<(), TextureError> {
    let data = [0_u32, u32::MAX / 2, u32::MAX]
        .map(u32::to_le_bytes)
        .concat();

    let image = decode(dxgi(42), 3, 1, &data)?;

    assert_eq!(
        image.pixels(),
        [0, 0, 0, 255, 127, 127, 127, 255, 255, 255, 255, 255],
        "black, just under half, white"
    );
    assert!(dxgi(42).is_decodable(), "R32_UINT has a decoder");
    assert!(dxgi(34).is_decodable(), "R16G16_FLOAT has a decoder");
    Ok(())
}
