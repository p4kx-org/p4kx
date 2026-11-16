// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Array and volume textures: one PNG per element or slice, with the alpha tail merged per image.

use std::error::Error;

use p4kx_extraction::{ConflictPolicy, CubemapLayout, Texture};
use p4kx_testkit::{SplitDds, TestTextureFormat};

use super::fixture::{Fixture, HEADER, files, read_png, run};

type TestResult = Result<(), Box<dyn Error>>;

const RGBA8: TestTextureFormat = TestTextureFormat::Dx10Pixel {
    dxgi_format: 28,
    pixel_bytes: 4,
};

#[test]
fn a_texture_named_like_an_element_of_an_array_is_written_after_it() -> TestResult {
    let array = SplitDds::new(RGBA8, 8, 8).array(2).mips(1);
    let plain = SplitDds::new(TestTextureFormat::LegacyBgra8, 4, 4).mips(1);
    let mut entries = array.entries(HEADER);
    entries.extend(plain.entries("Data\\Textures\\rock_diff_layer0.dds"));
    let fixture = Fixture::new(entries)?;

    let report = run(&fixture, ConflictPolicy::Overwrite);

    assert!(report.is_complete(), "{report:?}");
    let (width, height, _) = read_png(&fixture.target.join("Data/Textures/rock_diff_layer0.png"))?;
    assert_eq!(
        (width, height),
        (4, 4),
        "the later texture replaces the element, never a mix of both"
    );
    Ok(())
}

#[test]
fn an_array_is_exported_as_one_png_per_element() -> TestResult {
    let dds = SplitDds::new(RGBA8, 8, 8).array(3).split(2);
    let fixture = Fixture::new(dds.entries(HEADER))?;

    let report = run(&fixture, ConflictPolicy::Skip);

    assert!(report.is_complete(), "{report:?}");
    assert_eq!(
        files(&fixture.target)?,
        [
            "Data/Textures/rock_diff.dds",
            "Data/Textures/rock_diff.dds.p4kx-origin.jsonl",
            "Data/Textures/rock_diff_layer0.png",
            "Data/Textures/rock_diff_layer1.png",
            "Data/Textures/rock_diff_layer2.png",
        ],
        "the merged DDS and three elements"
    );
    let layers = Texture::from_archive(&fixture.archive, HEADER)?.decode_surfaces(0)?;
    assert_eq!(layers.len(), 3, "three decoded elements");
    for (index, image) in layers.into_iter().enumerate() {
        let path = fixture
            .target
            .join(format!("Data/Textures/rock_diff_layer{index}.png"));
        let (width, height, pixels) = read_png(&path)?;
        assert_eq!((width, height), (8, 8), "element {index}");
        assert_eq!(pixels, image.into_pixels(), "element {index}");
    }
    assert_eq!(
        fixture.read("Data/Textures/rock_diff.dds")?,
        dds.merged_bytes(),
        "merged"
    );
    Ok(())
}

#[test]
fn a_volume_is_exported_as_one_png_per_slice() -> TestResult {
    let dds = SplitDds::new(TestTextureFormat::LegacyBgra8, 4, 4)
        .volume(4)
        .split(1);
    let fixture = Fixture::new(dds.entries(HEADER))?;

    let report = run(&fixture, ConflictPolicy::Skip);

    assert!(report.is_complete(), "{report:?}");
    assert_eq!(
        files(&fixture.target)?,
        [
            "Data/Textures/rock_diff.dds",
            "Data/Textures/rock_diff.dds.p4kx-origin.jsonl",
            "Data/Textures/rock_diff_slice0.png",
            "Data/Textures/rock_diff_slice1.png",
            "Data/Textures/rock_diff_slice2.png",
            "Data/Textures/rock_diff_slice3.png",
        ],
        "the merged DDS and four slices"
    );
    let slices = Texture::from_archive(&fixture.archive, HEADER)?.decode_surfaces(0)?;
    for (index, image) in slices.into_iter().enumerate() {
        let path = fixture
            .target
            .join(format!("Data/Textures/rock_diff_slice{index}.png"));
        assert_eq!(
            read_png(&path)?.2,
            image.into_pixels(),
            "slice {index} of the volume"
        );
    }
    Ok(())
}

#[test]
fn the_alpha_tail_of_an_array_is_merged_per_element() -> TestResult {
    let dds = SplitDds::new(RGBA8, 4, 4)
        .array(2)
        .split(1)
        .alpha(TestTextureFormat::R8);
    let tail = dds.alpha_tail().ok_or("a tail")?;
    let fixture = Fixture::new(dds.entries(HEADER))?;

    let report = run(&fixture, ConflictPolicy::Skip);

    assert!(report.is_complete(), "{report:?}");
    for element in 0..2 {
        let path = fixture
            .target
            .join(format!("Data/Textures/rock_diff_layer{element}.png"));
        let (_, _, pixels) = read_png(&path)?;
        let alpha: Vec<u8> = pixels.iter().skip(3).step_by(4).copied().collect();
        assert_eq!(
            alpha,
            tail.surface_mip(element, 0),
            "the alpha of element {element} is its own tail image"
        );
    }
    Ok(())
}

#[test]
fn a_cubemap_array_is_exported_as_faces_or_as_one_cross_per_element() -> TestResult {
    let dds = SplitDds::new(RGBA8, 4, 4).cubemap().array(2).mips(1);
    let fixture = Fixture::new(dds.entries(HEADER))?;
    let texture = Texture::from_archive(&fixture.archive, HEADER)?;

    let faces = texture.png_to_file(&fixture.target.join("faces.png"), 0)?;
    let crosses =
        texture.png_to_file_as(&fixture.target.join("cross.png"), 0, CubemapLayout::Cross)?;

    assert_eq!(faces.files.len(), 12, "two elements of six faces");
    assert!(
        faces
            .files
            .iter()
            .any(|file| file.ends_with("faces_layer1_negz.png")),
        "{:?}",
        faces.files
    );
    assert_eq!(
        crosses
            .files
            .iter()
            .filter_map(|file| file.file_name()?.to_str())
            .collect::<Vec<_>>(),
        ["cross_layer0.png", "cross_layer1.png"],
        "one cross per element"
    );
    assert_eq!(
        (crosses.width, crosses.height),
        (16, 12),
        "four by three faces"
    );
    Ok(())
}

#[test]
fn a_volume_halves_its_slices_per_mip() -> TestResult {
    let dds = SplitDds::new(TestTextureFormat::LegacyBgra8, 8, 8).volume(4);
    let fixture = Fixture::new(dds.entries(HEADER))?;
    let texture = Texture::from_archive(&fixture.archive, HEADER)?;

    let report = texture.png_to_file(&fixture.target.join("mip.png"), 1)?;

    assert_eq!(report.files.len(), 2, "two slices at mip 1");
    assert_eq!((report.width, report.height), (4, 4), "mip 1");
    Ok(())
}
