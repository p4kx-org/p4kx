// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;

use p4kx_extraction::{ArchiveMetadataStatus, ConflictPolicy, Filter};
use p4kx_media::{ExportOptions, ExportOutcome, MediaError, Status};
use serde_json::{Value, json};

use super::layout_fixture;
use super::layout_support::{self as check, field, row};
use super::support::{self, TestResult};

fn diagnostic_options() -> Result<ExportOptions, Box<dyn std::error::Error>> {
    Ok(ExportOptions {
        diagnostic: true,
        include: Some(Filter::parse("*.not-selected")?),
        ..ExportOptions::default()
    })
}

#[test]
fn malformed_lazy_progress_reserved_and_free_rows_make_the_inventory_partial() -> TestResult {
    for malformed in ["progress", "reserved", "free-overflow"] {
        let folder = tempfile::tempdir()?;
        let mut fixture = layout_fixture::zip(&[])?;
        corrupt_table(&mut fixture, malformed)?;
        let archive = layout_fixture::open(folder.path(), &fixture.bytes, false)?;
        assert_eq!(
            archive.metadata().status(),
            ArchiveMetadataStatus::Available,
            "header availability does not validate lazy rows"
        );
        let output = folder.path().join("media");
        let error = p4kx_media::export(
            &archive,
            &output,
            &ExportOptions {
                diagnostic: false,
                ..diagnostic_options()?
            },
        )
        .err()
        .ok_or("normal export accepted malformed lazy row")?;
        assert!(
            projection_loss(&error),
            "lazy row, rather than incomplete enumeration, refuses normal publication"
        );
        assert!(!output.join("media-index.json").exists());
        let ExportOutcome::Incomplete { index, diagnostic } =
            p4kx_media::export(&archive, &output, &diagnostic_options()?)?
        else {
            return Err("malformed table was complete".into());
        };
        assert!(!index.complete);
        assert!(
            index.enumeration_complete,
            "known Entries and malformed table values are independent"
        );
        let rows = support::records(
            diagnostic.parent().ok_or("diagnostic root")?,
            &index,
            "archive.jsonl",
        )?;
        let partial = rows
            .iter()
            .find(|row| {
                row.issues
                    .iter()
                    .any(|issue| issue.code == "archive.metadata-table-value")
            })
            .ok_or("authoritative row diagnosis")?;
        assert_eq!(partial.status, Status::Partial);
        assert!(
            !field(&partial.data, "/raw")?.is_null(),
            "original invalid words remain retained"
        );
        check::validate(&rows)?;
    }
    Ok(())
}

#[test]
fn a_declared_extra_body_beyond_its_section_is_refused_at_archive_open() -> TestResult {
    let folder = tempfile::tempdir()?;
    let mut fixture = layout_fixture::zip(&[0xef, 0xbe, 6, 0, 0x11, 0xff])?;
    let record = *fixture.records.get(1).ok_or("second record")?;
    let start = record + 46 + b"Data/empty.bin".len();
    layout_fixture::put(&mut fixture.bytes, start + 2, &9_u16.to_le_bytes())?;
    let failure = layout_fixture::open(folder.path(), &fixture.bytes, false)
        .err()
        .ok_or("malformed extra body was admitted")?;
    assert!(
        matches!(
            failure.downcast_ref::<p4kx_extraction::ArchiveError>(),
            Some(p4kx_extraction::ArchiveError::Truncated { .. })
        ),
        "the original CDF parser refuses the body before an exporter exists"
    );
    assert_eq!(fs::read(folder.path().join("layout.p4k"))?, fixture.bytes);
    assert!(!folder.path().join("media").exists());
    Ok(())
}

#[test]
fn recovered_records_preserve_raw_bytes_but_never_invent_a_global_layout_or_ordinal() -> TestResult
{
    let folder = tempfile::tempdir()?;
    let mut fixture = layout_fixture::zip(&[])?;
    fixture.bytes.truncate(fixture.end);
    let archive = layout_fixture::open(folder.path(), &fixture.bytes, true)?;
    let output = folder.path().join("media");
    let ExportOutcome::Incomplete { index, diagnostic } =
        p4kx_media::export(&archive, &output, &diagnostic_options()?)?
    else {
        return Err("recovered layout was complete".into());
    };
    let root = diagnostic.parent().ok_or("diagnostic root")?;
    let rows = support::records(root, &index, "archive.jsonl")?;
    assert_eq!(
        field(&row(&rows, "archive-layout")?.data, "/layout")?,
        &json!("recovered-unknown")
    );
    let records: Vec<_> = rows
        .iter()
        .filter(|row| row.subresource.namespace == "archive-directory-record")
        .collect();
    assert!(
        !records.is_empty(),
        "surviving original records are retained"
    );
    for record in records {
        assert_eq!(
            field(&record.data, "/originalDirectoryOrdinal")?,
            &Value::Null
        );
        let reference = field(&record.data, "/raw")?;
        assert_eq!(
            field(reference, "/series/type")?,
            &json!("recovered-record")
        );
        assert_ne!(
            check::raw(root, &index, &rows, &record.source.identity, reference)?.as_slice(),
            b"",
            "each surviving record retains its original nonempty bytes"
        );
    }
    check::validate(&rows)?;
    Ok(())
}

#[test]
fn metadata_work_exhaustion_never_replaces_a_previous_complete_index() -> TestResult {
    let folder = tempfile::tempdir()?;
    let fixture = layout_fixture::zip(&[])?;
    let archive = layout_fixture::open(folder.path(), &fixture.bytes, false)?;
    let output = folder.path().join("media");
    p4kx_media::export(&archive, &output, &ExportOptions::default())?;
    let old = fs::read(output.join("media-index.json"))?;
    let mut limits = p4kx_media::Limits::default();
    limits.metadata_work = 1;
    let options = ExportOptions {
        conflict: ConflictPolicy::Overwrite,
        limits,
        ..ExportOptions::default()
    };
    let error = p4kx_media::export(&archive, &output, &options)
        .err()
        .ok_or("exhausted metadata work published")?;
    assert!(
        matches!(
            leaf(&error),
            MediaError::Limit {
                budget: "metadata_work",
                limit: 1
            }
        ),
        "original typed global budget cause"
    );
    assert_eq!(fs::read(output.join("media-index.json"))?, old);
    Ok(())
}

#[test]
fn a_zero_size_free_extent_is_preserved_without_inventing_an_engine_error() -> TestResult {
    let folder = tempfile::tempdir()?;
    let mut fixture = layout_fixture::zip(&[])?;
    layout_fixture::put(&mut fixture.bytes, fixture.free + 8, &0_u64.to_le_bytes())?;
    let archive = layout_fixture::open(folder.path(), &fixture.bytes, false)?;
    let output = folder.path().join("media");
    let index = support::published(p4kx_media::export(
        &archive,
        &output,
        &ExportOptions::default(),
    )?)?;
    let rows = support::records(&output, &index, "archive.jsonl")?;
    let free = row(&rows, "archive-free-range")?;
    assert_eq!(free.status, Status::Complete);
    assert_eq!(field(&free.data, "/state")?, &json!("zero-size"));
    assert_eq!(field(&free.data, "/extent/declaredLength")?, &json!("0"));
    check::validate(&rows)?;
    Ok(())
}

fn projection_loss(error: &MediaError) -> bool {
    matches!(leaf(error), MediaError::ProjectionLoss { .. })
}

pub(super) fn leaf(error: &MediaError) -> &MediaError {
    match error {
        MediaError::Source { cause, .. } => leaf(cause),
        _ => error,
    }
}

fn corrupt_table(fixture: &mut layout_fixture::Fixture, malformed: &str) -> TestResult {
    match malformed {
        "progress" => layout_fixture::put(
            &mut fixture.bytes,
            fixture.installation + 12,
            &1_u64.to_le_bytes(),
        )?,
        "reserved" => layout_fixture::put(
            &mut fixture.bytes,
            fixture.installation + 20,
            &0x1234_u32.to_le_bytes(),
        )?,
        _ => layout_fixture::put(&mut fixture.bytes, fixture.free, &u64::MAX.to_le_bytes())?,
    }
    Ok(())
}
