// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Read};
use std::sync::atomic::{AtomicBool, Ordering};

use p4kx_extraction::{Archive, MAX_METADATA_CHUNK, MetadataReadError, MetadataSection};
use p4kx_testkit::{ArchiveBuilder, TestEntry};

use crate::metadata_input::MetadataInput;
use crate::{Limits, MediaError};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn metadata_input_clamps_read_buffers_and_charges_actual_bytes() -> TestResult {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("source.p4k");
    let name = "x".repeat(64_000);
    ArchiveBuilder::new()
        .entry(TestEntry::stored(format!("a/{name}"), b""))
        .entry(TestEntry::stored(format!("b/{name}"), b""))
        .entry(TestEntry::stored(format!("c/{name}"), b""))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    let view = archive.metadata_view();
    let span = view
        .span(MetadataSection::Directory)?
        .ok_or("directory span")?;
    let mut charged = 0;
    let mut work = 0;
    let mut charge = |cost: p4kx_extraction::MetadataCost| {
        charged += cost.read_bytes();
        work += cost.work_items();
        Ok(())
    };
    let cancel = AtomicBool::new(false);
    let mut access = view.access(&mut charge, &cancel);
    let mut input = MetadataInput::new(&mut access, span);
    let mut output = vec![0; MAX_METADATA_CHUNK * 3];
    let mut counts = Vec::new();
    loop {
        let count = input.read(&mut output)?;
        if count == 0 {
            break;
        }
        counts.push(count);
    }
    assert_eq!(
        counts,
        [
            MAX_METADATA_CHUNK,
            MAX_METADATA_CHUNK,
            usize::try_from(span.range().len())? - MAX_METADATA_CHUNK * 2
        ]
    );
    assert_eq!(charged, span.range().len());
    assert_eq!(work, 3);
    Ok(())
}

#[test]
fn typed_budget_io_and_actual_cancel_causes_survive_metadata_adapters() -> TestResult {
    let budget = MediaError::from(MetadataReadError::Budget(io::Error::other(
        MediaError::Limit {
            budget: "metadata_work",
            limit: 7,
        },
    )));
    assert!(matches!(
        budget,
        MediaError::Limit {
            budget: "metadata_work",
            limit: 7
        }
    ));
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("source.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("empty", b""))
        .write_to(&path)?;
    let archive = Archive::open(&path)?;
    let view = archive.metadata_view();
    let span = view.span(MetadataSection::ZipEnd)?.ok_or("ZIP end")?;
    let mut charge = |_| Ok(());
    let cancel = AtomicBool::new(false);
    let mut access = view.access(&mut charge, &cancel);
    let mut input = MetadataInput::new(&mut access, span);
    cancel.store(true, Ordering::Relaxed);
    let failure = input
        .read(&mut [0; 1])
        .err()
        .ok_or("cancelled input read")?;
    assert!(matches!(
        crate::error::io(&path, failure),
        MediaError::Cancelled
    ));
    cancel.store(false, Ordering::Relaxed);
    std::fs::OpenOptions::new()
        .write(true)
        .open(&path)?
        .set_len(0)?;
    let failure = input
        .read(&mut [0; 1])
        .err()
        .ok_or("truncated source read")?;
    let MediaError::Metadata(cause) = crate::error::io(&path, failure) else {
        return Err("metadata cause erased".into());
    };
    assert!(
        matches!(cause.as_ref(), MetadataReadError::Read { source, .. } if source.kind() == io::ErrorKind::UnexpectedEof)
    );
    Ok(())
}

#[test]
fn existing_index_limits_keep_the_global_metadata_default_when_absent() -> TestResult {
    let mut value = serde_json::to_value(Limits::default())?;
    value
        .as_object_mut()
        .ok_or("limits object")?
        .remove("metadata_work");
    let limits: Limits = serde_json::from_value(value)?;
    assert_eq!(limits.metadata_work, 100_000_000);
    assert!(
        Limits {
            metadata_work: 0,
            ..limits
        }
        .validate()
        .is_err()
    );
    Ok(())
}
