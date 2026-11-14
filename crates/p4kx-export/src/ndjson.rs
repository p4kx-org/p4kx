// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use crate::ndjson_output::Output;
use p4kx_extraction::{
    Annotations, CancelToken, ConflictPolicy, DataForge, ExtractionError, Record,
    RecordExportError, RecordExportOptions, RecordFailure, TreeBuilder,
};
use rayon::prelude::*;

use crate::line_writer::LineWriter;
use crate::{NDJSON_FILE, RecordExportSummary, RecordForm};

const BATCH_LEN: usize = 4096;
const BATCH_BYTES: usize = 64 << 20;
const PART_LEN: usize = 64;
const READ_BUFFER_LEN: usize = 64 << 10;

/// One selected Record, in the order of the file.
struct Line<'r, 'f> {
    record: &'r Record<'f>,
}

/// The lines of one part of a batch and what happened to its Records.
struct Part<'r, 'f> {
    bytes: Vec<u8>,
    error_counts: Vec<usize>,
    failures: Vec<(&'r Record<'f>, RecordExportError)>,
}

struct Scratch<'f> {
    trees: TreeBuilder<'f>,
    lines: LineWriter,
}

/// Writes a single-file form of a Record export, see [`crate::write_record_export`].
pub(crate) fn write(
    forge: &DataForge<'_>,
    records: &[Record<'_>],
    form: RecordForm,
    target: &Path,
    options: &RecordExportOptions<'_>,
) -> RecordExportSummary {
    let mut summary = RecordExportSummary {
        form,
        ..RecordExportSummary::default()
    };
    let lines = ordered_lines(forge, records, &mut summary);
    let foreign = summary.failures.len();
    if options.cancel.is_some_and(CancelToken::is_cancelled) {
        summary.cancelled = lines.len();
        return summary;
    }
    if let Err(error) = run(&lines, target, options, &mut summary) {
        discard(&mut summary);
        if matches!(&error, ExtractionError::Installation(cause) if cause.is_cancelled()) {
            summary.cancelled = lines
                .len()
                .saturating_sub(summary.failures.len().saturating_sub(foreign));
        } else {
            summary.error = Some(error);
        }
    }
    summary
}

fn run(
    lines: &[Line<'_, '_>],
    target: &Path,
    options: &RecordExportOptions<'_>,
    summary: &mut RecordExportSummary,
) -> Result<(), ExtractionError> {
    p4kx_extraction::check_protected_output(target, options.protection, options.cancel)?;
    let file = summary.form.file().unwrap_or(NDJSON_FILE);
    let destination = target.join(file);
    let existing = existing_file(&destination)?;
    if existing.is_some() && options.policy == ConflictPolicy::Skip {
        summary.skipped.push(file.to_owned());
        return Ok(());
    }
    let mut output = Output::create(target, options)?;
    let result = write_output(
        lines,
        &mut output,
        &destination,
        existing,
        options,
        summary,
        file,
    );
    output.complete(result, options.cancel)
}

fn write_output(
    lines: &[Line<'_, '_>],
    output: &mut Output,
    destination: &Path,
    existing: Option<u64>,
    options: &RecordExportOptions<'_>,
    summary: &mut RecordExportSummary,
    file: &str,
) -> Result<(), ExtractionError> {
    let mut header = Vec::new();
    LineWriter::new(summary.form)
        .header(options.texts.is_some(), &mut header)
        .and_then(|()| output.append(&header))
        .map_err(|error| write_error(destination, error))?;
    if !stream(lines, output, destination, options, summary)? {
        return Ok(());
    }
    let kept = match existing {
        Some(len) if options.policy == ConflictPolicy::IfChanged && len == output.len => {
            file_crc(destination)? == output.crc
        }
        _ => false,
    };
    if kept {
        summary.skipped.push(file.to_owned());
        summary.written = 0;
        return Ok(());
    }
    if output.persist(destination, existing.is_some(), options)? {
        summary.skipped.push(file.to_owned());
        discard(summary);
    }
    Ok(())
}

/// Builds and writes `lines` in batches of at most [`BATCH_LEN`] Records, sized from the previous
/// batch to hold about [`BATCH_BYTES`]; returns `false` when the run was cancelled.
fn stream(
    lines: &[Line<'_, '_>],
    output: &mut Output,
    destination: &Path,
    options: &RecordExportOptions<'_>,
    summary: &mut RecordExportSummary,
) -> Result<bool, ExtractionError> {
    let form = summary.form;
    let mut failed_lines = 0_usize;
    let mut start = 0;
    let mut batch_len = BATCH_LEN;
    while start < lines.len() {
        let end = start.saturating_add(batch_len).min(lines.len());
        let batch = lines.get(start..end).unwrap_or_default();
        start = end;
        if options.cancel.is_some_and(CancelToken::is_cancelled) {
            discard(summary);
            summary.cancelled = lines.len().saturating_sub(failed_lines);
            return Ok(false);
        }
        let parts: Vec<Part<'_, '_>> = batch
            .par_chunks(PART_LEN)
            .map_init(
                || Scratch {
                    trees: TreeBuilder::new(),
                    lines: LineWriter::new(form),
                },
                |scratch, lines| {
                    write_part(
                        scratch,
                        lines,
                        options.progress,
                        options.annotations(),
                        form,
                    )
                },
            )
            .collect();
        batch_len = next_batch_len(batch.len(), parts.iter().map(|part| part.bytes.len()).sum());
        for part in parts {
            output
                .append(&part.bytes)
                .map_err(|error| write_error(destination, error))?;
            failed_lines = failed_lines.saturating_add(part.failures.len());
            summary.written = summary.written.saturating_add(part.error_counts.len());
            for errors in part.error_counts {
                summary.error_values = summary.error_values.saturating_add(errors);
                summary.records_with_errors = summary
                    .records_with_errors
                    .saturating_add(usize::from(errors > 0));
            }
            summary.failures.extend(
                part.failures
                    .into_iter()
                    .map(|(record, error)| failure(record, error)),
            );
        }
    }
    Ok(true)
}

/// Returns how many Records the next batch takes after `len` Records gave `bytes`.
fn next_batch_len(len: usize, bytes: usize) -> usize {
    len.saturating_mul(BATCH_BYTES)
        .checked_div(bytes)
        .unwrap_or(BATCH_LEN)
        .clamp(PART_LEN, BATCH_LEN)
}

/// Forgets the Records counted as written, since the file is not put in place.
const fn discard(summary: &mut RecordExportSummary) {
    summary.written = 0;
    summary.error_values = 0;
    summary.records_with_errors = 0;
}

/// Returns the selected Records of `forge` in Record order ([`DataForge::path_order`]), each once,
/// with those whose record path cannot be read last in definition order; Records of another
/// DataForge become failures.
fn ordered_lines<'r, 'f>(
    forge: &DataForge<'_>,
    records: &'r [Record<'f>],
    summary: &mut RecordExportSummary,
) -> Vec<Line<'r, 'f>> {
    let mut selected: Vec<Option<&'r Record<'f>>> = vec![None; forge.record_count()];
    for record in records {
        match selected.get_mut(record.index() as usize) {
            Some(slot) if record.is_from(forge) => *slot = Some(record),
            _ => summary
                .failures
                .push(failure(record, RecordExportError::OtherDataForge)),
        }
    }
    let mut lines: Vec<Line<'r, 'f>> = forge
        .path_order()
        .iter()
        .filter_map(|index| selected.get_mut(*index as usize).and_then(Option::take))
        .map(|record| Line { record })
        .collect();
    lines.extend(selected.into_iter().flatten().map(|record| Line { record }));
    lines
}

fn write_part<'r, 'f>(
    scratch: &mut Scratch<'f>,
    lines: &[Line<'r, 'f>],
    progress: Option<&(dyn Fn() + Sync)>,
    annotations: Annotations<'_>,
    form: RecordForm,
) -> Part<'r, 'f> {
    let mut part = Part {
        bytes: Vec::new(),
        error_counts: Vec::with_capacity(lines.len()),
        failures: Vec::new(),
    };
    for line in lines {
        match scratch.trees.build(line.record) {
            Ok(tree) => {
                let start = part.bytes.len();
                match scratch.lines.write(&tree, annotations, &mut part.bytes) {
                    Ok(()) => part.error_counts.push(tree.error_count()),
                    Err(error) => {
                        part.bytes.truncate(start);
                        part.failures.push((
                            line.record,
                            ExtractionError::Write {
                                path: PathBuf::from(form.file().unwrap_or(NDJSON_FILE)),
                                source: error,
                            }
                            .into(),
                        ));
                    }
                }
            }
            Err(error) => part.failures.push((line.record, error.into())),
        }
        if let Some(progress) = progress {
            progress();
        }
    }
    part
}

/// Returns the size of the file at `path`, or `None` when there is none; a folder or link there
/// is an error.
fn existing_file(path: &Path) -> Result<Option<u64>, ExtractionError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() => Ok(Some(metadata.len())),
        Ok(_) => Err(write_error(
            path,
            io::Error::new(
                io::ErrorKind::AlreadyExists,
                "a folder or link is in the way",
            ),
        )),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(read_existing_error(path, source)),
    }
}

fn file_crc(path: &Path) -> Result<u32, ExtractionError> {
    let mut buffer = vec![0; READ_BUFFER_LEN];
    let mut file = File::open(path).map_err(|source| read_existing_error(path, source))?;
    let mut crc = 0;
    loop {
        match file.read(&mut buffer) {
            Ok(0) => return Ok(crc),
            Ok(count) => crc = crc32c::crc32c_append(crc, buffer.get(..count).unwrap_or_default()),
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(source) => return Err(read_existing_error(path, source)),
        }
    }
}

fn failure(record: &Record<'_>, error: RecordExportError) -> RecordFailure {
    RecordFailure::new(
        record.guid(),
        record.record_path().unwrap_or_default().to_owned(),
        error,
    )
}

fn write_error(path: &Path, source: io::Error) -> ExtractionError {
    ExtractionError::Write {
        path: path.to_path_buf(),
        source,
    }
}

fn read_existing_error(path: &Path, source: io::Error) -> ExtractionError {
    ExtractionError::ReadExisting {
        path: path.to_path_buf(),
        source,
    }
}
