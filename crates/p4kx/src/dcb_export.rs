// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::io::{self, Write};
use std::path::Path;
use std::process::ExitCode;

use anyhow::Result;
use p4kx_export::{
    Language, NDJSON_FILE, RecordExportOptions, RecordExportSummary, RecordFailure, RecordForm,
    write_record_export,
};
use p4kx_extraction::{
    Archive, DataForge, Filter, LanguageTexts, LocalizationSet, Record, RecordExportReport,
    RecordRenamed, TagResolver, TagTree, TextResolver, export_combined,
};
use serde::Serialize;
use thiserror::Error;

use crate::cli::{Conflict, DcbExportArgs, RecordFormat};
use crate::exit;
use crate::extract::{cancel_on_interrupt, policy, prepare_target_protected};
use crate::forge_input::ForgeInput;
use crate::json;
use crate::loc::{LanguageNotInArchive, print_notes};
use crate::renamed_report::{RenamedJson, renamed_json, write_renamed};

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
        Some(RecordFormat::Ndjson) if args.csv_raw => {
            Err(FlatConflict("--csv-raw needs --flat --format csv"))
        }
        Some(RecordFormat::Ndjson) => Ok(RecordForm::FlatNdjson),
        Some(RecordFormat::Xml | RecordFormat::Json) => Err(FlatConflict(
            "--flat writes --format csv or --format ndjson",
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
    let tags = args.tags.then(|| TagTree::build(&forge));
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
        return job.write(None);
    };
    let given = given_text_archive(&input, args.text_archive.as_deref())?;
    let text_archive = given
        .as_ref()
        .or_else(|| input.archive())
        .ok_or(TextsNeedArchive)?;
    let set = LocalizationSet::read(text_archive, &[language])?;
    if set.entry(language).is_none() {
        return Err(LanguageNotInArchive { language }.into());
    }
    let texts = set.index()?;
    print_notes(&set.notes(&texts));
    job.write(Some(&LanguageTexts::new(&texts, language)))
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
    tags: Option<&'a TagTree>,
}

impl Job<'_, '_> {
    /// Writes the Records with `texts` resolved and prints the report.
    fn write(&self, texts: Option<&dyn TextResolver>) -> Result<ExitCode> {
        let Self {
            args,
            as_json,
            form,
            input,
            forge,
            records,
            tags,
        } = *self;
        let entry = input.entry();
        let cancel = cancel_on_interrupt()?;
        let protection = p4kx_extraction::ArchiveWriteProtection::new(input.path(), cancel.flag())?;
        let target =
            prepare_target_protected(args.out.as_deref(), input.archive(), &protection, &cancel)?;
        let options = RecordExportOptions {
            protection: Some(&protection),
            policy: policy(args.conflict),
            cancel: Some(&cancel),
            progress: None,
            texts,
            tags: tags.map(|tags| tags as &dyn TagResolver),
        };
        let combined = args.combined.then(|| combined_name(entry.path()));
        let combined_report;
        let summary;
        let report: Outcome<'_> = if let Some(name) = &combined {
            combined_report = export_combined(forge, records, &target.join(name), &options)?;
            (&combined_report).into()
        } else {
            summary = write_record_export(forge, records, form, &target, &options);
            (&summary).into()
        };
        let printed = if as_json {
            json::print(&ReportJson {
                archive: json::path(input.path()),
                entry: entry.path(),
                format: form.name(),
                target: json::path(&target),
                file: combined.as_deref(),
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
            })
        } else {
            let into = combined.map_or_else(|| target.clone(), |name| target.join(name));
            print_report(&report, &into, args.conflict, form)
        };
        if let Err(error) = printed
            && error.kind() != io::ErrorKind::BrokenPipe
        {
            return Err(error.into());
        }
        Ok(report.exit_code())
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

/// What a Record export did, from a per-Record or NDJSON export or a combined file.
pub(crate) struct Outcome<'a> {
    written: usize,
    skipped: &'a [String],
    failures: &'a [RecordFailure],
    renamed: &'a [RecordRenamed],
    cancelled: usize,
    error_values: usize,
    records_with_errors: usize,
    error: Option<&'a dyn Error>,
}

impl Outcome<'_> {
    const fn is_complete(&self) -> bool {
        self.failures.is_empty() && self.cancelled == 0 && self.error.is_none()
    }

    fn exit_code(&self) -> ExitCode {
        ExitCode::from(if self.is_complete() {
            exit::SUCCESS
        } else {
            exit::PARTIAL_FAILURE
        })
    }
}

impl<'a> From<&'a RecordExportSummary> for Outcome<'a> {
    fn from(summary: &'a RecordExportSummary) -> Self {
        Self {
            written: summary.written(),
            skipped: summary.skipped(),
            failures: summary.failures(),
            renamed: summary.renamed(),
            cancelled: summary.cancelled(),
            error_values: summary.error_values(),
            records_with_errors: summary.records_with_errors(),
            error: summary.error().map(|error| error as &dyn Error),
        }
    }
}

impl<'a> From<&'a RecordExportReport> for Outcome<'a> {
    fn from(report: &'a RecordExportReport) -> Self {
        Self {
            written: report.written(),
            skipped: report.skipped(),
            failures: report.failures(),
            renamed: report.renamed(),
            cancelled: report.cancelled(),
            error_values: report.error_values(),
            records_with_errors: report.records_with_errors(),
            error: None,
        }
    }
}

fn reason(error: &dyn Error) -> String {
    let mut reason = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        reason.push_str(": ");
        reason.push_str(&cause.to_string());
        source = cause.source();
    }
    reason
}

/// Prints the failed Records to stderr and the kept files and a summary to stdout.
pub(crate) fn print_report(
    report: &Outcome<'_>,
    target: &Path,
    conflict: Conflict,
    form: RecordForm,
) -> io::Result<()> {
    let file = form.file().unwrap_or(NDJSON_FILE);
    let mut err = io::stderr().lock();
    for failure in report.failures {
        writeln!(
            err,
            "failed {} ({}): {}",
            failure.record_path(),
            failure.guid(),
            reason(failure.error())
        )?;
    }
    write_renamed(&mut err, report.renamed)?;
    if let Some(error) = report.error {
        writeln!(err, "failed {file}: {}", reason(error))?;
    }
    let mut out = io::stdout().lock();
    let kept = if conflict == Conflict::IfChanged {
        "unchanged"
    } else {
        "already exists"
    };
    for file in report.skipped {
        writeln!(out, "skipped {file}: {kept}")?;
    }
    writeln!(
        out,
        "exported {} Records, skipped {}, failed {}, cancelled {} into {}; {} error values in {} Records",
        report.written,
        report.skipped.len(),
        report.failures.len(),
        report.cancelled,
        target.display(),
        report.error_values,
        report.records_with_errors
    )
}
