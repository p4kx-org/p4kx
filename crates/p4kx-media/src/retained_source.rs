// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::path::{Path, PathBuf};

use p4kx_extraction::{Archive, ConflictPolicy, Entry, ExtractOptions, ExtractionReport};

use crate::error;
use crate::hashing;
use crate::output_path;
use crate::raw_output;
use crate::{ByteScope, ExtractionCounts, MediaError, MediaWriter, RawReference};

#[cfg(test)]
#[path = "retained_source_tests.rs"]
mod tests;

pub(crate) fn retain(
    archive: &Archive,
    entry: &Entry,
    writer: &mut MediaWriter<'_>,
) -> Result<(RawReference, PathBuf), MediaError> {
    writer.checkpoint()?;
    let size = entry.uncompressed_size();
    if size > writer.limits.entry_bytes {
        return Err(MediaError::Limit {
            budget: "entry_bytes",
            limit: writer.limits.entry_bytes,
        });
    }
    writer.reserve_output(size)?;
    if !output_path::directory(&writer.root)? {
        return Err(MediaError::Destination(
            "source staging directory is missing".into(),
        ));
    }
    let temporary = tempfile::Builder::new()
        .prefix(".entry-")
        .tempdir_in(&writer.root)
        .map_err(|source| error::io(&writer.root, source))?;
    extract_one(archive, entry, temporary.path(), writer)?;
    let payload = only_payload(temporary.path())?;
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
    let retained = raw_output::place(&payload, digest, size, writer)?;
    temporary
        .close()
        .map_err(|source| error::io(&writer.root, source))?;
    Ok(retained)
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
        ..ExtractOptions::default()
    };
    let report = p4kx_extraction::extract(archive, &[entry], destination, &options);
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

fn only_payload(root: &Path) -> Result<PathBuf, MediaError> {
    let mut current = root.to_path_buf();
    loop {
        if !output_path::directory(&current)? {
            return Err(MediaError::Destination(
                "extraction staging directory is missing".into(),
            ));
        }
        let mut entries = fs::read_dir(&current).map_err(|source| error::io(&current, source))?;
        let entry = entries
            .next()
            .transpose()
            .map_err(|source| error::io(&current, source))?
            .ok_or_else(|| MediaError::Contract("extraction produced no payload".into()))?;
        if entries.next().is_some() {
            return Err(MediaError::Contract(
                "single Entry extraction produced multiple files".into(),
            ));
        }
        let metadata = entry
            .file_type()
            .map_err(|source| error::io(&entry.path(), source))?;
        if metadata.is_file() {
            if !output_path::file(&entry.path())? {
                return Err(MediaError::Destination(
                    "extracted payload is missing".into(),
                ));
            }
            return Ok(entry.path());
        }
        if !metadata.is_dir() {
            return Err(MediaError::Contract(
                "extracted payload is not a regular file".into(),
            ));
        }
        current = entry.path();
    }
}
