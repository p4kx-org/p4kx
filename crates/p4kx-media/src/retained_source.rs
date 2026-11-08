// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::{Path, PathBuf};

use p4kx_extraction::{
    Archive, ConflictPolicy, Entry, ExtractOptions, ExtractionReport, OwnedDirectoryPermit,
};

use crate::hashing;
use crate::output_path;
use crate::raw_output;
use crate::staged_directory::StagedDirectory;
use crate::{ByteScope, Digest, ExtractionCounts, MediaError, MediaWriter, RawReference};

const PAYLOAD_FILE: &str = "payload";

#[cfg(test)]
#[path = "retained_source_tests.rs"]
mod tests;

pub(crate) fn retain(
    archive: &Archive,
    entry: &Entry,
    writer: &mut MediaWriter<'_>,
) -> Result<(RawReference, PathBuf), MediaError> {
    materialize(archive, entry, writer, |payload, permit, digest, writer| {
        raw_output::place(payload, permit, digest, entry.uncompressed_size(), writer)
    })
}

pub(crate) fn with_private<T>(
    archive: &Archive,
    entry: &Entry,
    writer: &mut MediaWriter<'_>,
    consume: impl FnOnce(Digest, &Path, &mut MediaWriter<'_>) -> Result<T, MediaError>,
) -> Result<T, MediaError> {
    materialize(
        archive,
        entry,
        writer,
        |payload, _permit, digest, writer| consume(digest, payload, writer),
    )
}

fn materialize<T>(
    archive: &Archive,
    entry: &Entry,
    writer: &mut MediaWriter<'_>,
    consume: impl FnOnce(
        &Path,
        &OwnedDirectoryPermit,
        Digest,
        &mut MediaWriter<'_>,
    ) -> Result<T, MediaError>,
) -> Result<T, MediaError> {
    writer.checkpoint()?;
    let size = entry.uncompressed_size();
    if size > writer.limits.entry_bytes {
        return Err(MediaError::Limit {
            budget: "entry_bytes",
            limit: writer.limits.entry_bytes,
        });
    }
    writer.reserve_output(size)?;
    writer
        .namespace
        .ensure_namespace_writable(&writer.root, writer.checkpoint.cancel.flag())?;
    if !output_path::directory(&writer.root)? {
        return Err(MediaError::Destination(
            "source staging directory is missing".into(),
        ));
    }
    let temporary = StagedDirectory::create(&writer.root, ".entry-", writer.checkpoint)?;
    let result = (|| {
        extract_one(archive, entry, temporary.path(), writer)?;
        let payload = temporary.path().join(PAYLOAD_FILE);
        if !output_path::file(&payload)? {
            return Err(MediaError::Destination(
                "required Entry payload is missing".into(),
            ));
        }
        let (digest, actual) = hashing::file(
            &payload,
            ByteScope::DecodedEntryPayload,
            size,
            &writer.checkpoint,
        )?;
        if actual != size {
            return Err(MediaError::Contract(
                "decoded source length disagrees with its Entry".into(),
            ));
        }
        consume(&payload, temporary.permit(), digest, writer)
    })();
    temporary.finish(result)
}

fn extract_one(
    archive: &Archive,
    entry: &Entry,
    destination: &Path,
    writer: &MediaWriter<'_>,
) -> Result<(), MediaError> {
    let options = ExtractOptions {
        policy: ConflictPolicy::Overwrite,
        verify: true,
        cancel: Some(writer.checkpoint.cancel),
        watch: Some(writer.checkpoint.watch),
        protection: Some(writer.checkpoint.protection),
        ..ExtractOptions::default()
    };
    let report =
        p4kx_extraction::extract_entry_payload(archive, entry, destination, PAYLOAD_FILE, &options);
    finish_extraction(report, writer)
}

fn finish_extraction(report: ExtractionReport, writer: &MediaWriter<'_>) -> Result<(), MediaError> {
    let counts = ExtractionCounts {
        extracted: report.extracted(),
        skipped: report.skipped().len(),
        failed: report.failures().len(),
        cancelled: report.cancelled().len(),
        archive_changed: report.archive_changed(),
    };
    if !report.failures().is_empty() {
        if let Some(failure) = report.into_failures().into_iter().next() {
            return Err(MediaError::Extraction {
                cause: Box::new(failure.into_error()),
                report: counts,
            });
        }
        return Err(MediaError::Contract(
            "extractor lost its reported failure".into(),
        ));
    }
    writer
        .checkpoint()
        .map_err(|cause| MediaError::ExtractionInterrupted {
            cause: Box::new(cause),
            report: counts,
        })?;
    if !report.is_complete() || report.extracted() != 1 || !report.skipped().is_empty() {
        return Err(MediaError::Contract(
            "one physical Entry was not retained".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "retained_source_fault_tests.rs"]
mod fault_tests;
