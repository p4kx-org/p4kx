// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::path::PathBuf;

use p4kx_extraction as extraction;
use p4kx_extraction::{Archive, ConflictPolicy, Entry, ExtractOptions, ExtractionError};
use p4kx_testkit::{ArchiveBuilder, CryXmlByteOrder, CryXmlSignature, TestElement, TestEntry};

pub(super) type TestResult = Result<(), Box<dyn Error>>;

pub(super) struct Fixture {
    pub(super) folder: tempfile::TempDir,
    pub(super) archive: Archive,
    pub(super) output: PathBuf,
}

pub(super) fn fixture(
    entries: impl IntoIterator<Item = TestEntry>,
) -> Result<Fixture, Box<dyn Error>> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("source.p4k");
    let mut builder = ArchiveBuilder::new();
    for entry in entries {
        builder = builder.entry(entry);
    }
    builder.write_to(&path)?;
    let archive = Archive::open(path)?;
    let output = folder.path().join("out");
    fs::create_dir(&output)?;
    Ok(Fixture {
        folder,
        archive,
        output,
    })
}

pub(super) fn entry(fixture: &Fixture, index: usize) -> Result<&Entry, Box<dyn Error>> {
    Ok(fixture
        .archive
        .entries()
        .get(index)
        .ok_or("physical Entry")?)
}

#[test]
fn zero_and_populated_directory_named_records_keep_their_real_decoded_bytes() -> TestResult {
    let fixture = fixture([
        TestEntry::stored("Data/", b""),
        TestEntry::zstd("Data/populated/", b"directory-named bytes"),
        TestEntry::stored("normal.bin", b"normal bytes"),
    ])?;
    let expected: [&[u8]; 3] = [b"", b"directory-named bytes", b"normal bytes"];
    let cases = expected.into_iter().enumerate();
    for (index, bytes) in cases {
        let original = entry(&fixture, index)?;
        let leaf = format!("payload-{index}");
        let report = extraction::extract_entry_payload(
            &fixture.archive,
            original,
            &fixture.output,
            &leaf,
            &ExtractOptions {
                verify: true,
                ..ExtractOptions::default()
            },
        );
        assert!(
            report.is_complete(),
            "every selected physical record is read and written"
        );
        assert_eq!(report.extracted(), 1, "one payload per physical record");
        assert_eq!(
            fs::read(fixture.output.join(&leaf))?,
            bytes,
            "decoded bytes survive source-name interpretation"
        );
    }
    assert_eq!(
        fs::read_dir(&fixture.output)?.count(),
        3,
        "no source-named folder is synthesized"
    );
    Ok(())
}

#[test]
fn invalid_utf8_source_names_are_metadata_while_normal_extraction_stays_strict() -> TestResult {
    let fixture = fixture([TestEntry::stored("x", b"retained").with_path_bytes(b"Data/\xff/")])?;
    let original = entry(&fixture, 0)?;
    assert_eq!(
        fixture.archive.raw_path(original.index()),
        Some(b"Data/\xff/".as_slice()),
        "original bytes are retained by the reader"
    );
    let report = extraction::extract_entry_payload(
        &fixture.archive,
        original,
        &fixture.output,
        "payload",
        &ExtractOptions {
            verify: true,
            ..ExtractOptions::default()
        },
    );
    assert!(
        report.is_complete(),
        "raw path bytes do not choose the fixed destination"
    );
    assert_eq!(
        fs::read(fixture.output.join("payload"))?,
        b"retained",
        "invalid UTF8 does not change payload"
    );
    let natural = fixture.folder.path().join("natural");
    fs::create_dir(&natural)?;
    let ordinary = extraction::extract(
        &fixture.archive,
        &[original],
        &natural,
        &ExtractOptions::default(),
    );
    assert!(
        matches!(
            ordinary
                .failures()
                .first()
                .map(extraction::EntryFailure::error),
            Some(ExtractionError::InvalidName { .. })
        ),
        "ordinary source-path extraction keeps its existing filename checks"
    );
    assert_eq!(
        fs::read_dir(natural)?.count(),
        0,
        "ordinary unsafe names write nothing"
    );
    Ok(())
}

#[test]
fn conversion_settings_never_convert_a_raw_payload_and_normal_bytes_match() -> TestResult {
    let xml = TestElement::new("Synthetic")
        .attribute("raw", "yes")
        .to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little);
    let fixture = fixture([TestEntry::zstd("Data/original.xml", xml.as_slice())])?;
    let original = entry(&fixture, 0)?;
    let report = extraction::extract_entry_payload(
        &fixture.archive,
        original,
        &fixture.output,
        "payload",
        &ExtractOptions {
            convert: true,
            verify: true,
            ..ExtractOptions::default()
        },
    );
    assert!(
        report.is_complete(),
        "raw payload extraction succeeds under conversion-capable caller options"
    );
    assert_eq!(
        report.converted(),
        None,
        "payload API explicitly disables conversion"
    );
    let normal = fixture.folder.path().join("normal");
    fs::create_dir(&normal)?;
    let ordinary = extraction::extract(
        &fixture.archive,
        &[original],
        &normal,
        &ExtractOptions {
            verify: true,
            ..ExtractOptions::default()
        },
    );
    assert!(
        ordinary.is_complete(),
        "ordinary raw extraction also succeeds"
    );
    assert_eq!(
        fs::read(fixture.output.join("payload"))?,
        xml,
        "binary XML is not rewritten"
    );
    assert_eq!(
        fs::read(normal.join("Data/original.xml"))?,
        xml,
        "both paths share the same decoded bytes"
    );
    Ok(())
}

#[test]
fn verified_zero_length_directory_payload_fails_its_actual_crc_and_keeps_old_output() -> TestResult
{
    let fixture = fixture([TestEntry::stored("Data/", b"").with_crc32c(1)])?;
    let original = entry(&fixture, 0)?;
    fs::write(fixture.output.join("payload"), b"old")?;
    let report = extraction::extract_entry_payload(
        &fixture.archive,
        original,
        &fixture.output,
        "payload",
        &ExtractOptions {
            policy: ConflictPolicy::Overwrite,
            verify: true,
            ..ExtractOptions::default()
        },
    );
    assert_eq!(
        (report.extracted(), report.failures().len()),
        (0, 1),
        "zero length is not a shortcut around verification"
    );
    let failure = report
        .failures()
        .first()
        .ok_or("original checksum failure")?;
    assert_eq!(
        failure.path(),
        "Data/",
        "report keeps the original physical source name"
    );
    assert!(
        matches!(failure.error(), ExtractionError::ChecksumMismatch { .. }),
        "original checksum cause survives"
    );
    assert_eq!(
        fs::read(fixture.output.join("payload"))?,
        b"old",
        "failed verification cannot replace a previous file"
    );
    assert_eq!(
        fs::read_dir(&fixture.output)?.count(),
        1,
        "owned partial file is removed"
    );
    Ok(())
}

#[test]
fn explicitly_selected_output_names_remain_single_portable_leaves() -> TestResult {
    let fixture = fixture([TestEntry::stored("source.bin", b"data")])?;
    let original = entry(&fixture, 0)?;
    for leaf in ["../escape", "a/b", r"a\b", "", ".", "NUL"] {
        let report = extraction::extract_entry_payload(
            &fixture.archive,
            original,
            &fixture.output,
            leaf,
            &ExtractOptions::default(),
        );
        assert!(
            matches!(
                report
                    .failures()
                    .first()
                    .map(extraction::EntryFailure::error),
                Some(ExtractionError::InvalidName { .. })
            ),
            "invalid output leaf is refused: {leaf:?}"
        );
        assert_eq!(
            report
                .failures()
                .first()
                .map(extraction::EntryFailure::path),
            Some("source.bin"),
            "failure report keeps source identity"
        );
    }
    assert_eq!(
        fs::read_dir(&fixture.output)?.count(),
        0,
        "rejected output leaves create no files"
    );
    Ok(())
}
