// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use p4kx_dataforge::{Annotations, DataForge, Record, RecordXml};
use rayon::prelude::*;

use crate::dependent::split_dependent;
use crate::portable_name::fold;
use crate::record_files::RecordFiles;
use crate::record_format::{RecordFileFormat, xml_receipt};
use crate::record_plan::RecordFilePlan;
use crate::{
    ConflictPolicy, DataForgeEntry, ExtractionError, RecordExportError, RecordExportOptions,
    RecordExportProvenance, RecordExportReport, RecordRenamed,
};

#[path = "record_export_accounting.rs"]
mod accounting;
use accounting::summarise;

#[path = "record_json_export.rs"]
mod json_export;
pub use json_export::{
    export_annotated_record_json_files, export_planned_record_json_files_with_origins,
    export_record_json_files_with_origins,
};

#[path = "record_xml_export.rs"]
mod xml_export;
pub use xml_export::{
    export_annotated_record_xml_files, export_localized_record_xml_files,
    export_planned_record_xml_files_with_origins, export_record_xml_files_with_origins,
};

#[path = "record_export_sources.rs"]
mod sources;
pub use sources::RecordExportSources;

#[path = "record_artifact_writer.rs"]
mod writer;
pub(crate) use writer::replace_record;
use writer::{Observation, Scratch, Writer};

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
    write_with_options::<F>(forge, &plan, records, target, options, false, None)
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
    write_planned(forge, plan, records, target, options, false, None)
}

/// Writes the planned Records and retains the definition positions of the committed files.
#[must_use]
pub fn export_planned_record_files_with_receipt<F: RecordFileFormat>(
    forge: &DataForge<'_>,
    plan: &RecordFilePlan<F>,
    records: &[Record<'_>],
    target: &Path,
    options: &RecordExportOptions<'_>,
) -> RecordExportReport {
    write_planned(forge, plan, records, target, options, true, None)
}

fn write_planned<F: RecordFileFormat>(
    forge: &DataForge<'_>,
    plan: &RecordFilePlan<F>,
    records: &[Record<'_>],
    target: &Path,
    options: &RecordExportOptions<'_>,
    retain_receipt: bool,
    observation: Option<Observation<'_, F>>,
) -> RecordExportReport {
    let rebuilt;
    let files = if plan.fits(forge) {
        plan.files()
    } else {
        rebuilt = RecordFiles::plan(forge, F::EXTENSION);
        &rebuilt
    };
    write_with_options::<F>(
        forge,
        files,
        records,
        target,
        options,
        retain_receipt,
        observation,
    )
}

fn write_observed<F: RecordFileFormat>(
    forge: &DataForge<'_>,
    records: &[Record<'_>],
    target: &Path,
    options: &RecordExportOptions<'_>,
    observation: Observation<'_, F>,
    plan: Option<&RecordFilePlan<F>>,
) -> Result<RecordExportReport, ExtractionError> {
    if options.annotations().is_active() {
        return Err(crate::write::write_error(
            target.to_path_buf(),
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Record annotation sources were not supplied",
            ),
        ));
    }
    if let Some(plan) = plan {
        return Ok(write_planned(
            forge,
            plan,
            records,
            target,
            options,
            true,
            Some(observation),
        ));
    }
    let plan = RecordFiles::plan(forge, F::EXTENSION);
    Ok(write_with_options::<F>(
        forge,
        &plan,
        records,
        target,
        options,
        false,
        Some(observation),
    ))
}

fn write_with_options<F: RecordFileFormat>(
    forge: &DataForge<'_>,
    plan: &RecordFiles,
    records: &[Record<'_>],
    target: &Path,
    options: &RecordExportOptions<'_>,
    retain_receipt: bool,
    observation: Option<Observation<'_, F>>,
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
    write_records_inner::<F>(
        forge,
        plan,
        records,
        target,
        &run,
        retain_receipt,
        observation,
    )
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

/// Writes converted Record XML files and their source evidence under the extraction run.
pub(crate) fn write_converted_record_xml(
    input: &DataForgeEntry,
    forge: &DataForge<'_>,
    records: &[Record<'_>],
    target: &Path,
    run: &RecordRun<'_>,
) -> RecordExportReport {
    let plan = RecordFiles::plan(forge, RecordXml::EXTENSION);
    let observation = Some(Observation::new(input, &xml_receipt::render));
    let mut report = write_records_inner(forge, &plan, records, target, run, true, observation);
    let indices = report.take_written_record_indices();
    report.written_provenance = Some(
        RecordExportProvenance::capture_count(input.origin_of(forge), indices, report.written())
            .map(Box::new),
    );
    report
}

fn write_records_inner<'f, F: RecordFileFormat>(
    forge: &DataForge<'_>,
    plan: &RecordFiles,
    records: &[Record<'f>],
    target: &Path,
    run: &RecordRun<'_>,
    retain_receipt: bool,
    observation: Option<Observation<'_, F>>,
) -> RecordExportReport {
    let written_record_indices = retain_receipt.then(Vec::new);
    if let Err(refused) = crate::check_protected_output(target, run.protection, run.cancel) {
        if refused.is_cancelled() {
            return RecordExportReport {
                cancelled: records.len(),
                written_record_indices,
                ..RecordExportReport::default()
            };
        }
        let outcomes = (0..records.len())
            .map(|index| {
                let error = ExtractionError::Installation(refused.clone());
                (index, RecordOutcome::Failed(error.into()))
            })
            .collect();
        return summarise(records, outcomes, written_record_indices);
    }
    let files = select_files(forge, plan, records);
    let (independent, dependent) = split_dependent(files.iter().map(|file| match file {
        Selected::File(file) => *file,
        Selected::Taken(_) | Selected::Unsafe => "",
    }));
    let absolute = std::path::absolute(target).unwrap_or_else(|_| target.to_path_buf());
    let reserved = if observation.is_some() {
        companion_folders(&files)
    } else {
        HashSet::new()
    };
    let writer = Writer::new(run, &absolute, forge, observation, reserved);
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
    let mut report = summarise(records, outcomes, written_record_indices);
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

/// The file a selected Record is written to, if it can be.
#[derive(Clone, Copy)]
enum Selected<'p> {
    File(&'p str),
    Taken(&'p str),
    Unsafe,
}

fn companion_folders(files: &[Selected<'_>]) -> HashSet<String> {
    files
        .iter()
        .filter_map(|file| match file {
            Selected::File(path) => Some(*path),
            _ => None,
        })
        .flat_map(|path| {
            path.match_indices('/')
                .filter_map(move |(end, _)| path.get(..end))
        })
        .filter(|folder| {
            folder.rsplit('/').next().is_some_and(|part| {
                crate::file_receipt::is_companion_name(std::ffi::OsStr::new(part))
            })
        })
        .map(|folder| fold(folder).collect())
        .collect()
}

enum RecordOutcome {
    Written {
        errors: usize,
        receipt: Option<PathBuf>,
        origin_failure: Option<ExtractionError>,
    },
    Kept {
        file: String,
        errors: usize,
    },
    Cancelled,
    Failed(RecordExportError),
}
