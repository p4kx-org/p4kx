// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The effective decoder is observed without probing or rereading source data.

use std::error::Error;
use std::io::Read;

use p4kx_archive::{Archive, EffectiveEncoding};
use p4kx_testkit::{ArchiveBuilder, TestEntry, test_key};

type TestResult = Result<(), Box<dyn Error>>;

fn check_stream(
    builder: &ArchiveBuilder,
    entry: TestEntry,
    expected: EffectiveEncoding,
    encrypted: bool,
    data: &[u8],
) -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("Data.p4k");
    let entry = if encrypted { entry.encrypted() } else { entry };
    builder.clone().entry(entry).write_to(&path)?;
    let archive = Archive::open_with_key(&path, Some(test_key()))?;
    let entry = archive.entries().first().ok_or("Entry missing")?;
    let mut reader = archive.open_entry(entry)?;
    let before = match expected {
        EffectiveEncoding::Zstandard | EffectiveEncoding::PassThrough => {
            EffectiveEncoding::Undetermined
        }
        other => other,
    };
    assert_eq!(
        reader.effective_encoding(),
        before,
        "inspection does not initialize a decoder"
    );
    assert_eq!(reader.read(&mut [])?, 0, "empty reads consume no bytes");
    assert_eq!(
        reader.effective_encoding(),
        before,
        "empty reads do not select a decoder"
    );
    let mut first = [0; 1];
    reader.read_exact(&mut first)?;
    assert_eq!(
        reader.effective_encoding(),
        expected,
        "ordinary reading selects the decoder"
    );
    let mut actual = first.to_vec();
    reader.read_to_end(&mut actual)?;
    assert_eq!(
        actual, data,
        "encoding inspection preserves the complete stream"
    );
    assert_eq!(
        reader.effective_encoding(),
        expected,
        "the selected decoder remains observable"
    );
    Ok(())
}

#[test]
fn selected_encoding_tracks_real_decoder_and_preserves_the_stream() -> TestResult {
    let data = b"synthetic payload\0\0\0".to_vec();
    for builder in [ArchiveBuilder::new(), ArchiveBuilder::v2()] {
        for encrypted in [false, true] {
            for (entry, expected) in [
                (
                    TestEntry::stored("Data\\stored.bin", data.clone()),
                    EffectiveEncoding::Stored,
                ),
                (
                    TestEntry::deflated("Data\\deflate.bin", data.clone()),
                    EffectiveEncoding::Deflate,
                ),
                (
                    TestEntry::zstd("Data\\zstd.bin", data.clone()),
                    EffectiveEncoding::Zstandard,
                ),
                (
                    TestEntry::zstd_without_magic("Data\\plain.bin", data.clone()),
                    EffectiveEncoding::PassThrough,
                ),
            ] {
                check_stream(&builder, entry, expected, encrypted, &data)?;
            }
        }
    }
    Ok(())
}

#[test]
fn unknown_method_100_encoding_is_not_inferred_from_an_empty_logical_entry() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("Data\\empty.bin", Vec::new()))
        .write_to(&path)?;
    let archive = Archive::open(&path)?;
    let entry = archive.entries().first().ok_or("Entry missing")?;
    let mut reader = archive.open_entry(entry)?;
    let mut data = Vec::new();
    reader.read_to_end(&mut data)?;
    assert_eq!(data, Vec::<u8>::new(), "the logical Entry is empty");
    assert_eq!(
        reader.effective_encoding(),
        EffectiveEncoding::Undetermined,
        "an unread prefix gives no encoding evidence"
    );
    Ok(())
}
