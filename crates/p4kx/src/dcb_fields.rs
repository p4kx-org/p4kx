// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, BufWriter, Write};
use std::process::ExitCode;

use anyhow::Result;
use p4kx_extraction::field_statistics_report::{
    FieldStatisticsAddressError, FieldStatisticsReport, FieldStatisticsReportError,
};
use p4kx_extraction::query::Query;
use p4kx_extraction::{CsvCellMode, DataForge, FieldStat, FieldStatistics, Filter};

use crate::cli::{DcbFieldsArgs, QueryFormat};
use crate::dcb_export::FlatConflict;
use crate::dcb_query::{InvalidQuery, compile_error};
use crate::dcb_refs::JsonWithTable;
use crate::forge_input::ForgeInput;
use crate::{dcb_fields_json, dcb_fields_out, exit};

/// The words a Query starts with; a selection that starts with another word is a type name.
const QUERY_WORDS: [&str; 5] = ["from", "at", "referencing", "select", "where"];

/// The Records to count as the Query text that selects them.
struct Selection {
    text: String,
    query: Query,
}

enum PlainFormat {
    Table,
    Csv(CsvCellMode),
}

/// The counts of a result, with the paths it lists.
pub(crate) struct Listing<'a> {
    pub(crate) statistics: &'a FieldStatistics,
    pub(crate) fields: Vec<&'a FieldStat>,
}

/// Counts the field paths of the Archive's DataForge as the arguments ask and writes the result
/// to stdout.
pub(crate) fn run(args: &DcbFieldsArgs, as_json: bool) -> Result<ExitCode> {
    let format = match (as_json, args.format) {
        (true, Some(QueryFormat::Table | QueryFormat::Csv | QueryFormat::CsvStrict)) => {
            return Err(JsonWithTable.into());
        }
        (true, _) => QueryFormat::Json,
        (false, format) => format.unwrap_or(QueryFormat::Table),
    };
    if args.csv_raw && format != QueryFormat::Csv {
        return Err(FlatConflict("--csv-raw needs --format csv").into());
    }
    let query = args
        .selection
        .as_deref()
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(parse_selection)
        .transpose()?;
    let input = ForgeInput::open(&args.archive)?;
    let forge = input.entry().open()?;
    let plain = match format {
        QueryFormat::Json => return run_json(&input, &forge, query.as_ref(), args.path.as_deref()),
        QueryFormat::Table => PlainFormat::Table,
        QueryFormat::Csv if args.csv_raw => PlainFormat::Csv(CsvCellMode::Raw),
        QueryFormat::Csv => PlainFormat::Csv(CsvCellMode::Spreadsheet),
        QueryFormat::CsvStrict => PlainFormat::Csv(CsvCellMode::Strict),
    };
    let statistics = statistics(&forge, query.as_ref())?;
    let filter = args.path.as_deref().map(Filter::parse).transpose()?;
    let fields: Vec<&FieldStat> = statistics
        .fields()
        .iter()
        .filter(|field| {
            filter
                .as_ref()
                .is_none_or(|filter| filter.matches(field.path()))
        })
        .collect();
    let listing = Listing {
        statistics: &statistics,
        fields,
    };
    let mut out = BufWriter::new(io::stdout().lock());
    let written = match plain {
        PlainFormat::Table => dcb_fields_out::table(&mut out, &listing),
        PlainFormat::Csv(mode) => dcb_fields_out::csv(&mut out, &listing, mode),
    }
    .and_then(|()| out.flush());
    finish(written, &statistics)
}

fn run_json(
    input: &ForgeInput,
    forge: &DataForge<'_>,
    selection: Option<&Selection>,
    path: Option<&str>,
) -> Result<ExitCode> {
    let source = input.source_input()?;
    let result = selection.map_or_else(
        || FieldStatisticsReport::all(forge, source),
        |selection| FieldStatisticsReport::query(forge, source, &selection.query),
    );
    let report = result.map_err(|error| report_error(error, selection))?;
    let filter = path.map(Filter::parse).transpose()?;
    let displayed = report.addressed_origins(filter.as_ref())?;
    let statistics = report.reduction.statistics();
    let fields = displayed
        .ordinals
        .iter()
        .map(|&ordinal| {
            statistics
                .fields()
                .get(ordinal)
                .ok_or(FieldStatisticsAddressError::MissingRow)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let listing = Listing { statistics, fields };
    let mut out = BufWriter::new(io::stdout().lock());
    let written = dcb_fields_json::write(&mut out, input.path(), &listing, &report, &displayed)
        .and_then(|()| out.flush());
    finish(written, statistics)
}

fn report_error(error: FieldStatisticsReportError, selection: Option<&Selection>) -> anyhow::Error {
    match error {
        FieldStatisticsReportError::Compile(error) => {
            if let Some(selection) = selection {
                compile_error(&error, &selection.text)
            } else {
                error.into()
            }
        }
        FieldStatisticsReportError::Select(error) => error.into(),
        FieldStatisticsReportError::Scan(error) => error.into(),
        other => other.into(),
    }
}

fn finish(written: io::Result<()>, statistics: &FieldStatistics) -> Result<ExitCode> {
    match written {
        Err(error) if error.kind() != io::ErrorKind::BrokenPipe => return Err(error.into()),
        Ok(()) | Err(_) => {}
    }
    report(statistics);
    Ok(ExitCode::from(if statistics.unreadable_records() == 0 {
        exit::SUCCESS
    } else {
        exit::PARTIAL_FAILURE
    }))
}

/// Reads the selection as a Query, or as a type name.
fn parse_selection(text: &str) -> Result<Selection, InvalidQuery> {
    let first = text.split_whitespace().next().unwrap_or_default();
    let query_text = if QUERY_WORDS
        .iter()
        .any(|word| first.eq_ignore_ascii_case(word))
    {
        text.to_owned()
    } else if text.contains('`') {
        return Err(InvalidQuery(format!(
            "a type name cannot hold a backtick: {text}"
        )));
    } else {
        format!("from `{text}`")
    };
    match Query::parse(&query_text) {
        Ok(query) => Ok(Selection {
            text: query_text,
            query,
        }),
        Err(error) => Err(InvalidQuery(error.marked(&query_text).to_string())),
    }
}

/// Selects the Records of `query` and counts their field paths; without a Query every Record.
fn statistics(forge: &DataForge<'_>, selection: Option<&Selection>) -> Result<FieldStatistics> {
    let Some(selection) = selection else {
        return Ok(FieldStatistics::of_all(forge)?);
    };
    let compiled = selection
        .query
        .compile(forge)
        .map_err(|error| compile_error(&error, &selection.text))?;
    let run = compiled.run()?;
    Ok(FieldStatistics::of_records(forge, run.indexes())?)
}

/// Reports on stderr what could not be read; a report that cannot be written is dropped.
fn report(statistics: &FieldStatistics) {
    let mut err = io::stderr().lock();
    if statistics.unreadable_records() > 0 {
        let _unreported = writeln!(
            err,
            "{} Records could not be read",
            statistics.unreadable_records()
        );
    }
    if statistics.unreadable_values() > 0 {
        let _unreported = writeln!(
            err,
            "{} values could not be read and are not counted",
            statistics.unreadable_values()
        );
    }
}
