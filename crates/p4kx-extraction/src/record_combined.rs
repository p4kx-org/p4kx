// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use p4kx_dataforge::{DataForge, Record};
use rayon::prelude::*;

use crate::counting::holds;
use crate::output_creation::create_temporary_protected;
use crate::output_file::StagedOutput;
use crate::record_format::xml_receipt::XmlAnnotationSources;
use crate::write::{read_existing_error, write_error};
use crate::{
    ConflictPolicy, DataForgeEntry, DataForgeOrigin, ExtractionError, ObservedTagTree,
    RecordExportError, RecordExportOptions, RecordExportReport, RecordExportSources, RecordFailure,
    RecordLocalization, SourceEvidenceError,
};

#[path = "record_combined_document.rs"]
mod document;
#[path = "record_combined_origin.rs"]
mod origin;

use document::Document;

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
    export(
        forge,
        records,
        file,
        options,
        None,
        XmlAnnotationSources::default(),
        false,
    )
}

/// Writes combined Record XML and publishes a companion with original field origins.
/// Annotations require a separate source binding and use ordinary publication.
///
/// # Errors
/// Returns an output error or an error when `forge` is not opened over `input`.
pub fn export_combined_with_origins(
    input: &DataForgeEntry,
    forge: &DataForge<'_>,
    records: &[Record<'_>],
    file: &Path,
    options: &RecordExportOptions<'_>,
) -> Result<RecordExportReport, ExtractionError> {
    let original = if options.annotations().is_active() {
        None
    } else {
        Some(original_of(input, forge, file)?)
    };
    export(
        forge,
        records,
        file,
        options,
        original,
        XmlAnnotationSources::default(),
        false,
    )
}

/// Writes localized combined Record XML and its field-origin companion.
///
/// # Errors
/// Returns an output error for foreign input or independently supplied annotations.
pub fn export_localized_combined(
    input: &DataForgeEntry,
    forge: &DataForge<'_>,
    records: &[Record<'_>],
    file: &Path,
    options: &RecordExportOptions<'_>,
    localization: &RecordLocalization<'_>,
) -> Result<RecordExportReport, ExtractionError> {
    export_annotated_combined(
        input,
        forge,
        records,
        file,
        options,
        Some(localization),
        None,
    )
}

/// Writes combined Record XML with observed annotations and its field-origin companion.
///
/// # Errors
/// Returns an output error for foreign input or independently supplied annotations.
pub fn export_annotated_combined(
    input: &DataForgeEntry,
    forge: &DataForge<'_>,
    records: &[Record<'_>],
    file: &Path,
    options: &RecordExportOptions<'_>,
    localization: Option<&RecordLocalization<'_>>,
    tags: Option<&ObservedTagTree>,
) -> Result<RecordExportReport, ExtractionError> {
    export_annotated(
        RecordExportSources::new(input, localization, tags),
        forge,
        records,
        file,
        options,
        false,
    )
}

/// Writes annotated combined Record XML and retains the committed definition indices.
///
/// # Errors
/// Returns an output error for foreign input or independently supplied annotations.
pub fn export_annotated_combined_with_receipt(
    sources: RecordExportSources<'_, '_>,
    forge: &DataForge<'_>,
    records: &[Record<'_>],
    file: &Path,
    options: &RecordExportOptions<'_>,
) -> Result<RecordExportReport, ExtractionError> {
    export_annotated(sources, forge, records, file, options, true)
}

fn export_annotated(
    sources: RecordExportSources<'_, '_>,
    forge: &DataForge<'_>,
    records: &[Record<'_>],
    file: &Path,
    options: &RecordExportOptions<'_>,
    retain_receipt: bool,
) -> Result<RecordExportReport, ExtractionError> {
    let RecordExportSources {
        input,
        localization,
        tags,
    } = sources;
    if options.annotations().is_active() {
        return Err(write_error(
            file.to_path_buf(),
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "Record annotation sources were not supplied",
            ),
        ));
    }
    let original = original_of(input, forge, file)?;
    let sources = XmlAnnotationSources { localization, tags };
    sources
        .validate(forge)
        .map_err(|source| write_error(file.to_path_buf(), source))?;
    export(
        forge,
        records,
        file,
        options,
        Some(original),
        sources,
        retain_receipt,
    )
}

fn original_of<'input>(
    input: &'input DataForgeEntry,
    forge: &DataForge<'_>,
    file: &Path,
) -> Result<&'input DataForgeOrigin, ExtractionError> {
    input.origin_of(forge).ok_or_else(|| {
        write_error(
            file.to_path_buf(),
            io::Error::new(
                io::ErrorKind::InvalidInput,
                SourceEvidenceError::ForeignEvidence,
            ),
        )
    })
}

fn export(
    forge: &DataForge<'_>,
    records: &[Record<'_>],
    file: &Path,
    options: &RecordExportOptions<'_>,
    original: Option<&DataForgeOrigin>,
    sources: XmlAnnotationSources<'_, '_>,
    retain_receipt: bool,
) -> Result<RecordExportReport, ExtractionError> {
    let mut report = RecordExportReport {
        written_record_indices: retain_receipt.then(Vec::new),
        ..RecordExportReport::default()
    };
    let mut selected = records.len();
    match (Job {
        forge,
        file,
        options,
        original,
    })
    .write(records, sources, &mut report, &mut selected)
    {
        Ok(()) => Ok(report),
        Err(error) if crate::guard_cancellation::extraction_cancelled(&error) => {
            cancel_report(&mut report, selected, 0);
            Ok(report)
        }
        Err(error) => Err(error),
    }
}

struct Job<'a, 'source> {
    forge: &'a DataForge<'source>,
    file: &'a Path,
    options: &'a RecordExportOptions<'a>,
    original: Option<&'a DataForgeOrigin>,
}

impl Job<'_, '_> {
    fn write(
        &self,
        records: &[Record<'_>],
        sources: XmlAnnotationSources<'_, '_>,
        report: &mut RecordExportReport,
        selected: &mut usize,
    ) -> Result<(), ExtractionError> {
        let (file, options) = (self.file, self.options);
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
        let order = order(self.forge, records, report);
        let foreign = report.failures.len();
        *selected = order.len().saturating_add(foreign);
        let folder = match file.parent() {
            Some(parent) if !parent.as_os_str().is_empty() => parent.to_path_buf(),
            _ => PathBuf::from("."),
        };
        let (temporary, part, ownership) =
            create_temporary_protected(&folder, options.protection, options.cancel)?;
        let streamed = Document::new(part, file, self.original).and_then(|document| {
            document.stream(self.forge, records, &order, options, sources, report)
        });
        let staged = StagedOutput::new(
            &temporary,
            file,
            &ownership,
            options.protection,
            options.cancel,
        );
        let placed = streamed.and_then(|streamed| {
            let Some(streamed) = streamed else {
                cancel_report(report, order.len(), foreign);
                return Ok(false);
            };
            if existing
                && options.policy == ConflictPolicy::IfChanged
                && holds(file, streamed.len, streamed.crc, &mut Vec::new())
                    .map_err(|source| read_existing_error(file, source))?
            {
                origin::kept(name, report);
                return Ok(false);
            }
            origin::place(&staged, streamed.origin, options, report, name)
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
}

fn cancel_report(report: &mut RecordExportReport, selected: usize, foreign: usize) {
    let failed = report.failures.len().saturating_sub(foreign);
    *report = RecordExportReport {
        cancelled: selected.saturating_sub(failed),
        failures: std::mem::take(&mut report.failures),
        written_record_indices: report.written_record_indices.as_ref().map(|_| Vec::new()),
        ..RecordExportReport::default()
    };
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
