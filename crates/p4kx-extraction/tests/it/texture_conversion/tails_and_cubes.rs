// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_extraction::{CUBE_FACE_NAMES, ConflictPolicy, Texture, TextureConversion};
use p4kx_testkit::{SplitDds, TestTextureFormat};

use super::fixture::{Fixture, HEADER, files, read_png, run, run_with};

type TestResult = Result<(), Box<dyn Error>>;

fn tailed() -> SplitDds {
    SplitDds::new(TestTextureFormat::LegacyBgra8, 16, 8)
        .split(2)
        .alpha(TestTextureFormat::R8)
}

#[test]
fn an_alpha_tail_goes_into_the_png_and_its_entries_are_not_written_raw() -> TestResult {
    let dds = tailed();
    let fixture = Fixture::new(dds.entries(HEADER))?;
    let tail = dds.alpha_tail().ok_or("a tail")?;

    let report = run(&fixture, ConflictPolicy::Skip);

    assert!(report.is_complete(), "{report:?}");
    assert_eq!(
        report.extracted(),
        6,
        "header, two parts, tail header and two tail parts"
    );
    assert_eq!(
        report.converted(),
        Some(6),
        "all of them belong to the texture"
    );
    assert_eq!(
        files(&fixture.target)?,
        [
            "Data/Textures/rock_diff.dds",
            "Data/Textures/rock_diff.dds.p4kx-origin.jsonl",
            "Data/Textures/rock_diff.png",
        ],
        "no tail Entries"
    );
    assert_eq!(
        fixture.read("Data/Textures/rock_diff.dds")?,
        dds.merged_bytes(),
        "the merged DDS holds the colour texture"
    );
    let (_, _, pixels) = read_png(&fixture.target.join("Data/Textures/rock_diff.png"))?;
    let expected = Texture::from_archive(&fixture.archive, HEADER)?.decode_mip(0)?;
    assert_eq!(
        pixels,
        expected.into_pixels(),
        "colour with the tail's alpha"
    );
    let alpha: Vec<u8> = pixels.iter().skip(3).step_by(4).copied().collect();
    assert_eq!(
        alpha,
        tail.mip(0),
        "the alpha channel is the tail's largest mip"
    );
    Ok(())
}

#[test]
fn keeping_raw_parts_also_writes_the_alpha_tail_entries() -> TestResult {
    let fixture = Fixture::new(tailed().entries(HEADER))?;
    let textures = TextureConversion {
        keep_raw: true,
        ..TextureConversion::default()
    };

    let report = run_with(&fixture, ConflictPolicy::Skip, textures);

    assert!(report.is_complete(), "{report:?}");
    assert_eq!(
        files(&fixture.target)?,
        [
            "Data/Textures/rock_diff.dds",
            "Data/Textures/rock_diff.dds.1",
            "Data/Textures/rock_diff.dds.1a",
            "Data/Textures/rock_diff.dds.2",
            "Data/Textures/rock_diff.dds.2a",
            "Data/Textures/rock_diff.dds.a",
            "Data/Textures/rock_diff.dds.p4kx-origin.jsonl",
            "Data/Textures/rock_diff.png",
        ]
    );
    Ok(())
}

#[test]
fn a_broken_alpha_tail_is_reported_and_the_raw_entries_are_written() -> TestResult {
    let mut entries = tailed().entries(HEADER);
    entries.remove(5);
    let fixture = Fixture::new(entries)?;

    let report = run(&fixture, ConflictPolicy::Skip);

    let [failure] = report.unconverted() else {
        return Err(format!("one unconverted texture expected: {report:?}").into());
    };
    let reason = Error::source(failure.error()).map(ToString::to_string);
    assert_eq!(
        reason,
        Some(format!("{HEADER}.2a is missing")),
        "names the missing tail part"
    );
    assert_eq!(
        files(&fixture.target)?,
        [
            "Data/Textures/rock_diff.dds",
            "Data/Textures/rock_diff.dds.1",
            "Data/Textures/rock_diff.dds.1a",
            "Data/Textures/rock_diff.dds.2",
            "Data/Textures/rock_diff.dds.a",
            "Data/Textures/rock_diff.dds.p4kx-origin.jsonl",
        ],
        "the merged DDS keeps the header's path, the other Entries are written raw"
    );
    Ok(())
}

#[test]
fn a_cubemap_is_merged_and_exported_as_six_faces() -> TestResult {
    let dds = SplitDds::new(TestTextureFormat::LegacyBgra8, 8, 8)
        .cubemap()
        .split(1);
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
            .map(|face| format!("Data/Textures/rock_diff_{face}.png")),
    );
    expected.sort();
    assert_eq!(files(&fixture.target)?, expected, "the DDS and six faces");
    assert_eq!(
        fixture.read("Data/Textures/rock_diff.dds")?,
        dds.merged_bytes(),
        "merged"
    );
    let faces = Texture::from_archive(&fixture.archive, HEADER)?.decode_surfaces(0)?;
    for (face, image) in CUBE_FACE_NAMES.iter().zip(faces) {
        let path = fixture
            .target
            .join(format!("Data/Textures/rock_diff_{face}.png"));
        assert_eq!(read_png(&path)?.2, image.into_pixels(), "face {face}");
    }
    let again = run(&fixture, ConflictPolicy::IfChanged);
    assert_eq!(
        again.skipped().len(),
        2,
        "identical outputs are kept: {again:?}"
    );
    Ok(())
}

#[test]
fn merging_alone_writes_the_alpha_tail_as_its_own_merged_dds() -> TestResult {
    let dds = tailed();
    let fixture = Fixture::new(dds.entries(HEADER))?;
    let tail = dds.alpha_tail().ok_or("a tail")?;
    let textures = TextureConversion {
        merge: true,
        png: None,
        keep_raw: false,
    };

    let report = run_with(&fixture, ConflictPolicy::Skip, textures);

    assert!(report.is_complete(), "{report:?}");
    assert_eq!(report.converted(), Some(6), "every Entry of the texture");
    assert_eq!(
        files(&fixture.target)?,
        [
            "Data/Textures/rock_diff.dds",
            "Data/Textures/rock_diff.dds.a",
            "Data/Textures/rock_diff.dds.a.p4kx-origin.jsonl",
            "Data/Textures/rock_diff.dds.p4kx-origin.jsonl",
        ],
        "the colour and the tail, each merged"
    );
    assert_eq!(
        fixture.read("Data/Textures/rock_diff.dds")?,
        dds.merged_bytes()
    );
    assert_eq!(
        fixture.read("Data/Textures/rock_diff.dds.a")?,
        tail.merged_bytes(),
        "the tail merged like any split texture"
    );
    Ok(())
}

#[test]
fn merging_alone_with_raw_parts_keeps_the_merged_tail_under_the_tail_header_path() -> TestResult {
    let dds = tailed();
    let fixture = Fixture::new(dds.entries(HEADER))?;
    let tail = dds.alpha_tail().ok_or("a tail")?;
    let textures = TextureConversion {
        merge: true,
        png: None,
        keep_raw: true,
    };

    let report = run_with(&fixture, ConflictPolicy::Skip, textures);

    assert!(report.is_complete(), "{report:?}");
    assert_eq!(
        files(&fixture.target)?.len(),
        8,
        "two merged files, two companions and four raw parts"
    );
    assert_eq!(
        fixture.read("Data/Textures/rock_diff.dds.a")?,
        tail.merged_bytes(),
        "the merged tail is not overwritten by its raw header"
    );
    assert_eq!(
        fixture.read("Data/Textures/rock_diff.dds.1a")?,
        tail.part(1)
    );
    Ok(())
}
