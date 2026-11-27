// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::path::Path;

use exr::prelude::{ReadChannels, ReadLayers, Vec2};
use p4kx_extraction::{CUBE_FACE_NAMES, ConflictPolicy, Texture};
use p4kx_testkit::{SplitDds, TestTextureFormat};

use super::fixture::{Fixture, HEADER, files, run};

type TestResult = Result<(), Box<dyn Error>>;

const BC6H_UF16: TestTextureFormat = TestTextureFormat::Dx10Block {
    dxgi_format: 95,
    block_bytes: 16,
};

fn read_exr(path: &Path) -> Result<Vec<f32>, Box<dyn Error>> {
    let image = exr::prelude::read()
        .no_deep_data()
        .largest_resolution_level()
        .rgba_channels(
            |resolution, _| (resolution, vec![0.0_f32; resolution.area() * 3]),
            |(resolution, pixels): &mut (Vec2<usize>, Vec<f32>),
             position: Vec2<usize>,
             (r, g, b, _): (f32, f32, f32, f32)| {
                let start = (position.y() * resolution.width() + position.x()) * 3;
                if let Some(slot) = pixels.get_mut(start..start + 3) {
                    slot.copy_from_slice(&[r, g, b]);
                }
            },
        )
        .first_valid_layer()
        .all_attributes()
        .from_file(path)?;
    Ok(image.layer_data.channel_data.pixels.1)
}

#[test]
fn a_high_range_texture_is_exported_as_openexr_with_its_range() -> TestResult {
    let dds = SplitDds::new(BC6H_UF16, 32, 16).split(2);
    let fixture = Fixture::new(dds.entries(HEADER))?;

    let report = run(&fixture, ConflictPolicy::Skip);

    assert!(report.is_complete(), "{report:?}");
    assert_eq!(report.converted(), Some(3), "header and two parts");
    assert_eq!(
        files(&fixture.target)?,
        [
            "Data/Textures/rock_diff.dds",
            "Data/Textures/rock_diff.dds.p4kx-origin.jsonl",
            "Data/Textures/rock_diff.exr",
        ],
        "the merged DDS and OpenEXR instead of PNG"
    );
    let expected = Texture::from_archive(&fixture.archive, HEADER)?.decode_mip_hdr(0)?;
    assert_eq!(
        read_exr(&fixture.target.join("Data/Textures/rock_diff.exr"))?,
        expected.pixels(),
        "the decoded values, unclipped"
    );
    let again = run(&fixture, ConflictPolicy::IfChanged);
    assert_eq!(
        again.skipped().len(),
        3,
        "identical outputs are kept: {again:?}"
    );
    Ok(())
}

#[test]
fn a_high_range_cubemap_is_exported_as_six_openexr_faces() -> TestResult {
    let dds = SplitDds::new(BC6H_UF16, 8, 8).cubemap().split(1);
    let fixture = Fixture::new(dds.entries(HEADER))?;

    let report = run(&fixture, ConflictPolicy::Skip);

    assert!(report.is_complete(), "{report:?}");
    let mut expected = vec![
        "Data/Textures/rock_diff.dds".to_owned(),
        "Data/Textures/rock_diff.dds.p4kx-origin.jsonl".to_owned(),
    ];
    expected.extend(
        CUBE_FACE_NAMES
            .iter()
            .map(|face| format!("Data/Textures/rock_diff_{face}.exr")),
    );
    expected.sort();
    assert_eq!(files(&fixture.target)?, expected, "the DDS and six faces");
    let faces = Texture::from_archive(&fixture.archive, HEADER)?.decode_surfaces_hdr(0)?;
    for (face, image) in CUBE_FACE_NAMES.iter().zip(faces) {
        let path = fixture
            .target
            .join(format!("Data/Textures/rock_diff_{face}.exr"));
        assert_eq!(read_exr(&path)?, image.pixels(), "face {face}");
    }
    Ok(())
}
