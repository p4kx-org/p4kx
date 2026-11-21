// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::{self, File};
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};

use p4kx_dataforge::{
    Annotations, DataForge, RECORD_XML_NAMESPACE, Record, RecordXml, TreeBuilder,
};
use rayon::prelude::*;

use crate::counting::{Counting, holds};
use crate::output_creation::create_temporary_protected;
use crate::write::{read_existing_error, write_error};
use crate::{
    CancelToken, ConflictPolicy, ExtractionError, RecordExportError, RecordExportOptions,
    RecordExportReport, RecordFailure,
};

const BATCH_LEN: usize = 1024;
const WRITE_BUFFER_LEN: usize = 256 << 10;
const HEAD: &[u8] = b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<p4kx:records xmlns:p4kx=\"";

/// Writes `records` of `forge` as one Record XML document, the file `file`, in record-path order.
///
/// The root element `p4kx:records` declares the `p4kx` namespace once and holds one Record
/// element per Record in Record order, as [`DataForge::path_order`] gives it; a
/// Record selected twice is written once. Records are built in parallel and streamed to the file
/// in that order, so the same Records always give the same bytes and no more than a batch of
/// Records is held at a time. An existing `file` is handled by the [`ConflictPolicy`] of
/// `options`, where [`ConflictPolicy::IfChanged`] compares size and CRC-32C with the new
/// document; a kept file is reported as skipped by its file name. A Record whose tree cannot be
/// built, or that belongs to another DataForge than `forge`, is reported as failed and left out.
/// The document goes to a temporary file next to `file` first, so a cancelled or failed run
/// leaves no partial file and keeps an existing one; once cancelled, every Record not failed is
/// reported as cancelled.
///
/// # Errors
///
/// Returns [`ExtractionError::Write`] or [`ExtractionError::ReadExisting`] when the file cannot
/// be written, a folder or link is in its way, or the existing file cannot be compared.
pub fn export_combined(
    forge: &DataForge<'_>,
    records: &[Record<'_>],
    file: &Path,
    options: &RecordExportOptions<'_>,
) -> Result<RecordExportReport, ExtractionError> {
    let mut report = RecordExportReport::default();
    let mut selected = records.len();
    match write_combined(forge, records, file, options, &mut report, &mut selected) {
        Ok(()) => Ok(report),
        Err(error) if crate::guard_cancellation::extraction_cancelled(&error) => {
            Ok(RecordExportReport {
                cancelled: selected.saturating_sub(report.failures.len()),
                failures: report.failures,
                ..RecordExportReport::default()
            })
        }
        Err(error) => Err(error),
    }
}

fn write_combined(
    forge: &DataForge<'_>,
    records: &[Record<'_>],
    file: &Path,
    options: &RecordExportOptions<'_>,
    report: &mut RecordExportReport,
    selected: &mut usize,
) -> Result<(), ExtractionError> {
    crate::check_protected_output(file, options.protection, options.cancel)?;
    let name = file
        .file_name()
        .ok_or_else(|| write_error(file.to_path_buf(), io::ErrorKind::InvalidInput.into()))?
        .to_string_lossy()
        .into_owned();
    let existing = existing_file(file)?;
    if existing && options.policy == ConflictPolicy::Skip {
        report.skipped.push(name);
        return Ok(());
    }
    let order = order(forge, records, report);
    let foreign = report.failures.len();
    *selected = order.len().saturating_add(foreign);
    let folder = match file.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.to_path_buf(),
        _ => PathBuf::from("."),
    };
    let (temporary, part, ownership) =
        create_temporary_protected(&folder, options.protection, options.cancel)?;
    let streamed = Document::new(part, file)
        .and_then(|document| document.stream(records, &order, options, report));
    let placed = streamed.and_then(|streamed| {
        let Some((len, crc)) = streamed else {
            let failed = report.failures.len().saturating_sub(foreign);
            *report = RecordExportReport {
                cancelled: order.len().saturating_sub(failed),
                failures: report.failures.split_off(0),
                ..RecordExportReport::default()
            };
            return Ok(false);
        };
        if existing
            && options.policy == ConflictPolicy::IfChanged
            && holds(file, len, crc, &mut Vec::new())
                .map_err(|source| read_existing_error(file, source))?
        {
            report.skipped.push(name);
            report.written = 0;
            return Ok(false);
        }
        crate::check_protected_output(file, options.protection, options.cancel)?;
        ownership.ensure_writable(&temporary, crate::output_file::cancel_flag(options.cancel))?;
        fs::rename(&temporary, file)
            .map(|()| true)
            .map_err(|source| write_error(file.to_path_buf(), source))
    });
    if !matches!(placed, Ok(true)) {
        return crate::output_cleanup::extraction(
            placed.map(|_| ()),
            &ownership,
            &temporary,
            options.cancel,
        );
    }
    placed.map(|_| ())
}

/// Returns whether a file is at `file`; a folder or link there is an error.
fn existing_file(file: &Path) -> Result<bool, ExtractionError> {
    match fs::symlink_metadata(file) {
        Ok(metadata) if metadata.is_file() => Ok(true),
        Ok(_) => Err(write_error(
            file.to_path_buf(),
            io::Error::new(
                io::ErrorKind::AlreadyExists,
                "a folder or link is in the way",
            ),
        )),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(source) => Err(read_existing_error(file, source)),
    }
}

/// Returns the positions in `records` of the Records to write in Record order without repeats,
/// those without a readable record path last; Records of another DataForge are reported as
/// failed.
fn order(
    forge: &DataForge<'_>,
    records: &[Record<'_>],
    report: &mut RecordExportReport,
) -> Vec<usize> {
    let mut rank = vec![u32::MAX; forge.record_count()];
    for (position, index) in (0_u32..).zip(forge.path_order()) {
        if let Some(slot) = rank.get_mut(*index as usize) {
            *slot = position;
        }
    }
    let mut order = Vec::with_capacity(records.len());
    for (position, record) in records.iter().enumerate() {
        if record.is_from(forge) {
            order.push(position);
        } else {
            report
                .failures
                .push(failure(record, RecordExportError::OtherDataForge));
        }
    }
    let key = |position: &usize| {
        records.get(*position).map(|record| {
            let index = record.index();
            (rank.get(index as usize).copied().unwrap_or(u32::MAX), index)
        })
    };
    order.par_sort_unstable_by_key(key);
    order.dedup_by_key(|position| records.get(*position).map(Record::index));
    order
}

fn failure(record: &Record<'_>, error: RecordExportError) -> RecordFailure {
    RecordFailure {
        guid: record.guid(),
        record_path: record.record_path().unwrap_or_default().to_owned(),
        error,
    }
}

/// How one Record of a batch was built.
enum Built {
    Xml { bytes: Vec<u8>, error_values: usize },
    Failed(RecordFailure),
    Cancelled,
}

#[derive(Default)]
struct Scratch<'f> {
    trees: TreeBuilder<'f>,
    xml: RecordXml,
}

fn build<'f>(
    record: &Record<'f>,
    scratch: &mut Scratch<'f>,
    cancel: Option<&CancelToken>,
    annotations: Annotations<'_>,
    file: &Path,
) -> Built {
    if cancel.is_some_and(CancelToken::is_cancelled) {
        return Built::Cancelled;
    }
    let tree = match scratch.trees.build(record) {
        Ok(tree) => tree,
        Err(error) => return Built::Failed(failure(record, error.into())),
    };
    let mut bytes = Vec::new();
    match scratch
        .xml
        .write_wrapped_element_annotated(&tree, annotations, 1, &mut bytes)
    {
        Ok(()) => Built::Xml {
            bytes,
            error_values: tree.error_count(),
        },
        Err(source) => Built::Failed(failure(
            record,
            write_error(file.to_path_buf(), source).into(),
        )),
    }
}

/// The combined document while it is written: the temporary file, with the size and CRC-32C of
/// what it holds.
struct Document<'p> {
    out: Counting<BufWriter<File>>,
    file: &'p Path,
    has_records: bool,
}

impl<'p> Document<'p> {
    fn new(part: File, file: &'p Path) -> Result<Self, ExtractionError> {
        let mut document = Self {
            out: Counting::new(BufWriter::with_capacity(WRITE_BUFFER_LEN, part)),
            file,
            has_records: false,
        };
        document.put(HEAD)?;
        document.put(RECORD_XML_NAMESPACE.as_bytes())?;
        document.put(b"\"")?;
        Ok(document)
    }

    /// Writes the Records at `order` batch by batch, building the next batch while the last one
    /// is written; returns the finished document, or `None` once cancelled.
    fn stream(
        mut self,
        records: &[Record<'_>],
        order: &[usize],
        options: &RecordExportOptions<'_>,
        report: &mut RecordExportReport,
    ) -> Result<Option<(u64, u32)>, ExtractionError> {
        let file = self.file;
        let (cancel, progress, annotations) =
            (options.cancel, options.progress, options.annotations());
        let mut pending = Vec::new();
        for batch in order.chunks(BATCH_LEN) {
            let (written, next) = rayon::join(
                || self.put_batch(std::mem::take(&mut pending), report),
                || {
                    batch
                        .par_iter()
                        .filter_map(|position| records.get(*position))
                        .map_init(Scratch::default, |scratch, record| {
                            let built = build(record, scratch, cancel, annotations, file);
                            if let Some(progress) = progress {
                                progress();
                            }
                            built
                        })
                        .collect::<Vec<_>>()
                },
            );
            if !written? {
                return Ok(None);
            }
            pending = next;
        }
        if !self.put_batch(pending, report)? {
            return Ok(None);
        }
        self.put(if self.has_records {
            b"</p4kx:records>\n".as_slice()
        } else {
            b"/>\n".as_slice()
        })?;
        self.out
            .flush()
            .map_err(|source| write_error(self.file.to_path_buf(), source))?;
        Ok(Some((self.out.len(), self.out.crc())))
    }

    /// Writes one built batch; returns `false` when a Record of it was cancelled.
    fn put_batch(
        &mut self,
        batch: Vec<Built>,
        report: &mut RecordExportReport,
    ) -> Result<bool, ExtractionError> {
        for built in batch {
            match built {
                Built::Xml {
                    bytes,
                    error_values,
                } => {
                    if !self.has_records {
                        self.has_records = true;
                        self.put(b">\n")?;
                    }
                    self.put(&bytes)?;
                    report.written = report.written.saturating_add(1);
                    report.error_values = report.error_values.saturating_add(error_values);
                    report.records_with_errors = report
                        .records_with_errors
                        .saturating_add(usize::from(error_values > 0));
                }
                Built::Failed(failure) => report.failures.push(failure),
                Built::Cancelled => return Ok(false),
            }
        }
        Ok(true)
    }

    fn put(&mut self, bytes: &[u8]) -> Result<(), ExtractionError> {
        self.out
            .write_all(bytes)
            .map_err(|source| write_error(self.file.to_path_buf(), source))
    }
}
