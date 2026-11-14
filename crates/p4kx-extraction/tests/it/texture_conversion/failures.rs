// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_extraction::{ConflictPolicy, ExtractionError, ExtractionReport, NameRule};
use p4kx_testkit::{SplitDds, TestEntry, TestTextureFormat};

use super::fixture::{Fixture, HEADER, files, merged_layout, rock, run, texture_entries};

type TestResult = Result<(), Box<dyn Error>>;

const HULL: &str = "Data\\Ships\\hull_ddna.dds";

fn reason(report: &ExtractionReport) -> Result<(String, String), Box<dyn Error>> {
    let [failure] = report.unconverted() else {
        return Err(format!("one unconverted texture expected: {report:?}").into());
    };
    let ExtractionError::Texture(cause) = failure.error() else {
        return Err(format!("a texture error expected: {:?}", failure.error()).into());
    };
    Ok((failure.path().to_owned(), cause.to_string()))
}

#[test]
fn a_texture_with_a_missing_part_is_reported_and_its_raw_entries_are_written() -> TestResult {
    let dds = rock();
    let mut entries = texture_entries(&dds);
    entries.remove(2);
    entries.extend(
        SplitDds::new(TestTextureFormat::LegacyBgra8, 8, 8)
            .split(1)
            .entries(HULL),
    );
    entries.push(TestEntry::stored("Data\\readme.txt", b"hello".to_vec()));
    let fixture = Fixture::new(entries)?;

    let report = run(&fixture, ConflictPolicy::Skip);

    assert!(!report.is_complete(), "a texture was not converted");
    assert!(report.failures().is_empty(), "every Entry was written");
    assert_eq!(
        reason(&report)?,
        (HEADER.to_owned(), format!("{HEADER}.2 is missing")),
        "the texture and why"
    );
    assert_eq!(report.extracted(), 6, "every Entry");
    assert_eq!(report.converted(), Some(2), "the other texture");
    assert_eq!(
        files(&fixture.target)?,
        [
            "Data/Ships/hull_ddna.dds",
            "Data/Ships/hull_ddna.png",
            "Data/Textures/rock_diff.dds",
            "Data/Textures/rock_diff.dds.1",
            "Data/Textures/rock_diff.dds.3",
            "Data/readme.txt",
        ],
        "the raw Entries of the failed texture, the rest converted"
    );
    assert_eq!(
        fixture.read("Data/Textures/rock_diff.dds")?,
        dds.header_bytes(),
        "the raw header"
    );
    Ok(())
}

#[test]
fn a_format_without_a_decoder_keeps_the_merged_dds_and_writes_the_raw_parts() -> TestResult {
    let integer = TestTextureFormat::Dx10Pixel {
        dxgi_format: 18,
        pixel_bytes: 8,
    };
    let dds = SplitDds::new(integer, 16, 16).split(2);
    let fixture = Fixture::new(dds.entries(HULL))?;

    let report = run(&fixture, ConflictPolicy::Skip);

    let (path, why) = reason(&report)?;
    assert_eq!(path, HULL, "the texture");
    assert!(why.contains(HULL), "names the texture: {why}");
    let source = report
        .unconverted()
        .first()
        .and_then(|failure| Error::source(failure.error()))
        .and_then(Error::source)
        .map_or_default(ToString::to_string);
    assert!(source.contains("R32G32_SINT"), "names the format: {source}");
    assert_eq!(
        files(&fixture.target)?,
        [
            "Data/Ships/hull_ddna.dds",
            "Data/Ships/hull_ddna.dds.1",
            "Data/Ships/hull_ddna.dds.2",
        ],
        "the merged DDS and the raw parts, no PNG"
    );
    let mut merged = dds
        .header_bytes()
        .get(..dds.header_len())
        .unwrap_or_default()
        .to_vec();
    for level in 0..5 {
        merged.extend_from_slice(&dds.mip(level));
    }
    assert_eq!(fixture.read("Data/Ships/hull_ddna.dds")?, merged, "merged");
    Ok(())
}

#[test]
fn a_header_that_is_no_dds_is_reported_and_written_raw() -> TestResult {
    let fixture = Fixture::new(vec![
        TestEntry::stored(HEADER, b"not a texture".to_vec()),
        TestEntry::stored(format!("{HEADER}.1"), vec![1; 64]),
    ])?;

    let report = run(&fixture, ConflictPolicy::Skip);

    assert_eq!(reason(&report)?.0, HEADER, "the texture");
    assert_eq!(report.extracted(), 2, "both raw Entries");
    assert_eq!(
        fixture.read("Data/Textures/rock_diff.dds")?,
        b"not a texture"
    );
    assert_eq!(fixture.read("Data/Textures/rock_diff.dds.1")?, vec![1; 64]);
    Ok(())
}

#[test]
fn a_failed_png_keeps_an_existing_merged_dds_instead_of_the_raw_header() -> TestResult {
    let dds = rock();
    let fixture = Fixture::new(texture_entries(&dds))?;
    run(&fixture, ConflictPolicy::Skip);
    let broken = Fixture::new({
        let mut entries = texture_entries(&dds);
        entries.remove(3);
        entries
    })?;
    broken.write(
        "Data/Textures/rock_diff.dds",
        &fixture.read("Data/Textures/rock_diff.dds")?,
    )?;

    let report = run(&broken, ConflictPolicy::Skip);

    assert_eq!(reason(&report)?.1, format!("{HEADER}.3 is missing"));
    assert_eq!(
        report.skipped(),
        [HEADER],
        "the raw header is not written over it"
    );
    assert_eq!(
        broken.read("Data/Textures/rock_diff.dds")?,
        merged_layout(&dds),
        "kept"
    );
    Ok(())
}

#[test]
fn a_texture_whose_path_would_leave_the_target_fails_once_and_writes_nothing() -> TestResult {
    let dds = SplitDds::new(TestTextureFormat::LegacyBgra8, 8, 8).split(1);
    let fixture = Fixture::new(dds.entries("..\\escape.dds"))?;

    let report = run(&fixture, ConflictPolicy::Skip);

    assert!(report.unconverted().is_empty(), "{report:?}");
    assert_eq!(report.failures().len(), 2, "the header and its part");
    assert!(
        report.failures().iter().all(|failure| matches!(
            failure.error(),
            ExtractionError::InvalidName {
                rule: NameRule::DotPart,
                ..
            }
        )),
        "{report:?}"
    );
    assert!(files(&fixture.target)?.is_empty(), "nothing written");
    Ok(())
}
