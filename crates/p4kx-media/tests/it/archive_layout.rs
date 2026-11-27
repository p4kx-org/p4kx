// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Filter;
use p4kx_media::{ExportOptions, Identity, Limits};
use p4kx_testkit::{ArchiveBuilder, TestEntry};
use serde_json::json;

use super::layout_fixture;
use super::layout_support::{self as check, field, number, row};
use super::support::{self, TestResult};

#[test]
fn zero_signature_sector_and_invalid_dos_calendar_remain_authored_observations() -> TestResult {
    let folder = tempfile::tempdir()?;
    let mut fixture = layout_fixture::v2()?;
    let record = *fixture.records.get(1).ok_or("file record")?;
    layout_fixture::put(&mut fixture.bytes, record + 42, &[0; 128])?;
    layout_fixture::put(&mut fixture.bytes, record + 4, &0_u16.to_le_bytes())?;
    layout_fixture::put(&mut fixture.bytes, fixture.end + 96, &0_u64.to_le_bytes())?;
    let archive = layout_fixture::open(folder.path(), &fixture.bytes, false)?;
    let output = folder.path().join("media");
    let options = ExportOptions {
        include: Some(Filter::parse("*.not-selected")?),
        ..ExportOptions::default()
    };
    let index = support::published(p4kx_media::export(&archive, &output, &options)?)?;
    let rows = support::records(&output, &index, "archive.jsonl")?;
    let layout = row(&rows, "archive-layout")?;
    assert_eq!(number(&layout.data, "/v2/sectorSize")?, 0);
    let record = rows
        .iter()
        .find(|record| {
            record.subresource.namespace == "archive-directory-record"
                && record.data.get("originalDirectoryOrdinal") == Some(&json!("1"))
        })
        .ok_or("file directory record")?;
    assert_eq!(
        check::raw(
            &output,
            &index,
            &rows,
            &record.source.identity,
            field(&record.data, "/signature")?
        )?,
        vec![0; 128]
    );
    assert_eq!(
        field(&record.data, "/metadata/dos_timestamp/raw_date")?,
        &json!(0)
    );
    assert_eq!(
        field(&record.data, "/metadata/dos_timestamp/calendar/state")?,
        &json!("invalid")
    );
    assert!(
        index.complete,
        "opaque zeros and calendar-invalid source words are not invented format errors"
    );
    Ok(())
}

#[test]
fn zip64_layout_unknown_duplicate_extras_and_raw_sidecars_are_exact() -> TestResult {
    let folder = tempfile::tempdir()?;
    let fixture = layout_fixture::zip(&[0xef, 0xbe, 6, 0, 0xa5, 0xff, 0xef, 0xbe, 4, 0])?;
    let archive = layout_fixture::open(folder.path(), &fixture.bytes, false)?;
    let output = folder.path().join("media");
    let mut limits = Limits::default();
    limits.chunk_bytes = 7;
    let options = ExportOptions {
        limits,
        ..ExportOptions::default()
    };
    let index = support::published(p4kx_media::export(&archive, &output, &options)?)?;
    let rows = support::records(&output, &index, "archive.jsonl")?;
    check::validate(&rows)?;
    let layout = row(&rows, "archive-layout")?;
    assert_eq!(field(&layout.data, "/zip/end/disk")?, &json!(7));
    assert_eq!(field(&layout.data, "/zip/zip64/disk")?, &json!(11));
    assert_eq!(field(&layout.data, "/zip/locator/disks")?, &json!(19));
    assert_eq!(
        field(&layout.data, "/zip/installation/sectorSize")?,
        &json!(2048)
    );
    let extras: Vec<_> = rows
        .iter()
        .filter(|row| row.subresource.namespace == "archive-extra")
        .collect();
    assert_eq!(
        extras.len(),
        2,
        "duplicate unknown IDs remain separate authored fields"
    );
    for (ordinal, extra) in extras.into_iter().enumerate() {
        assert_eq!(field(&extra.data, "/id")?, &json!(0xbeef));
        assert_eq!(number(&extra.data, "/ordinal")?, u64::try_from(ordinal)?);
    }
    assert_raw_records(&output, &index, &rows, &fixture.bytes)?;
    assert_eq!(
        index.source_entries.enumerated,
        u64::try_from(archive.entries().len())?,
        "metadata rows add no physical Entries"
    );
    assert_eq!(
        index.source_entries.written,
        u64::try_from(archive.entries().len())?
    );
    Ok(())
}

#[test]
fn v2_split_directories_signatures_zero_slot_and_both_trailer_slots_are_retained() -> TestResult {
    let folder = tempfile::tempdir()?;
    let fixture = layout_fixture::v2()?;
    let archive = layout_fixture::open(folder.path(), &fixture.bytes, false)?;
    let output = folder.path().join("media");
    let mut limits = Limits::default();
    limits.chunk_bytes = 19;
    let options = ExportOptions {
        include: Some(Filter::parse("*.not-selected")?),
        limits,
        ..ExportOptions::default()
    };
    let index = support::published(p4kx_media::export(&archive, &output, &options)?)?;
    let rows = support::records(&output, &index, "archive.jsonl")?;
    check::validate(&rows)?;
    let layout = row(&rows, "archive-layout")?;
    assert_eq!(number(&layout.data, "/v2/baseCount")?, 1);
    assert_eq!(number(&layout.data, "/v2/extensionCount")?, 1);
    assert_eq!(number(&layout.data, "/v2/sectorSize")?, 8192);
    assert_v2_records(&output, &index, &rows)?;
    assert_v2_sections(&output, &index, &rows)?;
    assert_eq!(index.source_entries.written, 0);
    assert_eq!(
        index.source_entries.skipped,
        u64::try_from(archive.entries().len())?
    );
    Ok(())
}

#[test]
fn nested_layout_uses_its_parent_container_identity_and_retains_omitted_folder_records()
-> TestResult {
    let folder = tempfile::tempdir()?;
    let inner = layout_fixture::zip(&[])?;
    let path = folder.path().join("outer.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("same.socpak", inner.bytes.as_slice()))
        .entry(TestEntry::stored("same.socpak", inner.bytes.as_slice()))
        .write_to(&path)?;
    let archive = p4kx_extraction::Archive::open(path)?;
    let output = folder.path().join("media");
    let index = support::published(p4kx_media::export(
        &archive,
        &output,
        &ExportOptions::default(),
    )?)?;
    let rows = support::records(&output, &index, "archive.jsonl")?;
    let layouts: Vec<_> = rows
        .iter()
        .filter(|row| row.subresource.namespace == "archive-layout")
        .collect();
    assert_eq!(layouts.len(), 3, "one callback per already opened source");
    assert_nested_sources(&output, &index, &rows, &layouts, inner.bytes.len())?;
    assert_eq!(
        index.source_entries.enumerated, 4,
        "two parent containers and their one admitted child each"
    );
    Ok(())
}

fn assert_raw_records(
    output: &std::path::Path,
    index: &p4kx_media::Index,
    rows: &[p4kx_media::MediaRecord],
    bytes: &[u8],
) -> TestResult {
    for record in rows
        .iter()
        .filter(|row| row.subresource.namespace == "archive-directory-record")
    {
        assert_eq!(
            record.source.raw, None,
            "directory source is not claimed as a retained complete Archive"
        );
        let reference = field(&record.data, "/raw")?;
        let offset = usize::try_from(number(reference, "/range/offset")?)?;
        let length = usize::try_from(number(reference, "/range/length")?)?;
        let expected = bytes
            .get(offset..offset + length)
            .ok_or("authored raw record")?;
        assert_eq!(
            check::raw(output, index, rows, &record.source.identity, reference)?,
            expected,
            "each retained record matches its exact owning-source slice"
        );
    }
    Ok(())
}

fn assert_v2_records(
    output: &std::path::Path,
    index: &p4kx_media::Index,
    rows: &[p4kx_media::MediaRecord],
) -> TestResult {
    let records: Vec<_> = rows
        .iter()
        .filter(|row| row.subresource.namespace == "archive-directory-record")
        .collect();
    assert_eq!(records.len(), 2, "both original v2 records are projected");
    for (position, record) in records.into_iter().enumerate() {
        let signature = check::raw(
            output,
            index,
            rows,
            &record.source.identity,
            field(&record.data, "/signature")?,
        )?;
        assert_eq!(
            signature,
            vec![0x85 + u8::try_from(position)?; 128],
            "all authored signature bytes stay with their original record"
        );
        let digest = check::raw(
            output,
            index,
            rows,
            &record.source.identity,
            field(&record.data, "/storedDigest")?,
        )?;
        assert_eq!(
            digest,
            if position == 0 {
                vec![0; 32]
            } else {
                vec![0x6a; 32]
            },
            "zero and nonzero digest slots retain their exact authored bytes"
        );
    }
    Ok(())
}

fn assert_v2_sections(
    output: &std::path::Path,
    index: &p4kx_media::Index,
    rows: &[p4kx_media::MediaRecord],
) -> TestResult {
    for (section, expected) in [
        ("v2-opaque-first", vec![0x13; 32]),
        ("v2-opaque-second", vec![0xdb; 32]),
        ("v2-trie", vec![0, 0xee, 0xff, 0x73]),
    ] {
        let section = rows
            .iter()
            .find(|row| {
                row.subresource.namespace == "archive-section"
                    && row.data.get("section") == Some(&json!(section))
            })
            .ok_or("section row")?;
        assert_eq!(
            check::raw(
                output,
                index,
                rows,
                &Identity::Root,
                field(&section.data, "/raw")?
            )?,
            expected,
            "both opaque trailer slots and the trie retain exact source bytes"
        );
    }
    Ok(())
}

fn assert_nested_sources(
    output: &std::path::Path,
    index: &p4kx_media::Index,
    rows: &[p4kx_media::MediaRecord],
    layouts: &[&p4kx_media::MediaRecord],
    inner_length: usize,
) -> TestResult {
    for parent_index in [0, 1] {
        let parent = layouts.iter().find(|row| matches!(&row.source.identity, Identity::Entry { containers, entry } if containers.is_empty() && entry.entry_index == parent_index)).ok_or("physical parent identity")?;
        assert_eq!(
            parent.source.size,
            u64::try_from(inner_length)?,
            "nested source size is not root file size"
        );
        let records: Vec<_> = rows
            .iter()
            .filter(|row| {
                row.subresource.namespace == "archive-directory-record"
                    && row.source.identity == parent.source.identity
            })
            .collect();
        assert_eq!(
            records.len(),
            2,
            "omitted folder remains an original metadata record"
        );
        let folder_row = records
            .iter()
            .find(|row| row.data.get("originalDirectoryOrdinal") == Some(&json!("0")))
            .ok_or("original folder record")?;
        assert_eq!(
            field(&folder_row.data, "/admittedEntryIndex")?,
            &serde_json::Value::Null,
            "omitted folder records have no admitted physical Entry index"
        );
        let reference = field(&folder_row.data, "/raw")?;
        let bytes = check::raw(output, index, rows, &parent.source.identity, reference)?;
        assert!(
            bytes.starts_with(b"PK\x01\x02"),
            "raw reference addresses its immediate source"
        );
    }
    Ok(())
}
