// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;
use std::io::{self, BufWriter, Write};
use std::path::Path;
use std::process::ExitCode;

use anyhow::{Context, Result};
use p4kx_export::{Language, TableFormat, write_localization_table};
use p4kx_extraction::{
    ArchiveWriteProtection, CancelToken, FieldOrigins, FileReceipt, LocalizationEntry,
    LocalizationNote, LocalizationSet, Lookup, OriginArtifactBinding, OriginDigestWriter,
    SourceInventory, StdoutReceipt, TextSet, decode_line_breaks,
};
use serde::Serialize;
use thiserror::Error;

use crate::cli_loc::{LocAction, LocArgs, LocExportArgs, LocGetArgs, TableOutput};
use crate::dcb_export::FlatConflict;
use crate::{exit, json};

#[path = "loc_locale_scope.rs"]
mod locale_scope;
#[path = "loc_json_origin.rs"]
mod origin;
#[path = "loc_table_origins.rs"]
mod table_origins;

use locale_scope::LocaleScope;
use table_origins::TableOrigins;

/// A language the Archive holds no `global.ini` for was asked for.
#[derive(Debug, Error)]
#[error("the Archive holds no global.ini for the language {language}")]
pub(crate) struct LanguageNotInArchive {
    pub(crate) language: Language,
}

/// `p4kx loc export` found no `global.ini` in the Archive.
#[derive(Debug, Error)]
#[error("the Archive holds no global.ini for any language")]
pub(crate) struct NoLanguages;

/// Runs `p4kx loc`.
pub(crate) fn run(args: &LocArgs, as_json: bool) -> Result<ExitCode> {
    match &args.action {
        LocAction::Languages { archive } => languages(archive, as_json).map(|()| exit_success()),
        LocAction::Get(args) => get(args, as_json),
        LocAction::Export(args) => export(args, as_json).map(|()| exit_success()),
    }
}

/// Prints a note on stderr for every `global.ini` Entry that was read lossily or had lines
/// skipped.
pub(crate) fn print_notes(notes: &[LocalizationNote]) {
    let mut err = io::stderr().lock();
    for note in notes {
        let _ignored = writeln!(err, "note: {note}");
    }
}

fn exit_success() -> ExitCode {
    ExitCode::from(exit::SUCCESS)
}

#[derive(Serialize)]
struct LanguageJson<'a> {
    code: &'static str,
    folder: &'static str,
    entry: &'a str,
}

#[derive(Serialize)]
struct LanguagesJson<'a> {
    archive: String,
    languages: Vec<LanguageJson<'a>>,
}

fn languages(path: &Path, as_json: bool) -> Result<()> {
    let archive = crate::key::open(path)?;
    let present = LocalizationEntry::languages(&archive);
    let listed: Vec<_> = present
        .iter()
        .filter_map(|&language| Some((language, LocalizationEntry::locate(&archive, language)?)))
        .collect();
    let mut out = BufWriter::new(io::stdout().lock());
    if as_json {
        let report = LanguagesJson {
            archive: json::path(archive.path()),
            languages: listed
                .iter()
                .map(|&(language, entry)| LanguageJson {
                    code: language.code(),
                    folder: language.folder(),
                    entry: entry.path(),
                })
                .collect(),
        };
        serde_json::to_writer(&mut out, &origin::languages(&archive, &listed, report)?)?;
        writeln!(out)?;
    } else {
        for (language, entry) in listed {
            writeln!(
                out,
                "{}\t{}\t{}",
                language.code(),
                language.folder(),
                entry.path()
            )?;
        }
    }
    out.flush()?;
    Ok(())
}

#[derive(Serialize)]
struct GetJson<'a> {
    key: &'a str,
    language: &'static str,
    fallback: Option<&'static str>,
    status: &'static str,
    text_language: Option<&'static str>,
    text: Option<&'a str>,
}

fn get(args: &LocGetArgs, as_json: bool) -> Result<ExitCode> {
    let archive = crate::key::open(&args.archive)?;
    let wanted: Vec<Language> = std::iter::once(args.lang).chain(args.fallback).collect();
    let set = LocalizationSet::read(&archive, &wanted)?;
    for &language in &wanted {
        if set.entry(language).is_none() {
            return Err(LanguageNotInArchive { language }.into());
        }
    }
    let texts = set.index()?;
    print_notes(&set.notes(&texts));
    let lookup = texts.lookup(&args.key, args.lang, args.fallback);
    let decoded = lookup.text().map(decode_line_breaks);
    let status = match lookup {
        Lookup::Found { language, .. } if language == args.lang => "found",
        Lookup::Found { .. } => "fallback",
        Lookup::Placeholder => "placeholder",
        _ => "missing",
    };
    let mut out = BufWriter::new(io::stdout().lock());
    if as_json {
        let report = GetJson {
            key: &args.key,
            language: args.lang.code(),
            fallback: args.fallback.map(Language::code),
            status,
            text_language: lookup.language().map(Language::code),
            text: decoded.as_deref(),
        };
        let report = origin::get(&set, &texts, args.lang, args.fallback, lookup, report)?;
        serde_json::to_writer(&mut out, &report)?;
        writeln!(out)?;
    } else if let Some(text) = &decoded {
        writeln!(out, "{}", printable(text))?;
    }
    out.flush()?;
    if lookup.is_missing() {
        let _ignored = writeln!(
            io::stderr().lock(),
            "error: the key {} has no text in {}{}",
            args.key,
            args.lang,
            args.fallback
                .map_or_else(String::new, |fallback| format!(" or {fallback}")),
        );
        return Ok(ExitCode::from(exit::INVALID_INPUT));
    }
    Ok(exit_success())
}

fn printable(text: &str) -> Cow<'_, str> {
    let unsafe_char = |c: char| {
        (c.is_control() && c != '\n' && c != '\t')
            || matches!(c, '\u{061c}' | '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
    };
    if !text.chars().any(unsafe_char) {
        return Cow::Borrowed(text);
    }
    let mut safe = String::with_capacity(text.len());
    for c in text.chars() {
        if unsafe_char(c) {
            safe.extend(c.escape_unicode());
        } else {
            safe.push(c);
        }
    }
    Cow::Owned(safe)
}

#[derive(Serialize)]
struct ExportJson<'a> {
    archive: String,
    out: String,
    format: &'static str,
    languages: Vec<&'a str>,
    keys: usize,
    #[serde(rename = "originReceipt")]
    origin_receipt: String,
    #[serde(rename = "fieldOrigins")]
    field_origins: FieldOrigins,
    provenance: SourceInventory,
}

fn table_format(args: &LocExportArgs) -> Result<TableFormat, FlatConflict> {
    match (args.format, args.csv_raw) {
        (TableOutput::Csv, true) => Ok(TableFormat::CsvRaw),
        (TableOutput::Json, true) => Err(FlatConflict("--csv-raw needs --format csv")),
        (format, false) => Ok(format.into()),
    }
}

fn export(args: &LocExportArgs, as_json: bool) -> Result<()> {
    let format = table_format(args)?;
    let archive = crate::key::open(&args.archive)?;
    crate::guard::ensure_writable_if(args.out.as_deref())?;
    let wanted: &[Language] = if args.lang.is_empty() {
        &Language::ALL
    } else {
        &args.lang
    };
    let set = LocalizationSet::read(&archive, wanted)?;
    if args.lang.is_empty() && set.entries().is_empty() {
        return Err(NoLanguages.into());
    }
    if let Some(&language) = args.lang.iter().find(|&&l| set.entry(l).is_none()) {
        return Err(LanguageNotInArchive { language }.into());
    }
    let columns = export_languages(&set, &args.lang);
    let texts = set.index()?;
    print_notes(&set.notes(&texts));
    let origins = TableOrigins::new(
        &archive,
        &set,
        &texts,
        &columns,
        format,
        args.lang.is_empty(),
    );
    let cancel = CancelToken::new();
    let protection = ArchiveWriteProtection::new(archive.path(), cancel.flag())?;
    if let Some(path) = &args.out {
        let output =
            FileReceipt::new_with_cancel(path, Some(&protection), Some(&protection), cancel)?;
        let published = output
            .publish(
                |writer| origins.render(writer),
                |writer, binding, receipt| receipt.write(writer, binding),
            )
            .with_context(|| format!("cannot write {}", path.display()))?;
        export_summary(
            args,
            as_json,
            &archive,
            &columns,
            &origins,
            &output,
            published.keys,
        )
    } else if args.origin_stderr || args.origin_out.is_some() {
        export_stdout(args, &protection, &origins)
    } else {
        write_raw_table(&texts, &columns, format)
    }
}

fn write_raw_table(texts: &TextSet<'_>, columns: &[Language], format: TableFormat) -> Result<()> {
    let mut out = BufWriter::new(io::stdout().lock());
    write_localization_table(texts, columns, format, &mut out)?;
    out.flush()?;
    Ok(())
}

fn export_stdout(
    args: &LocExportArgs,
    protection: &ArchiveWriteProtection,
    origins: &TableOrigins<'_>,
) -> Result<()> {
    let durable = args
        .origin_out
        .as_deref()
        .map(|path| StdoutReceipt::new(path, Some(protection)))
        .transpose()?;
    let mut out = OriginDigestWriter::new(BufWriter::new(io::stdout().lock()));
    let receipt = origins.render(&mut out)?;
    let binding = OriginArtifactBinding::Stdout {
        digest: out.finish()?,
    };
    if let Some(durable) = durable {
        durable.publish(|writer| receipt.write(writer, &binding))?;
    }
    if args.origin_stderr {
        receipt.write(&mut BufWriter::new(io::stderr().lock()), &binding)?;
    }
    Ok(())
}

fn export_summary(
    args: &LocExportArgs,
    as_json: bool,
    archive: &p4kx_extraction::Archive,
    columns: &[Language],
    origins: &TableOrigins<'_>,
    output: &FileReceipt<'_>,
    keys: usize,
) -> Result<()> {
    let path = args.out.as_ref().context("localization table output")?;
    if as_json {
        let receipt_name = output
            .path()
            .file_name()
            .context("origin companion basename")?;
        let (field_origins, provenance) = origins.summary_origins()?;
        json::print(&ExportJson {
            archive: json::path(archive.path()),
            out: json::path(path),
            format: match args.format {
                TableOutput::Json => "json",
                TableOutput::Csv => "csv",
            },
            languages: columns.iter().map(|language| language.code()).collect(),
            keys,
            origin_receipt: json::path(Path::new(receipt_name)),
            field_origins,
            provenance,
        })?;
    } else {
        writeln!(
            io::stdout().lock(),
            "wrote {}: {keys} keys in {} languages",
            path.display(),
            columns.len()
        )?;
    }
    Ok(())
}

fn export_languages(set: &LocalizationSet, requested: &[Language]) -> Vec<Language> {
    if requested.is_empty() {
        return set.languages().collect();
    }
    let mut columns = Vec::with_capacity(requested.len());
    for &language in requested {
        if !columns.contains(&language) {
            columns.push(language);
        }
    }
    columns
}
