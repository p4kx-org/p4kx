// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_archive::{
    Compression, Entry, EntryIssue, EntryRecordFormat, OpenOptions, ZipHeaderMetadata,
};
use p4kx_testkit::{ArchiveBuilder, TestEntry};

use super::TestResult;
use super::fixed_header_fixture::{self as header_fixture, HeaderWords};
use super::fixture::{self, Words};
use super::zip;

fn assert_source(entry: &Entry, format: EntryRecordFormat, ordinal: Option<u64>, method: u16) {
    let metadata = entry.metadata();
    assert_eq!(
        metadata.record_format(),
        format,
        "the physical record format remains explicit"
    );
    assert_eq!(
        metadata.original_directory_ordinal(),
        ordinal,
        "source order precedes admission and is not invented by recovery"
    );
    assert_eq!(
        metadata.raw_compression_method(),
        method,
        "the complete source method word survives"
    );
}

fn assert_dos(entry: &Entry, expected: Words) {
    let timestamp = entry.metadata().dos_timestamp();
    assert_eq!(
        timestamp.raw_date(),
        expected.date,
        "the actual source date word remains present"
    );
    assert_eq!(
        timestamp.raw_time(),
        expected.time,
        "the actual source time word remains present"
    );
}

fn assert_zip_words(header: &ZipHeaderMetadata, expected: HeaderWords) {
    assert_eq!(
        [
            header.raw_version_made_by(),
            header.raw_version_needed(),
            header.general_purpose_flags(),
            header.disk_start(),
            header.internal_attributes(),
        ],
        [
            expected.made_by,
            expected.needed,
            expected.flags,
            expected.disk,
            expected.internal
        ],
        "all raw u16 header words retain their distinct source values"
    );
    assert_eq!(
        header.external_attributes(),
        expected.external,
        "all external-attribute bits survive"
    );
}

fn assert_zip(entry: &Entry, expected: HeaderWords, ordinal: Option<u64>) -> TestResult {
    let metadata = entry.metadata();
    assert_source(
        entry,
        EntryRecordFormat::ZipCentralDirectory,
        ordinal,
        expected.method,
    );
    assert_eq!(
        metadata.raw_v2_encryption_word(),
        None,
        "a ZIP record has no v2 encryption word"
    );
    assert_zip_words(
        metadata
            .zip_header()
            .ok_or("ZIP header metadata is absent")?,
        expected,
    );
    assert!(
        !entry.is_encrypted(),
        "general-purpose flags do not replace the CIG encryption semantics"
    );
    Ok(())
}

fn assert_v2(entry: &Entry, index: usize, words: (u16, u16), timestamp: Words) -> TestResult {
    let (method, encryption) = words;
    let metadata = entry.metadata();
    assert_source(
        entry,
        EntryRecordFormat::P4kV2,
        Some(u64::try_from(index)?),
        method,
    );
    assert_dos(entry, timestamp);
    assert_eq!(
        metadata.raw_v2_encryption_word(),
        Some(encryption),
        "zero and unknown encryption words remain present"
    );
    assert_eq!(
        metadata.zip_header(),
        None,
        "v2 never invents ZIP-only words"
    );
    assert_eq!(
        entry.is_encrypted(),
        encryption != 0,
        "nonzero encryption interpretation is retained"
    );
    assert_eq!(
        entry.installed_bytes(),
        Some(3),
        "fixed metadata does not overwrite source progress"
    );
    Ok(())
}

fn assert_zip_archive(
    bytes: &[u8],
    words: HeaderWords,
    timestamp: Words,
    options: OpenOptions,
) -> TestResult {
    let (_directory, archive) = super::open(bytes, options)?;
    let folder = archive.entries().first().ok_or("folder Entry")?;
    let file = archive.entries().get(1).ok_or("file Entry")?;
    assert_zip(folder, header_fixture::FOLDER_WORDS, Some(0))?;
    assert_zip(file, words, Some(1))?;
    assert_dos(folder, header_fixture::FOLDER_STAMP);
    assert_dos(file, timestamp);
    let expected = if words.method == 0 {
        Compression::Store
    } else {
        Compression::Other(0xd00d)
    };
    assert_eq!(
        file.compression(),
        expected,
        "decoder selection keeps its source meaning"
    );
    assert!(
        archive.issues().is_empty(),
        "unknown header bits and requested recovery do not invent issues"
    );
    Ok(())
}

#[test]
fn v1_and_zip_fixed_words_remain_uninterpreted_and_zero_words_stay_present() -> TestResult {
    for layout in [zip::Layout::P4k, zip::Layout::Zip] {
        for words in [header_fixture::FILE_WORDS, header_fixture::ZERO_WORDS] {
            for timestamp in [header_fixture::STAMP, header_fixture::ZERO_STAMP] {
                let source = header_fixture::zip_source(layout, words, timestamp)?;
                for options in [OpenOptions::new(), OpenOptions::new().recover(true)] {
                    assert_zip_archive(&source.bytes, words, timestamp, options)?;
                }
            }
        }
    }
    Ok(())
}
fn assert_v2_folder_admission(base: usize) -> TestResult {
    let bytes = header_fixture::v2_folder_source(header_fixture::STAMP, base)?;
    for source in [
        TestEntry::stored("Data\\v2-headers.socpak", bytes.clone()),
        TestEntry::deflated("Data\\v2-headers.socpak", bytes),
    ] {
        let outer = ArchiveBuilder::new().entry(source).build();
        let (_directory, parent) = super::open(&outer, OpenOptions::new())?;
        let entry = parent.entries().first().ok_or("outer v2 container")?;
        let nested = parent.open_container(entry)?;
        let file = nested.entries().first().ok_or("admitted v2 file")?;
        assert_eq!(
            nested.entries().len(),
            3,
            "v2 admission removes the empty folder record"
        );
        assert_eq!(file.index(), 0, "the v2 file's admitted index changes");
        let words = header_fixture::V2_WORDS
            .get(1)
            .copied()
            .ok_or("v2 file words")?;
        assert_v2(file, 1, words, header_fixture::STAMP)?;
        assert!(
            nested.issues().is_empty(),
            "fixed metadata adds no nested v2 issue"
        );
    }
    Ok(())
}
#[test]
fn folder_admission_preserves_original_ordinals_and_each_files_own_header() -> TestResult {
    let source = header_fixture::zip_source(
        zip::Layout::Zip,
        header_fixture::FILE_WORDS,
        header_fixture::STAMP,
    )?;
    for entry in [
        TestEntry::stored("Data\\headers.socpak", source.bytes.clone()),
        TestEntry::deflated("Data\\headers.socpak", source.bytes),
    ] {
        let outer = ArchiveBuilder::new().entry(entry).build();
        let (_directory, parent) = super::open(&outer, OpenOptions::new())?;
        let container = parent.entries().first().ok_or("outer container")?;
        let nested = parent.open_container(container)?;
        let file = nested.entries().first().ok_or("admitted file")?;
        assert_eq!(
            nested.entries().len(),
            1,
            "admission removes the authored folder"
        );
        assert_eq!(file.index(), 0, "admission reindexes the physical file");
        assert_zip(file, header_fixture::FILE_WORDS, Some(1))?;
        assert_dos(file, header_fixture::STAMP);
    }
    for base in [header_fixture::V2_WORDS.len(), 1, 0] {
        assert_v2_folder_admission(base)?;
    }
    Ok(())
}

#[test]
fn recovery_never_invents_original_ordinals_after_lost_records() -> TestResult {
    for layout in [zip::Layout::P4k, zip::Layout::Zip] {
        let mut source =
            header_fixture::zip_source(layout, header_fixture::FILE_WORDS, header_fixture::STAMP)?;
        let first = *header_fixture::central_offsets(&source.bytes)
            .first()
            .ok_or("folder CDR")?;
        fixture::put(&mut source.bytes, first, &[0; 4])?;
        source.bytes.truncate(source.directory_end);
        let (_directory, archive) = super::open(&source.bytes, OpenOptions::new().recover(true))?;
        assert_eq!(
            archive.entries().len(),
            1,
            "only the valid file record is recovered"
        );
        let file = archive.entries().first().ok_or("recovered file")?;
        assert_eq!(file.index(), 0, "the recovered Entry index is known");
        assert_zip(file, header_fixture::FILE_WORDS, None)?;
        assert_dos(file, header_fixture::STAMP);
        assert_eq!(
            archive.issues(),
            [(0, EntryIssue::Recovered)],
            "the recovered inventory remains authoritative"
        );
    }
    Ok(())
}

#[test]
fn v2_raw_methods_encryption_words_and_combined_ordinals_keep_the_existing_issue() -> TestResult {
    for timestamp in [header_fixture::STAMP, header_fixture::ZERO_STAMP] {
        for base in [header_fixture::V2_WORDS.len(), 1, 0] {
            let bytes = header_fixture::v2_source(timestamp, base)?;
            for recover in [false, true] {
                let (_directory, archive) =
                    super::open(&bytes, OpenOptions::new().recover(recover))?;
                assert_eq!(
                    archive.entries().len(),
                    header_fixture::V2_WORDS.len(),
                    "base and extension records remain present"
                );
                let records = archive
                    .entries()
                    .iter()
                    .zip(header_fixture::V2_WORDS)
                    .enumerate();
                for (index, (entry, words)) in records {
                    assert_v2(entry, index, words, timestamp)?;
                }
                assert_eq!(
                    archive.issues(),
                    [(0, EntryIssue::UnknownEncryption { value: 0xffff })],
                    "the unknown-encryption issue is neither dropped nor invented by recovery selection"
                );
            }
        }
    }
    Ok(())
}
