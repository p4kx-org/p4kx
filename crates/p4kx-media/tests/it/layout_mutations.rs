// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::path::Path;

use p4kx_extraction::Filter;
use p4kx_media::{ExportOptions, ExportOutcome, Index, MediaRecord, Status};
use serde_json::json;

use super::layout_fixture;
use super::layout_support::{self as check, field, row};
use super::support::{self, TestResult};

#[test]
fn malformed_extra_framing_is_rejected_before_export_without_source_mutation() -> TestResult {
    for state in ["short-header", "length-before-header", "truncated-body"] {
        let folder = tempfile::tempdir()?;
        let mut fixture = layout_fixture::zip(&[0xef, 0xbe, 6, 0, 0xa5, 0xff])?;
        let record = *fixture.records.get(1).ok_or("file record")?;
        if state == "short-header" {
            layout_fixture::put(&mut fixture.bytes, record + 30, &3_u16.to_le_bytes())?;
        } else {
            let length = if state == "length-before-header" {
                2_u16
            } else {
                9
            };
            layout_fixture::put(
                &mut fixture.bytes,
                record + 46 + b"Data/empty.bin".len() + 2,
                &length.to_le_bytes(),
            )?;
        }
        let failure = layout_fixture::open(folder.path(), &fixture.bytes, false)
            .err()
            .ok_or("malformed extra framing was admitted")?;
        let failure = failure
            .downcast_ref::<p4kx_extraction::ArchiveError>()
            .ok_or("original Archive open error")?;
        match state {
            "truncated-body" => assert!(matches!(
                failure,
                p4kx_extraction::ArchiveError::Truncated { .. }
            )),
            _ => assert!(matches!(
                failure,
                p4kx_extraction::ArchiveError::Corrupt { .. }
            )),
        }
        assert_eq!(fs::read(folder.path().join("layout.p4k"))?, fixture.bytes);
        assert!(!folder.path().join("media").exists());
    }
    Ok(())
}

#[test]
fn truncated_comment_retains_its_prefix_and_unparsed_directory_tail_is_diagnosed() -> TestResult {
    for mutation in ["comment", "directory-tail"] {
        let folder = tempfile::tempdir()?;
        let mut fixture = layout_fixture::zip(&[])?;
        corrupt_extent(&mut fixture, mutation)?;
        let archive = layout_fixture::open(folder.path(), &fixture.bytes, false)?;
        let output = folder.path().join("media");
        let options = ExportOptions {
            diagnostic: true,
            include: Some(Filter::parse("*.not-selected")?),
            ..ExportOptions::default()
        };
        let ExportOutcome::Incomplete { index, diagnostic } =
            p4kx_media::export(&archive, &output, &options)?
        else {
            return Err("malformed extent or tail was complete".into());
        };
        let root = diagnostic.parent().ok_or("diagnostic root")?;
        let rows = support::records(root, &index, "archive.jsonl")?;
        if mutation == "directory-tail" {
            let diagnostic = row(&rows, "archive-metadata-diagnostic")?;
            assert_eq!(
                field(&diagnostic.data, "/failure")?,
                &json!("unparsed-directory-tail")
            );
            assert_eq!(field(&diagnostic.data, "/length")?, &json!("1"));
        } else {
            let comment = rows
                .iter()
                .find(|row| {
                    row.subresource.namespace == "archive-section"
                        && row.data.get("section") == Some(&json!("zip-comment"))
                })
                .ok_or("comment section")?;
            assert_eq!(comment.status, Status::Partial);
            assert_eq!(
                check::raw(
                    root,
                    &index,
                    &rows,
                    &comment.source.identity,
                    field(&comment.data, "/raw")?
                )?,
                fixture
                    .bytes
                    .get(fixture.end + 22..)
                    .ok_or("comment prefix")?
            );
        }
        check::validate(&rows)?;
    }
    Ok(())
}

fn corrupt_extent(fixture: &mut layout_fixture::Fixture, mutation: &str) -> TestResult {
    if mutation == "comment" {
        layout_fixture::put(
            &mut fixture.bytes,
            fixture.end + 20,
            &u16::MAX.to_le_bytes(),
        )?;
    } else {
        let directory = *fixture.records.first().ok_or("directory start")?;
        let zip64 = fixture
            .end
            .checked_sub(20 + 56 + 5)
            .ok_or("ZIP64 position")?;
        let size = u64::try_from(zip64 - directory + 1)?;
        fixture.bytes.insert(zip64, 0xa5);
        fixture.end = fixture.end.checked_add(1).ok_or("end-record relocation")?;
        let relocated = zip64.checked_add(1).ok_or("ZIP64 relocation")?;
        let locator = fixture.end.checked_sub(20).ok_or("locator position")?;
        layout_fixture::put(&mut fixture.bytes, relocated + 40, &size.to_le_bytes())?;
        layout_fixture::put(
            &mut fixture.bytes,
            locator + 8,
            &u64::try_from(relocated)?.to_le_bytes(),
        )?;
        layout_fixture::put(
            &mut fixture.bytes,
            fixture.end + 12,
            &u32::try_from(size)?.to_le_bytes(),
        )?;
    }
    Ok(())
}

#[test]
fn an_invalid_v2_trie_extent_refuses_normal_export_but_retains_diagnostic_metadata() -> TestResult {
    let folder = tempfile::tempdir()?;
    let mut fixture = layout_fixture::v2()?;
    layout_fixture::put(
        &mut fixture.bytes,
        fixture.end + 80,
        &u64::MAX.to_le_bytes(),
    )?;
    let archive = layout_fixture::open(folder.path(), &fixture.bytes, false)?;
    assert_eq!(
        archive.metadata().status(),
        p4kx_extraction::ArchiveMetadataStatus::Malformed
    );
    let output = folder.path().join("media");
    let options = ExportOptions {
        include: Some(Filter::parse("*.not-selected")?),
        ..ExportOptions::default()
    };
    let failure = p4kx_media::export(&archive, &output, &options)
        .err()
        .ok_or("invalid trie was complete")?;
    assert!(matches!(
        super::layout_health::leaf(&failure),
        p4kx_media::MediaError::ProjectionLoss { .. }
    ));
    assert!(!output.join("media-index.json").exists());
    let options = ExportOptions {
        diagnostic: true,
        ..options
    };
    let ExportOutcome::Incomplete { index, diagnostic } =
        p4kx_media::export(&archive, &output, &options)?
    else {
        return Err("invalid v2 metadata claimed complete publication".into());
    };
    let root = diagnostic.parent().ok_or("diagnostic root")?;
    let rows = support::records(root, &index, "archive.jsonl")?;
    verify_v2_declarations(&rows)?;
    verify_v2_trailer(root, &index, &rows, &fixture)?;
    check::validate(&rows)?;
    Ok(())
}

fn verify_v2_declarations(rows: &[MediaRecord]) -> TestResult {
    let layout = row(rows, "archive-layout")?;
    assert_eq!(
        layout.status,
        Status::Partial,
        "invalid trie declarations make the layout projection partial"
    );
    assert_eq!(
        field(&layout.data, "/v2/trieOffset")?,
        &json!(u64::MAX.to_string()),
        "the overflowing trie offset remains the original u64 word"
    );
    let trie = rows
        .iter()
        .find(|row| {
            row.subresource.namespace == "archive-section"
                && row.data.get("section") == Some(&json!("v2-trie"))
        })
        .ok_or("trie declaration")?;
    assert_eq!(
        trie.status,
        Status::Partial,
        "an invalid trie extent is partial"
    );
    assert_eq!(
        field(&trie.data, "/region/state")?,
        &json!("overflow"),
        "offset-plus-length overflow remains explicit"
    );
    assert!(
        field(&trie.data, "/raw")?.is_null(),
        "an overflowing extent does not invent retained raw bytes"
    );
    Ok(())
}

fn verify_v2_trailer(
    root: &Path,
    index: &Index,
    rows: &[MediaRecord],
    fixture: &layout_fixture::Fixture,
) -> TestResult {
    let trailer = rows
        .iter()
        .find(|row| {
            row.subresource.namespace == "archive-section"
                && row.data.get("section") == Some(&json!("v2-trailer"))
        })
        .ok_or("retained trailer")?;
    assert_eq!(
        check::raw(
            root,
            index,
            rows,
            &trailer.source.identity,
            field(&trailer.data, "/raw")?
        )?,
        fixture
            .bytes
            .get(fixture.end..)
            .ok_or("source trailer bytes")?,
        "the valid trailer remains retained despite its invalid trie declaration"
    );
    Ok(())
}
