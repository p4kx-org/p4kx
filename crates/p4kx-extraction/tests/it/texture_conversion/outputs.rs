// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_extraction::{ConflictPolicy, ExtractOptions, Texture, TextureConversion};
use p4kx_testkit::{SplitDds, TestEntry, TestTextureFormat};

use super::fixture::{
    Fixture, HEADER, files, merged_layout, read_png, rock, run, run_with, texture_entries,
};

type TestResult = Result<(), Box<dyn Error>>;

const RAW_ROCK_OUTPUTS: [&str; 8] = [
    "Data/Textures/rock_diff.dds",
    "Data/Textures/rock_diff.dds.1",
    "Data/Textures/rock_diff.dds.1.p4kx-origin.jsonl",
    "Data/Textures/rock_diff.dds.2",
    "Data/Textures/rock_diff.dds.2.p4kx-origin.jsonl",
    "Data/Textures/rock_diff.dds.3",
    "Data/Textures/rock_diff.dds.3.p4kx-origin.jsonl",
    "Data/Textures/rock_diff.dds.p4kx-origin.jsonl",
];

#[test]
fn conversion_merges_a_split_texture_and_exports_its_largest_mip_once() -> TestResult {
    let fixture = Fixture::new(texture_entries(&rock()))?;

    let report = run(&fixture, ConflictPolicy::Skip);

    assert!(report.is_complete(), "{report:?}");
    assert_eq!(report.extracted(), 4, "header and three parts");
    assert_eq!(report.converted(), Some(4), "every Entry of the texture");
    assert_eq!(
        files(&fixture.target)?,
        [
            "Data/Textures/rock_diff.dds",
            "Data/Textures/rock_diff.dds.p4kx-origin.jsonl",
            "Data/Textures/rock_diff.png",
        ],
        "one DDS and one PNG, no parts"
    );
    assert_eq!(
        fixture.read("Data/Textures/rock_diff.dds")?,
        merged_layout(&rock()),
        "the merged DDS"
    );
    let (width, height, pixels) = read_png(&fixture.target.join("Data/Textures/rock_diff.png"))?;
    assert_eq!((width, height), (32, 16), "the largest mip");
    let expected = Texture::from_archive(&fixture.archive, HEADER)?.decode_mip(0)?;
    assert_eq!(pixels, expected.into_pixels(), "the decoded pixels");
    Ok(())
}

#[test]
fn a_chosen_mip_level_is_exported_and_a_level_beyond_the_smallest_takes_the_smallest() -> TestResult
{
    for (level, side) in [(1, (16, 8)), (3, (4, 2)), (40, (1, 1))] {
        let fixture = Fixture::new(texture_entries(&rock()))?;
        let textures = TextureConversion {
            merge: false,
            png: Some(level),
            keep_raw: false,
        };

        let report = run_with(&fixture, ConflictPolicy::Skip, textures);

        assert!(report.is_complete(), "{report:?}");
        assert_eq!(
            files(&fixture.target)?,
            ["Data/Textures/rock_diff.png"],
            "only the PNG"
        );
        let (width, height, pixels) =
            read_png(&fixture.target.join("Data/Textures/rock_diff.png"))?;
        assert_eq!((width, height), side, "mip {level}");
        let expected = Texture::from_archive(&fixture.archive, HEADER)?.decode_mip(level.min(5))?;
        assert_eq!(pixels, expected.into_pixels(), "the pixels of mip {level}");
    }
    Ok(())
}

#[test]
fn merging_alone_writes_only_the_merged_dds() -> TestResult {
    let fixture = Fixture::new(texture_entries(&rock()))?;
    let textures = TextureConversion {
        merge: true,
        png: None,
        keep_raw: false,
    };

    let report = run_with(&fixture, ConflictPolicy::Skip, textures);

    assert!(report.is_complete(), "{report:?}");
    assert_eq!(
        files(&fixture.target)?,
        [
            "Data/Textures/rock_diff.dds",
            "Data/Textures/rock_diff.dds.p4kx-origin.jsonl"
        ],
        "merge-only output and its companion"
    );
    assert_eq!(
        fixture.read("Data/Textures/rock_diff.dds")?,
        merged_layout(&rock())
    );
    Ok(())
}

#[test]
fn keeping_raw_parts_with_the_merge_writes_the_parts_next_to_the_merged_dds() -> TestResult {
    let dds = rock();
    let fixture = Fixture::new(texture_entries(&dds))?;
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
            "Data/Textures/rock_diff.dds.1.p4kx-origin.jsonl",
            "Data/Textures/rock_diff.dds.2",
            "Data/Textures/rock_diff.dds.2.p4kx-origin.jsonl",
            "Data/Textures/rock_diff.dds.3",
            "Data/Textures/rock_diff.dds.3.p4kx-origin.jsonl",
            "Data/Textures/rock_diff.dds.p4kx-origin.jsonl",
            "Data/Textures/rock_diff.png",
        ]
    );
    assert_eq!(
        fixture.read("Data/Textures/rock_diff.dds")?,
        merged_layout(&dds),
        "the merged DDS takes the header's path"
    );
    for index in 1..=3 {
        assert_eq!(
            fixture.read(&format!("Data/Textures/rock_diff.dds.{index}"))?,
            dds.part(index),
            "raw part {index}"
        );
    }
    Ok(())
}

#[test]
fn keeping_raw_parts_without_the_merge_writes_the_header_and_parts_unchanged() -> TestResult {
    let dds = rock();
    let fixture = Fixture::new(texture_entries(&dds))?;
    let textures = TextureConversion {
        merge: false,
        png: Some(0),
        keep_raw: true,
    };

    let report = run_with(&fixture, ConflictPolicy::Skip, textures);

    assert!(report.is_complete(), "{report:?}");
    assert_eq!(
        files(&fixture.target)?,
        [
            "Data/Textures/rock_diff.dds",
            "Data/Textures/rock_diff.dds.1",
            "Data/Textures/rock_diff.dds.1.p4kx-origin.jsonl",
            "Data/Textures/rock_diff.dds.2",
            "Data/Textures/rock_diff.dds.2.p4kx-origin.jsonl",
            "Data/Textures/rock_diff.dds.3",
            "Data/Textures/rock_diff.dds.3.p4kx-origin.jsonl",
            "Data/Textures/rock_diff.dds.p4kx-origin.jsonl",
            "Data/Textures/rock_diff.png",
        ],
        "the raw Entries with their companions and the PNG"
    );
    assert_eq!(
        fixture.read("Data/Textures/rock_diff.dds")?,
        dds.header_bytes(),
        "the raw header"
    );
    Ok(())
}

#[test]
fn without_any_texture_output_textures_are_written_raw() -> TestResult {
    let dds = rock();
    let fixture = Fixture::new(texture_entries(&dds))?;
    let textures = TextureConversion {
        merge: false,
        png: None,
        keep_raw: false,
    };

    let report = run_with(&fixture, ConflictPolicy::Skip, textures);

    assert!(report.is_complete(), "{report:?}");
    assert_eq!(report.converted(), Some(0), "nothing converted");
    assert_eq!(
        files(&fixture.target)?,
        RAW_ROCK_OUTPUTS,
        "the raw Entries and their companions"
    );
    Ok(())
}

#[test]
fn without_conversion_textures_are_written_raw() -> TestResult {
    let dds = rock();
    let fixture = Fixture::new(texture_entries(&dds))?;
    let entries: Vec<_> = fixture.archive.entries().iter().collect();

    let report = p4kx_extraction::extract(
        &fixture.archive,
        &entries,
        &fixture.target,
        &ExtractOptions::default(),
    );

    assert!(report.is_complete(), "{report:?}");
    assert_eq!(report.converted(), None, "Conversion is off");
    assert_eq!(
        files(&fixture.target)?,
        RAW_ROCK_OUTPUTS,
        "the raw Entries and their companions"
    );
    assert_eq!(
        fixture.read("Data/Textures/rock_diff.dds")?,
        dds.header_bytes()
    );
    Ok(())
}

#[test]
fn a_selected_header_is_merged_with_parts_that_are_not_selected() -> TestResult {
    let dds = rock();
    let fixture = Fixture::new(texture_entries(&dds))?;
    let header: Vec<_> = fixture
        .archive
        .entries()
        .iter()
        .filter(|entry| entry.path() == HEADER)
        .collect();

    let options = ExtractOptions {
        convert: true,
        ..ExtractOptions::default()
    };
    let report = p4kx_extraction::extract(&fixture.archive, &header, &fixture.target, &options);

    assert!(report.is_complete(), "{report:?}");
    assert_eq!(report.extracted(), 1, "the one selected Entry");
    assert_eq!(
        fixture.read("Data/Textures/rock_diff.dds")?,
        merged_layout(&dds),
        "merged from the Archive's parts"
    );
    Ok(())
}

#[test]
fn a_texture_without_parts_and_other_entries_convert_alongside() -> TestResult {
    let whole = SplitDds::new(TestTextureFormat::LegacyBgra8, 8, 8);
    let mut entries = texture_entries(&rock());
    entries.push(TestEntry::stored(
        "Data\\UI\\icon.DDS",
        whole.header_bytes(),
    ));
    entries.push(TestEntry::stored("Data\\readme.txt", b"hello".to_vec()));
    let fixture = Fixture::new(entries)?;

    let report = run(&fixture, ConflictPolicy::Skip);

    assert!(report.is_complete(), "{report:?}");
    assert_eq!(report.extracted(), 6, "every Entry");
    assert_eq!(report.converted(), Some(5), "the Entries of both textures");
    assert_eq!(
        files(&fixture.target)?,
        [
            "Data/Textures/rock_diff.dds",
            "Data/Textures/rock_diff.dds.p4kx-origin.jsonl",
            "Data/Textures/rock_diff.png",
            "Data/UI/icon.DDS",
            "Data/UI/icon.DDS.p4kx-origin.jsonl",
            "Data/UI/icon.png",
            "Data/readme.txt",
            "Data/readme.txt.p4kx-origin.jsonl",
        ]
    );
    assert_eq!(fixture.read("Data/UI/icon.DDS")?, whole.header_bytes());
    Ok(())
}
