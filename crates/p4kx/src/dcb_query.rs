// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result};
use p4kx_extraction::query::{
    CompiledQuery, Query, QueryError, QueryErrorKind, QueryRun, ResultFormat, ResultWriter,
};
use p4kx_extraction::{
    ArchiveWriteProtection, CancelToken, FileReceipt, OriginArtifactBinding, OriginDigestWriter,
    StdoutReceipt, is_same_file,
};
use serde::Serialize;
use thiserror::Error;

use crate::cli::{DcbQueryArgs, QueryFormat};
use crate::dcb_export::FlatConflict;
use crate::forge_input::ForgeInput;
use crate::query_rows::{Counts, RowMode, for_each_row};
use crate::{exit, json, query_file, query_table};

#[path = "dcb_query_origins.rs"]
mod origins;

/// A Query that cannot be parsed or compiled, with its marked position.
#[derive(Debug, Error)]
#[error("{0}")]
pub(crate) struct InvalidQuery(pub(crate) String);

/// A Query naming a type whose layout the DataForge cannot give, with its marked position.
#[derive(Debug, Error)]
#[error("{0}")]
pub(crate) struct UnreadableSchema(String);

/// `--json` with a format other than JSON and without `--out`, so stdout would not hold JSON.
#[derive(Debug, Error)]
#[error("--json writes JSON to stdout: use --format json, or --out for the {} result", .0.name())]
pub(crate) struct JsonNeedsJsonFormat(QueryFormat);

/// `--out` names the Archive the Query reads.
#[derive(Debug, Error)]
#[error("{} is the Archive the Query reads", .0.display())]
pub(crate) struct OutputIsArchive(PathBuf);

#[derive(Serialize)]
struct Summary {
    archive: String,
    format: &'static str,
    output: String,
    records: u64,
    failed: u64,
    unresolved_references: u64,
    #[serde(rename = "originReceipt", skip_serializing_if = "Option::is_none")]
    origin_receipt: Option<String>,
}

/// A compiled Query with its selected Records, written in one format.
struct Output<'a, 'q, 'f> {
    compiled: &'a CompiledQuery<'f>,
    run: &'a QueryRun<'q, 'f>,
    format: QueryFormat,
    raw: bool,
}

/// Runs a Query over the Archive's DataForge and writes the result to stdout or `--out`.
pub(crate) fn run(args: &DcbQueryArgs, as_json: bool) -> Result<ExitCode> {
    let format = args.format.unwrap_or(if as_json {
        QueryFormat::Json
    } else {
        QueryFormat::Table
    });
    if as_json && args.out.is_none() && format != QueryFormat::Json {
        return Err(JsonNeedsJsonFormat(format).into());
    }
    if args.csv_raw && format != QueryFormat::Csv {
        return Err(FlatConflict("--csv-raw needs --format csv").into());
    }
    if let Some(out) = args.out.as_ref().or(args.origin_out.as_ref())
        && is_same_file(out, &args.archive)
    {
        return Err(OutputIsArchive(out.clone()).into());
    }
    let query = Query::parse(&args.query)
        .map_err(|error| InvalidQuery(error.marked(&args.query).to_string()))?;
    let input = ForgeInput::open(&args.archive)?;
    let forge = input.entry().open()?;
    let compiled = query
        .compile(&forge)
        .map_err(|error| compile_error(&error, &args.query))?;
    let run = compiled.run()?;
    let output = Output {
        compiled: &compiled,
        run: &run,
        format,
        raw: args.csv_raw,
    };
    let counts = output.publish(args, &input, as_json)?;
    Ok(ExitCode::from(if counts.failed == 0 {
        exit::SUCCESS
    } else {
        exit::PARTIAL_FAILURE
    }))
}

impl Output<'_, '_, '_> {
    fn publish(&self, args: &DcbQueryArgs, input: &ForgeInput, as_json: bool) -> Result<Counts> {
        let mut counts = Counts::default();
        match &args.out {
            Some(out) => {
                let receipt = if args.origin_receipt {
                    Some(self.to_observed_file(out, input, &mut counts)?)
                } else {
                    self.to_file(out, input.path(), &mut counts)?;
                    None
                };
                print_summary(args, out, self.format, counts, as_json, receipt)?;
            }
            None if args.origin_stderr || args.origin_out.is_some() => {
                self.to_observed_stdout(args, input, &mut counts)?;
            }
            None => self.to_stdout(&mut counts)?,
        }
        Ok(counts)
    }

    fn to_file(&self, out: &Path, input: &Path, counts: &mut Counts) -> Result<()> {
        let cancel = CancelToken::new();
        let protection = ArchiveWriteProtection::new(input, cancel.flag())?;
        query_file::write_replacing_protected(out, Some(&protection), Some(&cancel), |writer| {
            self.write(writer, false, counts)
        })
        .with_context(|| format!("cannot write {}", out.display()))
    }

    fn to_observed_file(
        &self,
        out: &Path,
        input: &ForgeInput,
        counts: &mut Counts,
    ) -> Result<String> {
        let source = input.source_input()?;
        let cancel = CancelToken::new();
        let protection = ArchiveWriteProtection::new(input.path(), cancel.flag())?;
        let receipt =
            FileReceipt::new_with_cancel(out, Some(&protection), Some(&protection), cancel)?;
        let name = receipt
            .path()
            .file_name()
            .context("query origin companion basename")?;
        receipt
            .publish(
                |out| {
                    origins::render(
                        self.compiled,
                        self.run,
                        source,
                        self.writer_format(false),
                        out,
                        counts,
                    )
                },
                |out, binding, result| result.write(out, binding),
            )
            .with_context(|| format!("cannot write {}", out.display()))?;
        Ok(json::path(Path::new(name)))
    }

    const fn csv_format(&self) -> ResultFormat {
        if self.raw {
            ResultFormat::CsvRaw
        } else {
            ResultFormat::Csv
        }
    }

    const fn writer_format(&self, count_line: bool) -> origins::Format {
        match self.format {
            QueryFormat::Table => origins::Format::Table { count_line },
            QueryFormat::Csv => origins::Format::Flat(self.csv_format()),
            QueryFormat::CsvStrict => origins::Format::Flat(ResultFormat::CsvStrict),
            QueryFormat::Json => origins::Format::Flat(ResultFormat::Json),
        }
    }

    /// Writes the result to stdout; a closed pipe ends the output early without an error.
    fn to_stdout(&self, counts: &mut Counts) -> io::Result<()> {
        let out = BufWriter::new(io::stdout().lock());
        match self.write(out, true, counts) {
            Err(error) if error.kind() == io::ErrorKind::BrokenPipe => Ok(()),
            written => written,
        }
    }

    fn to_observed_stdout(
        &self,
        args: &DcbQueryArgs,
        input: &ForgeInput,
        counts: &mut Counts,
    ) -> Result<()> {
        let cancel = CancelToken::new();
        let protection = ArchiveWriteProtection::new(input.path(), cancel.flag())?;
        let durable = args
            .origin_out
            .as_deref()
            .map(|path| StdoutReceipt::new(path, Some(&protection)))
            .transpose()?;
        let source = input.source_input()?;
        let mut out = OriginDigestWriter::new(BufWriter::new(io::stdout().lock()));
        let measured = origins::render(
            self.compiled,
            self.run,
            source,
            self.writer_format(true),
            &mut out,
            counts,
        )
        .and_then(|companion| Ok((companion, out.finish()?)));
        let (companion, digest) = match measured {
            Err(error) if error.kind() == io::ErrorKind::BrokenPipe => return Ok(()),
            measured => measured?,
        };
        let binding = OriginArtifactBinding::Stdout { digest };
        if let Some(durable) = durable {
            durable.publish(|out| companion.write(out, &binding))?;
        }
        if args.origin_stderr {
            companion.write(&mut BufWriter::new(io::stderr().lock()), &binding)?;
        }
        Ok(())
    }

    /// Writes the result to `out`, page by page, and reports failed Records on stderr; with
    /// `count_line`, a table ends with the number of Records.
    fn write(&self, out: impl Write, count_line: bool, counts: &mut Counts) -> io::Result<()> {
        let columns = self.compiled.columns();
        let format = match self.writer_format(count_line) {
            origins::Format::Table { count_line } => {
                return query_table::write(columns, self.run, out, count_line, counts, None);
            }
            origins::Format::Flat(format) => format,
        };
        let mut writer = ResultWriter::new(out, format, columns)?;
        for_each_row(self.run, counts, RowMode::Raw, |row, cells| {
            writer.write_row(row.record_path(), cells)
        })?;
        writer.finish().map(drop)
    }
}

/// Returns the error for a Query that does not compile: a schema the DataForge cannot give is
/// [`UnreadableSchema`], anything else [`InvalidQuery`].
pub(crate) fn compile_error(error: &QueryError, query: &str) -> anyhow::Error {
    let marked = error.marked(query).to_string();
    if matches!(error.kind(), QueryErrorKind::DataForge(_)) {
        UnreadableSchema(marked).into()
    } else {
        InvalidQuery(marked).into()
    }
}

fn print_summary(
    args: &DcbQueryArgs,
    out: &Path,
    format: QueryFormat,
    counts: Counts,
    as_json: bool,
    origin_receipt: Option<String>,
) -> io::Result<()> {
    if as_json {
        return json::print(&Summary {
            archive: json::path(&args.archive),
            format: format.name(),
            output: json::path(out),
            records: counts.records,
            failed: counts.failed,
            unresolved_references: counts.unresolved,
            origin_receipt,
        });
    }
    let written = format!(" written to {}", out.display());
    writeln!(io::stdout().lock(), "{}", counts.line(&written))
}
