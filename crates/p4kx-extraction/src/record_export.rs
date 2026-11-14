// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::path::Path;

use p4kx_dataforge::{Annotations, DataForge, Record, RecordXml, TreeBuilder};
use rayon::prelude::*;

use crate::dependent::split_dependent;
use crate::entry_path::relative_path;
use crate::folder_cache::FolderCache;
use crate::output_creation::create_temporary_protected;
use crate::portable_name::{check_path_length, fold};
use crate::record_files::RecordFiles;
use crate::record_format::RecordFileFormat;
use crate::record_plan::RecordFilePlan;
use crate::write::write_error;
use crate::{
    ConflictPolicy, ExtractionError, RecordExportError, RecordExportOptions, RecordExportReport,
    RecordFailure, RecordRenamed,
};

/// Writes `records` of `forge` as Record XML, one file per Record, below the existing folder
/// `target`; [`export_record_files`] with [`RecordXml`].
#[must_use]
pub fn export_records(
    forge: &DataForge<'_>,
    records: &[Record<'_>],
    target: &Path,
    options: &RecordExportOptions<'_>,
) -> RecordExportReport {
    export_record_files::<RecordXml>(forge, records, target, options)
}

/// Writes `records` of `forge` in the format `F`, one file per Record, below the existing folder
/// `target`.
///
/// A Record alone at its record path is written to `<record path><extension>`; Records that
/// share one go into the folder `<record path>/` as `<name><extension>`, with the GUID added when
/// names collide (ADR-0008). Files that already exist are handled by the [`ConflictPolicy`] of
/// `options`, where [`ConflictPolicy::IfChanged`] compares size and CRC-32C with the new file.
/// Records are built and written in parallel with the same files and report as a sequential run
/// in the order of `records`. Record paths that could leave `target` are rejected and existing
/// links below `target` are never followed; a failed or cancelled Record leaves no partial file.
/// A Record of another DataForge than `forge` fails with [`RecordExportError::OtherDataForge`].
#[must_use]
pub fn export_record_files<F: RecordFileFormat>(
    forge: &DataForge<'_>,
    records: &[Record<'_>],
    target: &Path,
    options: &RecordExportOptions<'_>,
) -> RecordExportReport {
    let plan = RecordFiles::plan(forge, F::EXTENSION);
    write_with_options::<F>(forge, &plan, records, target, options)
}

/// Writes `records` of `forge` like [`export_record_files`], with the files of `plan`.
///
/// `plan` must have been made from `forge`; a plan for a DataForge with another number of
/// Records or another size is not used, and the files are planned again.
#[must_use]
pub fn export_planned_record_files<F: RecordFileFormat>(
    forge: &DataForge<'_>,
    plan: &RecordFilePlan<F>,
    records: &[Record<'_>],
    target: &Path,
    options: &RecordExportOptions<'_>,
) -> RecordExportReport {
    if plan.fits(forge) {
        write_with_options::<F>(forge, plan.files(), records, target, options)
    } else {
        export_record_files::<F>(forge, records, target, options)
    }
}

fn write_with_options<F: RecordFileFormat>(
    forge: &DataForge<'_>,
    plan: &RecordFiles,
    records: &[Record<'_>],
    target: &Path,
    options: &RecordExportOptions<'_>,
) -> RecordExportReport {
    let (cancel, progress) = (options.cancel, options.progress);
    let run = RecordRun {
        protection: options.protection,
        cancel: options.cancel,
        policy: options.policy,
        annotations: options.annotations(),
        stop: &|| cancel.is_some_and(crate::CancelToken::is_cancelled),
        ended: &|| {
            if let Some(progress) = progress {
                progress();
            }
        },
    };
    write_records::<F>(forge, plan, records, target, &run)
}

/// How a run writes Record files: the Conflict policy, whether to stop before the next Record,
/// and what to call once a Record ended.
pub(crate) struct RecordRun<'a> {
    pub(crate) protection: Option<&'a crate::ArchiveWriteProtection>,
    pub(crate) cancel: Option<&'a crate::CancelToken>,
    pub(crate) policy: ConflictPolicy,
    pub(crate) annotations: Annotations<'a>,
    pub(crate) stop: &'a (dyn Fn() -> bool + Sync),
    pub(crate) ended: &'a (dyn Fn() + Sync),
}

/// Writes `records` of `forge` in the format `F` to the files of `plan` like
/// [`export_record_files`], under the control of `run`.
pub(crate) fn write_records<'f, F: RecordFileFormat>(
    forge: &DataForge<'_>,
    plan: &RecordFiles,
    records: &[Record<'f>],
    target: &Path,
    run: &RecordRun<'_>,
) -> RecordExportReport {
    if let Err(refused) = crate::check_protected_output(target, run.protection, run.cancel) {
        if refused.is_cancelled() {
            return RecordExportReport {
                cancelled: records.len(),
                ..RecordExportReport::default()
            };
        }
        let outcomes = (0..records.len())
            .map(|index| {
                let error = ExtractionError::Installation(refused.clone());
                (index, RecordOutcome::Failed(error.into()))
            })
            .collect();
        return summarise(records, outcomes);
    }
    let files = select_files(forge, plan, records);
    let (independent, dependent) = split_dependent(files.iter().map(|file| match file {
        Selected::File(file) => *file,
        Selected::Taken(_) | Selected::Unsafe => "",
    }));
    let absolute = std::path::absolute(target).unwrap_or_else(|_| target.to_path_buf());
    let writer = Writer {
        protection: run.protection,
        cancel: run.cancel,
        target: &absolute,
        policy: run.policy,
        annotations: run.annotations,
        stop: run.stop,
        folders: FolderCache::default(),
    };
    let ended = run.ended;
    let run = |scratch: &mut Scratch<'f, F>, index: usize| {
        let outcome = match (records.get(index), files.get(index)) {
            (Some(record), _) if !record.is_from(forge) => {
                RecordOutcome::Failed(RecordExportError::OtherDataForge)
            }
            (Some(record), Some(Selected::File(file))) => writer.write(record, file, scratch),
            (_, Some(Selected::Taken(file))) => {
                RecordOutcome::Failed(RecordExportError::FileTaken {
                    path: (*file).to_owned(),
                })
            }
            _ => RecordOutcome::Failed(ExtractionError::UnsafePath.into()),
        };
        ended();
        (index, outcome)
    };
    let mut outcomes: Vec<_> = independent
        .into_par_iter()
        .map_init(Scratch::default, run)
        .collect();
    let mut scratch = Scratch::default();
    outcomes.extend(dependent.into_iter().map(|index| run(&mut scratch, index)));
    outcomes.sort_unstable_by_key(|(index, _)| *index);
    let mut report = summarise(records, outcomes);
    report.renamed = renamed(forge, plan, records);
    report
}

/// Returns a note for every selected Record of `forge` whose file does not follow its name or
/// record path.
fn renamed(
    forge: &DataForge<'_>,
    plan: &RecordFiles,
    records: &[Record<'_>],
) -> Vec<RecordRenamed> {
    records
        .iter()
        .filter(|record| record.is_from(forge))
        .flat_map(|record| {
            plan.renames(record).map(|cause| {
                let path = record.record_path().unwrap_or_default();
                RecordRenamed::new(record.guid(), path.to_owned(), cause)
            })
        })
        .collect()
}

/// Returns the planned file of every selected Record; a file an earlier Record of the selection
/// takes, ignoring case, is taken, and a Record of another DataForge gets none.
fn select_files<'p>(
    forge: &DataForge<'_>,
    plan: &'p RecordFiles,
    records: &[Record<'_>],
) -> Vec<Selected<'p>> {
    let mut taken = HashSet::new();
    records
        .iter()
        .map(|record| match plan.file(record) {
            _ if !record.is_from(forge) => Selected::Unsafe,
            None => Selected::Unsafe,
            Some(file) if taken.insert(fold(file).collect::<String>()) => Selected::File(file),
            Some(file) => Selected::Taken(file),
        })
        .collect()
}

fn summarise(records: &[Record<'_>], outcomes: Vec<(usize, RecordOutcome)>) -> RecordExportReport {
    let mut report = RecordExportReport::default();
    for (index, outcome) in outcomes {
        let errors = match outcome {
            RecordOutcome::Written { errors } => {
                report.written += 1;
                errors
            }
            RecordOutcome::Kept { file, errors } => {
                report.skipped.push(file);
                errors
            }
            RecordOutcome::Cancelled => {
                report.cancelled += 1;
                0
            }
            RecordOutcome::Failed(error) => {
                let record = records.get(index);
                report.failures.push(RecordFailure {
                    guid: record.map_or_default(Record::guid),
                    record_path: record
                        .and_then(|record| record.record_path().ok())
                        .unwrap_or_default()
                        .to_owned(),
                    error,
                });
                0
            }
        };
        report.error_values += errors;
        report.records_with_errors += usize::from(errors > 0);
    }
    report
}

/// The file a selected Record is written to, if it can be.
#[derive(Clone, Copy)]
enum Selected<'p> {
    File(&'p str),
    Taken(&'p str),
    Unsafe,
}

enum RecordOutcome {
    Written { errors: usize },
    Kept { file: String, errors: usize },
    Cancelled,
    Failed(RecordExportError),
}

impl<F: Default> Default for Scratch<'_, F> {
    fn default() -> Self {
        Self {
            trees: TreeBuilder::default(),
            format: F::default(),
            bytes: Vec::new(),
            read: Vec::new(),
        }
    }
}

struct Scratch<'f, F> {
    trees: TreeBuilder<'f>,
    format: F,
    bytes: Vec<u8>,
    read: Vec<u8>,
}

struct Writer<'a> {
    protection: Option<&'a crate::ArchiveWriteProtection>,
    cancel: Option<&'a crate::CancelToken>,
    target: &'a Path,
    policy: ConflictPolicy,
    annotations: Annotations<'a>,
    stop: &'a (dyn Fn() -> bool + Sync),
    folders: FolderCache,
}

impl Writer<'_> {
    fn write<'f, F: RecordFileFormat>(
        &self,
        record: &Record<'f>,
        file: &str,
        scratch: &mut Scratch<'f, F>,
    ) -> RecordOutcome {
        if (self.stop)() {
            return RecordOutcome::Cancelled;
        }
        match self.try_write(record, file, scratch) {
            Ok(outcome) => outcome,
            Err(RecordExportError::Write(error))
                if crate::guard_cancellation::extraction_cancelled(&error) =>
            {
                RecordOutcome::Cancelled
            }
            Err(error) => RecordOutcome::Failed(error),
        }
    }

    fn try_write<'f, F: RecordFileFormat>(
        &self,
        record: &Record<'f>,
        file: &str,
        scratch: &mut Scratch<'f, F>,
    ) -> Result<RecordOutcome, RecordExportError> {
        let relative = relative_path(file)?;
        check_path_length(&self.target.join(&relative)).map_err(ExtractionError::from)?;
        let name = relative.file_name().ok_or(ExtractionError::UnsafePath)?;
        let tree = scratch.trees.build(record)?;
        let errors = tree.error_count();
        let folder = self.folders.folder_protected(
            self.target,
            relative.parent(),
            self.protection,
            self.cancel,
        )?;
        let destination = folder.join(name);
        scratch.bytes.clear();
        scratch
            .format
            .write_file(&tree, self.annotations, &mut scratch.bytes)
            .map_err(|source| write_error(destination.clone(), source))?;
        if crate::record_existing::keep(
            &destination,
            self.policy,
            &scratch.bytes,
            &mut scratch.read,
        )? {
            return Ok(RecordOutcome::Kept {
                file: file.to_owned(),
                errors,
            });
        }
        let (temporary, mut part, ownership) =
            create_temporary_protected(&folder, self.protection, self.cancel)?;
        let written = part
            .write_all(&scratch.bytes)
            .map_err(|source| write_error(temporary.clone(), source))
            .and_then(|()| {
                drop(part);
                ownership
                    .ensure_writable(&temporary, crate::output_file::cancel_flag(self.cancel))?;
                crate::check_protected_output(&destination, self.protection, self.cancel)?;
                fs::rename(&temporary, &destination)
                    .map_err(|source| write_error(destination.clone(), source))
            });
        if let Err(error) = written {
            return crate::output_cleanup::extraction(
                Err(error),
                &ownership,
                &temporary,
                self.cancel,
            )
            .map_err(Into::into);
        }
        Ok(RecordOutcome::Written { errors })
    }
}
