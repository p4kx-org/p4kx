// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_extraction::{CubemapLayout, TextureImageFace, TextureImageFormat, TextureImagePlan};
use p4kx_testkit::{SplitDds, TestTextureFormat};
use p4kx_texture::{DdsHeader, DdsRawHeader};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;
const CUBES: u32 = u32::MAX / 6 + 1;
const DX10_ARRAY_SIZE_OFFSET: usize = 140;

#[test]
fn a_large_declared_cube_array_keeps_exact_plan_count_and_direct_tail_access() -> TestResult {
    let source = source()?;
    let header = DdsHeader::from_raw(&source)?;
    for (layout, per_cube, face, size) in [
        (
            CubemapLayout::Faces,
            6_u64,
            TextureImageFace::NegativeZ,
            [1, 1],
        ),
        (CubemapLayout::Cross, 1_u64, TextureImageFace::Cross, [4, 3]),
    ] {
        let count = u64::from(CUBES) * per_cube;
        let mut plan = TextureImagePlan::new(&header, 0, layout, TextureImageFormat::Png)?;
        assert_eq!(
            u64::try_from(plan.len())?,
            count,
            "the complete original image plan"
        );
        let last = plan
            .nth(usize::try_from(count - 1)?)
            .ok_or("the final original output")?;
        assert_eq!(
            last.layer,
            CUBES - 1,
            "the last original cube's array index"
        );
        assert_eq!(
            last.face, face,
            "the last face or the caller's cross layout"
        );
        assert_eq!(last.slice, 0, "a cube has no volume index");
        assert_eq!(
            [last.width, last.height],
            size,
            "only the requested target layout changes the size"
        );
        let suffix = if layout == CubemapLayout::Faces {
            format!("_layer{}_negz", CUBES - 1)
        } else {
            format!("_layer{}", CUBES - 1)
        };
        assert_eq!(
            last.suffix, suffix,
            "the existing exact image naming contract"
        );
        assert_eq!(
            plan.len(),
            0,
            "direct tail access consumes the complete requested skip"
        );
        assert!(plan.next().is_none(), "no invented trailing source image");
    }
    Ok(())
}

fn source() -> TestResult<DdsRawHeader> {
    let mut bytes = SplitDds::new(TestTextureFormat::R8, 1, 1)
        .cubemap()
        .mips(1)
        .header_bytes();
    let word = bytes
        .get_mut(DX10_ARRAY_SIZE_OFFSET..DX10_ARRAY_SIZE_OFFSET + size_of::<u32>())
        .ok_or("the actual original array declaration")?;
    word.copy_from_slice(&CUBES.to_le_bytes());
    let raw = DdsRawHeader::parse(&bytes)?;
    let extended = raw.dx10().ok_or("the actual DX10 source header")?;
    assert_eq!(
        extended.array_size, CUBES,
        "the source count exists independently of the plan"
    );
    assert_eq!(
        extended.resource_dimension, 3,
        "the original 2D resource kind"
    );
    assert_eq!(extended.misc_flag & 4, 4, "the original cube flag");
    assert_eq!(
        bytes.len(),
        raw.header_len() + 6,
        "planning does not materialize the declared payload"
    );
    Ok(raw)
}
