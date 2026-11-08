// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::path::Path;

use p4kx_extraction::{
    CubemapLayout, DdsHeader, TextureError, TextureImage, TextureImageFace as Face,
    TextureImageFormat as Format, TextureImagePlan,
};
use p4kx_testkit::{SplitDds, TestTextureFormat};

type TestResult = Result<(), Box<dyn Error>>;
const RGBA: TestTextureFormat = TestTextureFormat::Dx10Pixel {
    dxgi_format: 28,
    pixel_bytes: 4,
};

fn plan(
    dds: &SplitDds,
    level: u32,
    layout: CubemapLayout,
) -> Result<Vec<TextureImage>, Box<dyn Error>> {
    let header = DdsHeader::parse(&dds.header_bytes())?;
    Ok(TextureImagePlan::new(&header, level, layout, Format::for_header(&header))?.collect())
}

#[test]
fn plain_and_volume_mips_keep_their_original_coordinates() -> TestResult {
    let plain = SplitDds::new(RGBA, 16, 8);
    let images = plan(&plain, 2, CubemapLayout::Faces)?;
    assert_eq!(images.len(), 1, "one plane");
    let image = images.first().ok_or("missing plane")?;
    assert_eq!(
        (image.level, image.layer, image.face, image.slice),
        (2, 0, Face::Plane, 0),
        "source coordinates"
    );
    assert_eq!(
        (
            image.width,
            image.height,
            image.format,
            image.suffix.as_str()
        ),
        (4, 2, Format::Png, ""),
        "mip shape and encoding"
    );
    let volume = plain.volume(4);
    let images = plan(&volume, 0, CubemapLayout::Faces)?;
    let rows: Vec<_> = images
        .iter()
        .map(|image| (image.slice, image.suffix.as_str()))
        .collect();
    assert_eq!(
        rows,
        [
            (0, "_slice0"),
            (1, "_slice1"),
            (2, "_slice2"),
            (3, "_slice3")
        ],
        "all volume slices"
    );
    let smallest = plan(&volume, 2, CubemapLayout::Faces)?;
    assert_eq!(smallest.len(), 1, "depth shrinks at the selected mip");
    assert_eq!(
        smallest.first().ok_or("missing smallest slice")?.suffix,
        "",
        "one slice is unsuffixed"
    );
    Ok(())
}

#[test]
fn arrays_use_the_existing_zero_padding_and_preserve_the_callers_extension() -> TestResult {
    let images = plan(
        &SplitDds::new(RGBA, 8, 8).array(12),
        1,
        CubemapLayout::Faces,
    )?;
    let names: Vec<_> = images.iter().map(|image| image.suffix.as_str()).collect();
    assert_eq!(
        names,
        [
            "_layer00", "_layer01", "_layer02", "_layer03", "_layer04", "_layer05", "_layer06",
            "_layer07", "_layer08", "_layer09", "_layer10", "_layer11"
        ],
        "every array element"
    );
    let middle = images.get(5).ok_or("missing middle layer")?;
    assert_eq!(
        (middle.layer, middle.level, middle.width, middle.height),
        (5, 1, 4, 4),
        "original layer at mip 1"
    );
    assert_eq!(
        middle.path(Path::new("preview.image")),
        Path::new("preview_layer05.image"),
        "the caller controls the extension"
    );
    Ok(())
}

#[test]
fn cubes_and_cube_arrays_keep_dds_face_order_and_cross_identity() -> TestResult {
    let cube = SplitDds::new(RGBA, 8, 4).cubemap();
    let images = plan(&cube, 0, CubemapLayout::Faces)?;
    let rows: Vec<_> = images
        .iter()
        .map(|image| (image.face, image.suffix.as_str()))
        .collect();
    assert_eq!(
        rows,
        [
            (Face::PositiveX, "_posx"),
            (Face::NegativeX, "_negx"),
            (Face::PositiveY, "_posy"),
            (Face::NegativeY, "_negy"),
            (Face::PositiveZ, "_posz"),
            (Face::NegativeZ, "_negz")
        ],
        "DDS face order"
    );
    let array = cube.array(2);
    let images = plan(&array, 1, CubemapLayout::Faces)?;
    assert_eq!(images.len(), 12, "two complete cubes");
    let boundary = images.get(6).ok_or("missing second cube")?;
    assert_eq!(
        (boundary.layer, boundary.face, boundary.suffix.as_str()),
        (1, Face::PositiveX, "_layer1_posx"),
        "array boundary"
    );
    let crosses = plan(&array, 1, CubemapLayout::Cross)?;
    let rows: Vec<_> = crosses
        .iter()
        .map(|image| {
            (
                image.layer,
                image.face,
                image.width,
                image.height,
                image.suffix.as_str(),
            )
        })
        .collect();
    assert_eq!(
        rows,
        [
            (0, Face::Cross, 16, 6, "_layer0"),
            (1, Face::Cross, 16, 6, "_layer1")
        ],
        "one 4-by-3 cross per cube"
    );
    Ok(())
}

#[test]
fn hdr_selection_does_not_override_explicit_png_previews_or_hide_invalid_mips() -> TestResult {
    let hdr = SplitDds::new(
        TestTextureFormat::Dx10Block {
            dxgi_format: 95,
            block_bytes: 16,
        },
        8,
        8,
    );
    let header = DdsHeader::parse(&hdr.header_bytes())?;
    assert_eq!(
        Format::for_header(&header),
        Format::Exr,
        "ordinary extraction preserves HDR"
    );
    assert_eq!(
        (Format::Exr.extension(), Format::Exr.bit_depth()),
        ("exr", 32),
        "floating-point EXR"
    );
    let preview = TextureImagePlan::new(&header, 0, CubemapLayout::Faces, Format::Png)?
        .next()
        .ok_or("preview")?;
    assert_eq!(
        (preview.format.extension(), preview.format.bit_depth()),
        ("png", 8),
        "an explicit PNG preview stays PNG"
    );
    assert!(
        matches!(
            TextureImagePlan::new(&header, 4, CubemapLayout::Faces, Format::Exr),
            Err(TextureError::NoSuchMip {
                level: 4,
                mip_count: 4
            })
        ),
        "an absent mip retains its typed error"
    );
    Ok(())
}
