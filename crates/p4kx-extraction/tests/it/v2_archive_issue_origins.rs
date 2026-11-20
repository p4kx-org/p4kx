// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_extraction::{
    Archive, ArchiveSummary, EntryIssue, FieldOrigin, FieldPath, InputScope, MetadataRange,
    ObservedV2DataOutsideFile, OpenOptions, SourceEncryptionState, SourceInventory, SourceLocation,
    SourceUseRole,
};
use p4kx_testkit::{ArchiveBuilder, TestEntry};
use serde_json::Value;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const V2_TRAILER_BYTES: usize = 175;
const V2_DIRECTORY_POINTER: usize = 16;
const V2_COMPRESSED_SIZE: usize = 10;
const V2_DATA_OFFSET: usize = 26;
const V2_PAYLOAD_END: u64 = 64;
#[path = "recovered_archive_issue_origins.rs"]
mod recovered;

const ENTRY: &str = "Data\\fixture.txt";

#[test]
fn v2_archive_issue_origins_bind_the_original_cached_words_and_ranges() -> TestResult {
    let (_directory, archive) = open(&v2_fixture(0, 5)?, true)?;
    assert_eq!(
        archive.issues(),
        [(0, EntryIssue::DataOutsideFile)],
        "normal v2 reports the admitted Entry whose range crosses the trailer boundary",
    );
    let bounds = archive
        .observed_v2_data_outside_file(0)
        .ok_or("cached v2 issue operands")?;
    check_cached_operand_ranges(&archive, &bounds)?;
    check_published_operand_origins(&archive, &bounds)
}

fn check_cached_operand_ranges(
    archive: &Archive,
    bounds: &ObservedV2DataOutsideFile<'_>,
) -> TestResult {
    let entry = archive.entries().first().ok_or("admitted Entry")?;
    assert!(
        std::ptr::eq(bounds.entry().entry(), entry),
        "bind the exact admitted Entry",
    );
    assert!(
        entry.is_encrypted(),
        "the fixture declares encrypted payload"
    );
    assert!(
        archive.observed_v2_data_outside_file(1).is_none(),
        "a missing issue position must not manufacture evidence",
    );
    let record = entry.metadata().record_range();
    let v2 = archive
        .metadata()
        .v2_directory()
        .ok_or("observed v2 trailer")?;
    let trailer = v2.trailer();
    for (word, value, offset) in [
        (
            bounds.data_offset(),
            0,
            record.offset() + u64::try_from(V2_DATA_OFFSET)?,
        ),
        (
            bounds.compressed_size(),
            5,
            record.offset() + u64::try_from(V2_COMPRESSED_SIZE)?,
        ),
        (bounds.payload_end(), 4, trailer.offset() + V2_PAYLOAD_END),
    ] {
        assert_eq!(
            word.value(),
            value,
            "retain the exact admitted original word"
        );
        assert_eq!(
            word.range().offset(),
            offset,
            "use the correct source offset"
        );
        assert_eq!(word.range().len(), 8, "a v2 operand is an eight-byte word");
    }
    assert!(
        bounds.compressed_size().value() < archive.file_len(),
        "the entry crosses payloadEnd while its proposed extent stays inside the file",
    );
    Ok(())
}

fn check_published_operand_origins(
    archive: &Archive,
    bounds: &ObservedV2DataOutsideFile<'_>,
) -> TestResult {
    let (_, output) = ArchiveSummary::project_with_issue_origins(archive, "", 1)?;
    let origin = output
        .field_origins
        .get(&FieldPath::parse("/issues/0/issue")?)
        .ok_or("v2 issue field origin")?;
    assert_eq!(
        origin.contains_encrypted_source(),
        Some(false),
        "only clear directory/trailer metadata contributes to the diagnostic",
    );
    assert!(
        origin.source_set_complete(),
        "all original bound operands contribute"
    );
    assert_eq!(
        origin.source_uses_refs().len(),
        3,
        "one use per original operand"
    );

    let inventory = serde_json::to_value(&output.provenance)?;
    let sources = inventory
        .get("sources")
        .and_then(Value::as_array)
        .ok_or("serialized evidence owners")?;
    let expected = [
        (
            "data_offset",
            bounds.data_offset().range(),
            "entryDirectoryRecord",
            "archiveEntry",
        ),
        (
            "compressed_size",
            bounds.compressed_size().range(),
            "entryDirectoryRecord",
            "archiveEntry",
        ),
        (
            "v2/payloadEnd",
            bounds.payload_end().range(),
            "archiveV2Trailer",
            "archiveDirectory",
        ),
    ];
    for (field, range, namespace, owner) in expected {
        check_use(
            &output.provenance,
            origin,
            sources,
            field,
            range,
            namespace,
            owner,
        )?;
    }
    Ok(())
}

#[test]
fn v2_archive_issue_origins_do_not_replace_overflow_or_exact_boundary() -> TestResult {
    let (_overflow, archive) = open(&v2_fixture(1, u64::MAX)?, false)?;
    let bounds = archive
        .observed_v2_data_outside_file(0)
        .ok_or("admitted overflow issue")?;
    assert_eq!(
        (
            bounds.data_offset().value(),
            bounds.compressed_size().value()
        ),
        (1, u64::MAX),
        "retain original operands without synthesizing a saturated end",
    );
    assert!(
        bounds
            .data_offset()
            .value()
            .checked_add(bounds.compressed_size().value())
            .is_none(),
        "this fixture exercises arithmetic overflow",
    );

    let (_exact, archive) = open(&v2_fixture(0, 4)?, false)?;
    assert!(
        archive.issues().is_empty(),
        "ending exactly at payloadEnd is valid"
    );
    assert!(
        archive.observed_v2_data_outside_file(0).is_none(),
        "no diagnostic evidence is present for the valid range",
    );
    Ok(())
}

fn check_use(
    inventory: &SourceInventory,
    origin: &FieldOrigin,
    sources: &[Value],
    field: &str,
    range: MetadataRange,
    namespace: &str,
    owner_kind: &str,
) -> TestResult {
    let expected = SourceLocation::Subresource {
        namespace: namespace.into(),
        indices: vec![range.offset(), range.offset() + range.len()],
        field: field.into(),
    };
    let used = inventory
        .iter_uses()
        .find(|used| origin.source_uses_refs().contains(used.id()) && used.location() == &expected)
        .ok_or("exact source operand binding")?;
    assert_eq!(
        used.role(),
        SourceUseRole::Conversion,
        "{field} is compared"
    );
    assert_eq!(
        used.scope(),
        InputScope::ArchiveDirectoryMetadata,
        "{field} comes from directory metadata",
    );
    assert_eq!(
        used.effective_encryption(),
        SourceEncryptionState::Unencrypted,
        "{field} is clear despite the encrypted leaf",
    );
    let owner = sources
        .iter()
        .find(|row| row.get("id").and_then(Value::as_str) == Some(used.source().as_str()))
        .ok_or("exact operand owner")?;
    assert_eq!(
        owner.get("kind").and_then(Value::as_str),
        Some(owner_kind),
        "{field} belongs to the correct physical source",
    );
    Ok(())
}

fn put_word(bytes: &mut [u8], offset: usize, value: u64) -> TestResult {
    bytes
        .get_mut(offset..offset + size_of::<u64>())
        .ok_or("synthetic original word")?
        .copy_from_slice(&value.to_le_bytes());
    Ok(())
}

fn v2_fixture(offset: u64, size: u64) -> TestResult<Vec<u8>> {
    let mut bytes = ArchiveBuilder::v2()
        .entry(TestEntry::stored(ENTRY, b"data").with_encryption_field(1))
        .build();
    let trailer = bytes
        .len()
        .checked_sub(V2_TRAILER_BYTES)
        .ok_or("v2 trailer")?;
    let pointer = trailer + V2_DIRECTORY_POINTER;
    let directory = usize::try_from(u64::from_le_bytes(
        bytes
            .get(pointer..pointer + size_of::<u64>())
            .ok_or("v2 directory position")?
            .try_into()?,
    ))?;
    put_word(&mut bytes, directory + V2_COMPRESSED_SIZE, size)?;
    put_word(&mut bytes, directory + V2_DATA_OFFSET, offset)?;
    Ok(bytes)
}

fn open(bytes: &[u8], recover: bool) -> TestResult<(tempfile::TempDir, Archive)> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Data.p4k");
    std::fs::write(&path, bytes)?;
    Ok((
        folder,
        Archive::open_with(path, OpenOptions::new().recover(recover))?,
    ))
}
