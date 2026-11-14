// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Cubemaps and texture arrays: merging parts that hold one mip of every image, reading one mip
//! of every face and laying the faces out as a cross.

use std::collections::BTreeMap;
use std::io;

use p4kx_testkit::{SplitDds, TestTextureFormat};
use p4kx_texture::{
    DdsHeader, Mip, RgbaImage, SiblingReader, TextureError, cube_cross, merge, read_mip,
};

/// Parts held in memory, by part number.
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

fn merged(dds: &SplitDds, parts: &Parts) -> Result<Vec<u8>, TextureError> {
    let header = dds.header_bytes();
    let mut out = Vec::new();
    let report = merge(header.as_slice(), header.len() as u64, parts, &mut out)?;
    assert_eq!(report.size, out.len() as u64, "the reported size");
    assert_eq!(report.parts, dds.part_count(), "every part is read");
    Ok(out)
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

#[test]
fn a_split_cubemap_merges_face_by_face_each_face_largest_mip_first() -> Result<(), TextureError> {
    let dds = SplitDds::new(TestTextureFormat::LegacyBgra8, 2, 2)
        .cubemap()
        .split(1);

    let out = merged(&dds, &Parts::of(&dds))?;

    let header = dds.header_len();
    assert_eq!(
        out.len(),
        header + 6 * (16 + 4),
        "header and six faces of two mips"
    );
    for face in 0..6 {
        let start = header + face as usize * 20;
        assert_eq!(
            out.get(start..start + 16),
            Some(dds.surface_mip(face, 0).as_slice()),
            "mip 0 of face {face} from part 1"
        );
        assert_eq!(
            out.get(start + 16..start + 20),
            Some(dds.surface_mip(face, 1).as_slice()),
            "mip 1 of face {face} from the header"
        );
    }
    Ok(())
}

#[test]
fn the_merged_cubemap_is_a_valid_cubemap_dds() -> Result<(), TextureError> {
    let dds = SplitDds::new(TestTextureFormat::LegacyDxt1, 32, 32)
        .cubemap()
        .split(3);

    let out = merged(&dds, &Parts::of(&dds))?;

    assert_eq!(out, dds.merged_bytes(), "the standard cubemap layout");
    let header = DdsHeader::parse(&out)?;
    assert!(header.is_cubemap(), "still a cubemap");
    assert_eq!(
        header.part_count(out.len() as u64)?,
        0,
        "every mip is inside"
    );
    Ok(())
}

#[test]
fn a_split_dx10_cubemap_and_texture_array_merge_image_by_image() -> Result<(), TextureError> {
    let cube = SplitDds::new(TestTextureFormat::BC7_SRGB, 16, 8)
        .cubemap()
        .split(2);
    let array = SplitDds::new(TestTextureFormat::R8, 8, 8).array(3).split(4);

    for dds in [cube, array] {
        assert_eq!(
            merged(&dds, &Parts::of(&dds))?,
            dds.merged_bytes(),
            "{dds:?}"
        );
    }
    Ok(())
}

/// Parts whose part 2 claims its full length but delivers only its first 100 bytes.
struct ShortPartTwo(Parts);

impl SiblingReader for ShortPartTwo {
    type Part<'a> = &'a [u8];

    fn part_len(&self, index: u32) -> io::Result<Option<u64>> {
        self.0.part_len(index)
    }

    fn open_part(&self, index: u32) -> io::Result<Self::Part<'_>> {
        self.0.open_part(index).map(|data| {
            if index == 2 {
                data.get(..100).unwrap_or(data)
            } else {
                data
            }
        })
    }
}

#[test]
fn a_cubemap_part_that_ends_early_is_truncated() {
    let dds = SplitDds::new(TestTextureFormat::LegacyDxt1, 8, 8)
        .cubemap()
        .split(2);
    let header = dds.header_bytes();
    let all = ShortPartTwo(Parts::of(&dds));

    let result = merge(
        header.as_slice(),
        header.len() as u64,
        &all,
        &mut io::sink(),
    );

    assert!(
        matches!(
            result,
            Err(TextureError::TruncatedPart {
                index: 2,
                expected: 32,
                actual: 4
            })
        ),
        "face 3 of part 2 ends after 4 of its 32 bytes: {result:?}"
    );
}

#[test]
fn a_missing_cubemap_part_is_named() {
    let dds = SplitDds::new(TestTextureFormat::LegacyDxt1, 8, 8)
        .cubemap()
        .split(2);
    let mut parts = Parts::of(&dds);
    parts.0.remove(&1);

    let result = merged(&dds, &parts);

    assert!(
        matches!(result, Err(TextureError::MissingPart { index: 1 })),
        "{result:?}"
    );
}

#[test]
fn a_cubemap_mip_holds_every_face_from_its_part_or_the_header() -> Result<(), TextureError> {
    let dds = SplitDds::new(TestTextureFormat::LegacyDxt5, 16, 16)
        .cubemap()
        .split(2);

    for level in 0..5 {
        let mip = read(&dds, level)?;
        assert_eq!(mip.surfaces(), 6, "six faces of mip {level}");
        for face in 0..6 {
            assert_eq!(
                mip.surface(u64::from(face)),
                Some(dds.surface_mip(face, level).as_slice()),
                "face {face} of mip {level}"
            );
        }
        assert_eq!(mip.surface(6), None, "no seventh face");
    }
    Ok(())
}

#[test]
fn every_face_decodes_but_not_as_one_image() -> Result<(), TextureError> {
    let dds = SplitDds::new(TestTextureFormat::LegacyBgra8, 4, 4)
        .cubemap()
        .split(1);
    let mip = read(&dds, 0)?;

    let faces = mip.decode_surfaces()?;

    assert_eq!(faces.len(), 6, "six faces");
    for (face, image) in (0..).zip(&faces) {
        let bgra = dds.surface_mip(face, 0);
        let first = bgra.get(..4).unwrap_or_default();
        assert_eq!(
            image.pixels().get(..4),
            Some([first[2], first[1], first[0], first[3]].as_slice()),
            "the first pixel of face {face}"
        );
    }
    assert!(
        matches!(
            mip.decode(),
            Err(TextureError::UnsupportedImage { images: 6 })
        ),
        "one image needs a layout"
    );
    Ok(())
}

fn face(colour: u8, width: u32, height: u32) -> Result<RgbaImage, TextureError> {
    let len = (width * height * 4) as usize;
    RgbaImage::new(width, height, vec![colour; len])
}

#[test]
fn the_faces_form_a_horizontal_cross() -> Result<(), TextureError> {
    let faces = (1..=6)
        .map(|colour| face(colour, 1, 1))
        .collect::<Result<Vec<_>, _>>()?;

    let cross = cube_cross(&faces)?;

    assert_eq!(
        (cross.width(), cross.height()),
        (4, 3),
        "four faces wide, three high"
    );
    let rows: Vec<Vec<u8>> = cross
        .pixels()
        .chunks(16)
        .map(|row| row.chunks(4).map(|pixel| pixel[0]).collect())
        .collect();
    assert_eq!(
        rows,
        [vec![0, 3, 0, 0], vec![2, 5, 1, 6], vec![0, 4, 0, 0]],
        "+Y above +Z; -X, +Z, +X, -Z across; -Y below +Z; the rest transparent"
    );
    assert_eq!(
        cross.pixels().get(3),
        Some(&0),
        "an empty cell is transparent"
    );
    Ok(())
}

#[test]
fn a_cross_needs_six_faces_of_one_size() -> Result<(), TextureError> {
    let five = (1..=5)
        .map(|colour| face(colour, 2, 2))
        .collect::<Result<Vec<_>, _>>()?;
    let mut mixed = five.clone();
    mixed.push(face(6, 1, 1)?);

    for faces in [five, mixed] {
        let result = cube_cross(&faces);
        assert!(
            matches!(result, Err(TextureError::NotACube { .. })),
            "{} faces: {result:?}",
            faces.len()
        );
    }
    Ok(())
}

#[test]
fn an_image_needs_four_bytes_per_pixel() {
    let result = RgbaImage::new(2, 2, vec![0; 15]);

    assert!(
        matches!(
            result,
            Err(TextureError::MipLength {
                expected: 16,
                actual: 15
            })
        ),
        "{result:?}"
    );
}
