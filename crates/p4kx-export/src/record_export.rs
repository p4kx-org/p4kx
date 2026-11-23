// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;
use std::sync::OnceLock;

use p4kx_extraction::{
    DataForge, Record, RecordExportOptions, RecordFileFormat, RecordFilePlan, RecordJson,
    RecordXml, export_planned_record_files, export_planned_record_files_with_receipt,
};

use crate::{RecordExportSummary, RecordForm, ndjson};

/// Writes `records` of `forge` in `form` below the existing folder `target` and returns the
/// summary.
///
/// [`RecordForm::Xml`] and [`RecordForm::Json`] write one file per Record under its record path
/// (ADR-0008), in parallel, with the [`ConflictPolicy`](crate::ConflictPolicy) of `options` for
/// every file. [`RecordForm::Ndjson`] writes [`NDJSON_FILE`](crate::NDJSON_FILE) with one compact
/// Record JSON object per line in Record order ([`DataForge::path_order`]);
/// each Record once. The flat forms ([`RecordForm::FlatCsv`], [`RecordForm::FlatCsvRaw`],
/// [`RecordForm::FlatNdjson`]) write one file of one row per leaf value in the same Record order
/// and the same way (`docs/record-flat.md`); with `options.texts` the rows of localisation keys
/// carry their text. The Records of a single-file form are built in parallel batches and streamed into a temporary file
/// that replaces the file only once complete; the Conflict policy applies to that file. The same
/// Records always give the same bytes. A Record whose tree cannot be built, or that belongs to
/// another DataForge, is a failure and every other Record is still written.
#[must_use]
pub fn write_record_export(
    forge: &DataForge<'_>,
    records: &[Record<'_>],
    form: RecordForm,
    target: &Path,
    options: &RecordExportOptions<'_>,
) -> RecordExportSummary {
    write_planned_record_export(
        forge,
        &RecordFilePlans::default(),
        records,
        form,
        target,
        options,
    )
}

/// Writes `records` of `forge` like [`write_record_export`], planning the files of a form with
/// one file per Record only the first time `plans` is used for it.
///
/// `plans` must only ever be used with `forge` or with the same DataForge opened again; a plan
/// for a DataForge with another number of Records or another size is not used, and the files are
/// planned again.
#[must_use]
pub fn write_planned_record_export(
    forge: &DataForge<'_>,
    plans: &RecordFilePlans,
    records: &[Record<'_>],
    form: RecordForm,
    target: &Path,
    options: &RecordExportOptions<'_>,
) -> RecordExportSummary {
    write_planned(forge, plans, records, form, target, options, false)
}

/// Writes Records like [`write_planned_record_export`] and retains the committed definition indices.
#[must_use]
pub fn write_planned_record_export_with_receipt(
    forge: &DataForge<'_>,
    plans: &RecordFilePlans,
    records: &[Record<'_>],
    form: RecordForm,
    target: &Path,
    options: &RecordExportOptions<'_>,
) -> RecordExportSummary {
    write_planned(forge, plans, records, form, target, options, true)
}

fn write_planned(
    forge: &DataForge<'_>,
    plans: &RecordFilePlans,
    records: &[Record<'_>],
    form: RecordForm,
    target: &Path,
    options: &RecordExportOptions<'_>,
    retain_receipt: bool,
) -> RecordExportSummary {
    match form {
        RecordForm::Xml => {
            let plan = plans.xml.get_or_init(|| RecordFilePlan::new(forge));
            write_files(forge, plan, records, form, target, options, retain_receipt)
        }
        RecordForm::Json => {
            let plan = plans.json.get_or_init(|| RecordFilePlan::new(forge));
            write_files(forge, plan, records, form, target, options, retain_receipt)
        }
        RecordForm::Ndjson
        | RecordForm::FlatCsv
        | RecordForm::FlatCsvRaw
        | RecordForm::FlatCsvStrict
        | RecordForm::FlatNdjson => {
            ndjson::write(forge, records, form, target, options, retain_receipt)
        }
    }
}

fn write_files<F: RecordFileFormat>(
    forge: &DataForge<'_>,
    plan: &RecordFilePlan<F>,
    records: &[Record<'_>],
    form: RecordForm,
    target: &Path,
    options: &RecordExportOptions<'_>,
    retain_receipt: bool,
) -> RecordExportSummary {
    let report = if retain_receipt {
        export_planned_record_files_with_receipt(forge, plan, records, target, options)
    } else {
        export_planned_record_files(forge, plan, records, target, options)
    };
    RecordExportSummary::from_report(form, report)
}

/// The files of the Records of one DataForge as Record XML and as Record JSON, each planned on
/// first use by [`write_planned_record_export`] and kept for every later Record export from that
/// DataForge.
#[derive(Debug, Default)]
pub struct RecordFilePlans {
    xml: OnceLock<RecordFilePlan<RecordXml>>,
    json: OnceLock<RecordFilePlan<RecordJson>>,
}
