// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;
use std::path::Path;
use std::process::ExitCode;

use anyhow::Result;
use p4kx_export::{Language, RecordExportOptions, RecordExportSummary, RecordFailure, RecordForm};
use p4kx_extraction::{
    Archive, ArchiveWriteProtection, DataForge, Filter, LocalizationSet, Record,
    RecordExportReport, RecordLocalization, TextResolver,
};
use serde::Serialize;
use thiserror::Error;

use crate::cli::{DcbExportArgs, RecordFormat};
use crate::extract::{cancel_on_interrupt, policy, prepare_target_protected};
use crate::forge_input::ForgeInput;
use crate::json;
use crate::loc::{LanguageNotInArchive, print_notes};
use crate::renamed_report::{RenamedJson, renamed_json};

#[path = "dcb_export_publication.rs"]
mod publication;
#[path = "dcb_export_report.rs"]
mod report;
#[path = "dcb_export_tags.rs"]
mod tags;
pub(crate) use report::print_report;
use report::{Outcome, reason};
use tags::RecordExportTags;

/// `--combined` with another format than Record XML.
#[derive(Debug, Error)]
#[error("--combined writes Record XML; use --format ndjson for one JSON file")]
pub(crate) struct CombinedNeedsXml;

/// A Filter that matches no Record of the DataForge.
#[derive(Debug, Error)]
#[error("no Record matches {0}")]
pub(crate) struct NoRecordMatch(String);

/// A combination of `--flat`, `--format`, `--csv-raw`, `--combined` and `--tags` that has no
/// meaning.
#[derive(Debug, Error)]
#[error("{0}")]
pub(crate) struct FlatConflict(pub(crate) &'static str);

/// `--lang` on a DataForge file without the `--archive` that holds the texts.
#[derive(Debug, Error)]
#[error("--lang on a DataForge file needs --archive <Data.p4k> for the localisation texts")]
pub(crate) struct TextsNeedArchive;

/// `--archive` with an Archive as input, which supplies its own texts.
#[derive(Debug, Error)]
#[error("--archive supplies the texts of a DataForge file; the input is an Archive already")]
pub(crate) struct ArchiveOptionRefused;

/// Returns the form of the export `args` ask for.
const fn record_form(args: &DcbExportArgs) -> Result<RecordForm, FlatConflict> {
    if !args.flat {
        if args.csv_raw {
            return Err(FlatConflict("--csv-raw needs --flat --format csv"));
        }
        return match args.format {
            None | Some(RecordFormat::Xml) => Ok(RecordForm::Xml),
            Some(RecordFormat::Json) => Ok(RecordForm::Json),
            Some(RecordFormat::Ndjson) => Ok(RecordForm::Ndjson),
            Some(RecordFormat::Csv) => Err(FlatConflict("--format csv needs --flat")),
            Some(RecordFormat::CsvStrict) => Err(FlatConflict("--format csv-strict needs --flat")),
        };
    }
    if args.combined {
        return Err(FlatConflict("--flat cannot be combined with --combined"));
    }
    if args.tags {
        return Err(FlatConflict("--flat cannot be combined with --tags"));
    }
    match args.format {
        None | Some(RecordFormat::Csv) if args.csv_raw => Ok(RecordForm::FlatCsvRaw),
        None | Some(RecordFormat::Csv) => Ok(RecordForm::FlatCsv),
        Some(RecordFormat::CsvStrict | RecordFormat::Ndjson) if args.csv_raw => {
            Err(FlatConflict("--csv-raw needs --flat --format csv"))
        }
        Some(RecordFormat::CsvStrict) => Ok(RecordForm::FlatCsvStrict),
        Some(RecordFormat::Ndjson) => Ok(RecordForm::FlatNdjson),
        Some(RecordFormat::Xml | RecordFormat::Json) => Err(FlatConflict(
            "--flat writes --format csv, csv-strict or ndjson",
        )),
    }
}

#[derive(Serialize)]
struct ReportJson<'a> {
    archive: String,
    entry: &'a str,
    format: &'static str,
    target: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    file: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    language: Option<&'static str>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    tags: bool,
    exported: usize,
    skipped: &'a [String],
    failed: Vec<FailureJson<'a>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    renamed: Vec<RenamedJson<'a>>,
    cancelled: usize,
    error_values: usize,
    records_with_errors: usize,
    error: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    origin_receipts: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    origin_failures: Vec<String>,
}

#[derive(Serialize)]
pub(crate) struct FailureJson<'a> {
    guid: String,
    path: &'a str,
    error: String,
}

/// Writes the selected Records of the Archive's DataForge in the chosen format into the target
/// folder and prints the report.
pub(crate) fn run(args: &DcbExportArgs, as_json: bool) -> Result<ExitCode> {
    let form = record_form(args)?;
    if args.combined && form != RecordForm::Xml {
        return Err(CombinedNeedsXml.into());
    }
    let input = ForgeInput::open(&args.archive)?;
    let forge = input.entry().open()?;
    let records = select_records(&forge, args.filter.as_deref())?;
    let tags = args
        .tags
        .then(|| RecordExportTags::read(&input, &forge, form))
        .transpose()?;
    let job = Job {
        args,
        as_json,
        form,
        input: &input,
        forge: &forge,
        records: &records,
        tags: tags.as_ref(),
    };
    let Some(language) = args.lang else {
        return job.write(None, None);
    };
    let given = given_text_archive(&input, args.text_archive.as_deref())?;
    let text_archive = given
        .as_ref()
        .or_else(|| input.archive())
        .ok_or(TextsNeedArchive)?;
    let set = LocalizationSet::read(text_archive, &[language])?;
    let entry = set
        .entry(language)
        .ok_or(LanguageNotInArchive { language })?;
    let localization = RecordLocalization::try_new(entry)?;
    if let Some(note) = localization.note() {
        print_notes(&[note]);
    }
    job.write(Some(&localization), given.as_ref().map(Archive::path))
}

/// Opens the Archive `--archive` names, which supplies the localisation texts of a DataForge
/// file; an input that is an Archive supplies them itself.
fn given_text_archive(input: &ForgeInput, given: Option<&Path>) -> Result<Option<Archive>> {
    let Some(path) = given else {
        return Ok(None);
    };
    if input.archive().is_some() {
        return Err(ArchiveOptionRefused.into());
    }
    Ok(Some(crate::key::open(path)?))
}

/// The selected Records of an export and what they are written with.
struct Job<'a, 'f> {
    args: &'a DcbExportArgs,
    as_json: bool,
    form: RecordForm,
    input: &'a ForgeInput,
    forge: &'a DataForge<'f>,
    records: &'a [Record<'f>],
    tags: Option<&'a RecordExportTags>,
}

impl Job<'_, '_> {
    /// Writes the Records with their selected annotations and prints the report.
    fn write(
        &self,
        localization: Option<&RecordLocalization<'_>>,
        additional_source: Option<&Path>,
    ) -> Result<ExitCode> {
        let Self {
            args, input, tags, ..
        } = *self;
        let cancel = cancel_on_interrupt()?;
        let protection = match additional_source {
            Some(source) => {
                ArchiveWriteProtection::for_sources(&[input.path(), source], cancel.flag())?
            }
            None => ArchiveWriteProtection::new(input.path(), cancel.flag())?,
        };
        let target =
            prepare_target_protected(args.out.as_deref(), input.archive(), &protection, &cancel)?;
        let options = RecordExportOptions {
            protection: Some(&protection),
            policy: policy(args.conflict),
            cancel: Some(&cancel),
            progress: None,
            texts: localization.map(|source| source as &dyn TextResolver),
            tags: tags.map(RecordExportTags::resolver),
        };
        let combined = args.combined.then(|| combined_name(input.entry().path()));
        let exported = self.publish(&options, &target, combined.as_deref(), localization)?;
        let report = exported.outcome();
        if let Err(error) = self.print_outcome(&report, &target, combined.as_deref())
            && error.kind() != io::ErrorKind::BrokenPipe
        {
            return Err(error.into());
        }
        Ok(report.exit_code())
    }

    fn print_outcome(
        &self,
        report: &Outcome<'_>,
        target: &Path,
        combined: Option<&str>,
    ) -> io::Result<()> {
        let Self {
            args,
            form,
            input,
            tags,
            ..
        } = *self;
        if self.as_json {
            json::print(&ReportJson {
                archive: json::path(input.path()),
                entry: input.entry().path(),
                format: form.name(),
                target: json::path(target),
                file: combined,
                language: args.lang.map(Language::code),
                tags: tags.is_some(),
                exported: report.written,
                skipped: report.skipped,
                failed: report.failures.iter().map(failure_json).collect(),
                renamed: report.renamed.iter().map(renamed_json).collect(),
                cancelled: report.cancelled,
                error_values: report.error_values,
                records_with_errors: report.records_with_errors,
                error: report.error.map(reason),
                origin_receipts: report
                    .origin_receipts
                    .iter()
                    .map(|path| json::path(path))
                    .collect(),
                origin_failures: report
                    .origin_failures
                    .iter()
                    .map(|error| reason(error))
                    .collect(),
            })
        } else {
            let into = combined.map(|name| target.join(name));
            print_report(
                report,
                into.as_deref().unwrap_or(target),
                args.conflict,
                form,
            )
        }
    }
}

enum ExportedRecords {
    Files(RecordExportReport),
    Summary(RecordExportSummary),
}

impl ExportedRecords {
    fn outcome(&self) -> Outcome<'_> {
        match self {
            Self::Files(report) => report.into(),
            Self::Summary(summary) => summary.into(),
        }
    }
}

/// Returns every Record of `forge`, or those whose record path `filter` matches.
fn select_records<'f>(forge: &'f DataForge<'_>, filter: Option<&str>) -> Result<Vec<Record<'f>>> {
    let Some(text) = filter else {
        return Ok(forge.records().collect());
    };
    let filter = Filter::parse(text)?;
    let selected: Vec<_> = forge
        .records()
        .filter(|record| record.record_path().is_ok_and(|path| filter.matches(path)))
        .collect();
    if selected.is_empty() {
        return Err(NoRecordMatch(text.to_owned()).into());
    }
    Ok(selected)
}

/// Returns the name of the combined file: the name of the DataForge Entry with `.xml` for its
/// extension, e.g. `Game2.xml`.
fn combined_name(entry: &str) -> String {
    let name = entry.rsplit(['\\', '/']).next().unwrap_or(entry);
    let stem = name.rsplit_once('.').map_or(name, |(stem, _)| stem);
    format!("{stem}.xml")
}

pub(crate) fn failure_json(failure: &RecordFailure) -> FailureJson<'_> {
    FailureJson {
        guid: failure.guid().to_string(),
        path: failure.record_path(),
        error: reason(failure.error()),
    }
}
