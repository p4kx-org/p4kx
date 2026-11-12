// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use p4kx_extraction::query::{
    CompiledQuery, Condition, Query, QueryError, ResultFormat, ResultWriter, Row,
};
use p4kx_extraction::{CancelToken, DataForge, DataForgeOrigin};
use serde_json::value::RawValue;

use crate::budget::{Counter, MAX_RESPONSE_LEN, fit_prefix, json_len};
use crate::error::{ToolError, chain};
use crate::forge_file::ForgeSource;
use crate::forge_shapes::{ColumnShape, FailedRow, QueryResult};
use crate::page::Page;
use crate::query_origins::{self, PageEvidence};
use crate::query_selection_sources::{SelectionReceipt, Survey, SurveyMode};

/// The bytes the end of the rows adds: the closing bracket and a line ending.
const CLOSING_LEN: usize = 2;

/// The most columns a Query may select.
pub(crate) const MAX_COLUMNS: usize = 64;

/// The most comparisons a Query may make in its condition.
pub(crate) const MAX_COMPARISONS: usize = 128;

/// How long selecting the Records of a Query may take.
pub(crate) const TIME_LIMIT: Duration = Duration::from_secs(30);

/// What a call runs a Query over and where it keeps the selection of the last Query.
pub(crate) struct Target<'a> {
    pub(crate) forge: &'a DataForge<'a>,
    pub(crate) archive: String,
    pub(crate) origin: &'a DataForgeOrigin,
    pub(crate) opened: &'a ForgeSource,
}

/// Runs `text` and returns one page of its rows: at most the page size, and no more than fit
/// the response cap.
///
/// The Records are selected once per Archive version and Query and kept for the pages that
/// follow; selecting stops with [`ToolError::QueryStopped`] when `cancel` is set or
/// [`TIME_LIMIT`] has passed.
pub(crate) fn run(
    target: &Target<'_>,
    text: &str,
    page: Page,
    cancel: &CancelToken,
) -> Result<QueryResult, ToolError> {
    let query = Query::parse(text).map_err(|error| invalid(&error, text))?;
    check_size(&query)?;
    let canonical = query.to_string();
    let (compiled, records) = compile_selection(target, &query, text, &canonical, cancel)?;
    let selected = compiled.resume(records.records.to_vec());
    let total = selected.len();
    page.check(total)?;
    let rows = selected.rows_observed(page.offset()..page.offset().saturating_add(page.limit()));
    let maximum = write_rows(&compiled, &rows)?.taken;
    let response = |count, mode| {
        let prefix = rows.split_at(count).0;
        let written = write_rows(&compiled, prefix)?;
        let next_cursor = page.next_cursor_after(count, total)?;
        let origins = query_origins::project(&PageEvidence {
            origin: target.origin,
            compiled: &compiled,
            rows: prefix,
            examined: &rows,
            columns: &written.column_positions,
            survey: &records.survey,
            mode,
            cursor: next_cursor.is_some(),
        })?;
        Ok(QueryResult {
            archive: target.archive.clone(),
            query: canonical.clone(),
            columns: columns(&compiled),
            total,
            rows: RawValue::from_string(String::from_utf8(written.json)?)?,
            failed: written.failed,
            unresolved_references: written.unresolved,
            next_cursor,
            origins,
        })
    };
    let (keep, witness) = fit_prefix(
        maximum,
        |count| response(count, SurveyMode::Witness),
        || {
            rows.first().map_or_else(
                || "query result".to_owned(),
                |row| format!("row of {}", row.record_path()),
            )
        },
    )?;
    let complete = response(keep, SurveyMode::Complete)?;
    if crate::budget::structured_len(&complete) <= MAX_RESPONSE_LEN {
        Ok(complete)
    } else {
        Ok(witness)
    }
}

fn compile_selection<'source>(
    target: &Target<'source>,
    query: &Query,
    text: &str,
    canonical: &str,
    cancel: &CancelToken,
) -> Result<(CompiledQuery<'source>, Arc<SelectionReceipt>), ToolError> {
    if let Some(records) = target.opened.selection(canonical) {
        let compiled = query
            .compile(target.forge)
            .map_err(|error| invalid(&error, text))?;
        return Ok((compiled, records));
    }
    let mut survey = Survey::default();
    let (compiled, coverage) = query
        .compile_observing_references(target.forge, |source| survey.record(source))
        .map_err(|error| invalid(&error, text))?;
    survey.set_reference_coverage(coverage);
    let records = selection(target.opened, &compiled, canonical, cancel, survey)?;
    Ok((compiled, records))
}

fn selection(
    opened: &ForgeSource,
    compiled: &CompiledQuery<'_>,
    canonical: &str,
    cancel: &CancelToken,
    survey: Survey,
) -> Result<Arc<SelectionReceipt>, ToolError> {
    let deadline = Instant::now() + TIME_LIMIT;
    let survey = Mutex::new(survey);
    let run = compiled
        .run_observed(
            || !cancel.is_cancelled() && Instant::now() < deadline,
            |source| {
                survey
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .record(source);
            },
        )?
        .ok_or(ToolError::QueryStopped(TIME_LIMIT.as_secs()))?;
    let receipt = Arc::new(SelectionReceipt {
        records: run.indexes().into(),
        survey: survey.into_inner().unwrap_or_else(PoisonError::into_inner),
    });
    opened.keep_selection(canonical, Arc::clone(&receipt));
    Ok(receipt)
}

fn check_size(query: &Query) -> Result<(), ToolError> {
    let columns = query.select().len();
    if columns > MAX_COLUMNS {
        return Err(ToolError::QueryTooComplex {
            what: "columns",
            count: columns,
            max: MAX_COLUMNS,
        });
    }
    let comparisons = query.condition().map_or(0, comparisons);
    if comparisons > MAX_COMPARISONS {
        return Err(ToolError::QueryTooComplex {
            what: "comparisons",
            count: comparisons,
            max: MAX_COMPARISONS,
        });
    }
    Ok(())
}

fn comparisons(condition: &Condition) -> usize {
    match condition {
        Condition::Not(inner) => comparisons(inner),
        Condition::And(parts) | Condition::Or(parts) => parts.iter().map(comparisons).sum(),
        _ => 1,
    }
}

fn invalid(error: &QueryError, text: &str) -> ToolError {
    let span = error.span();
    ToolError::Query {
        start: span.start(),
        end: span.end(),
        marked: error.marked(text).to_string(),
    }
}

fn columns(compiled: &CompiledQuery<'_>) -> Vec<ColumnShape> {
    compiled
        .columns()
        .iter()
        .zip(compiled.column_kinds())
        .map(|(name, kind)| ColumnShape {
            name: name.clone(),
            list: kind.is_multi_valued(),
            types: kind.data_types().iter().map(ToString::to_string).collect(),
        })
        .collect()
}

/// The rows of one page as the JSON `p4kx dcb query` writes, and what else the page holds.
struct Written {
    json: Vec<u8>,
    taken: usize,
    failed: Vec<FailedRow>,
    unresolved: usize,
    column_positions: Vec<usize>,
}

/// Writes `rows` as JSON while they and the failed Records fit the response cap; a counting
/// writer runs one row ahead of the output, so a row is only written once it is known to fit.
fn write_rows(compiled: &CompiledQuery<'_>, rows: &[Row<'_>]) -> Result<Written, ToolError> {
    let columns = compiled.columns();
    let mut ahead = ResultWriter::new(Counter::default(), ResultFormat::Json, columns)?;
    let mut out = ResultWriter::new(Vec::new(), ResultFormat::Json, columns)?;
    let mut taken = 0;
    let mut failed = Vec::new();
    let mut failed_len = 0_usize;
    let mut unresolved = 0_usize;
    for row in rows {
        match row.cells() {
            Ok(cells) => {
                ahead.write_row(row.record_path(), cells)?;
                let used = ahead.get_ref().len() + CLOSING_LEN + failed_len;
                if used > MAX_RESPONSE_LEN {
                    if taken == 0 {
                        return Err(ToolError::TooLargeItem {
                            what: format!("row of {}", row.record_path()),
                            limit: MAX_RESPONSE_LEN,
                            hint: "select fewer or narrower paths",
                        });
                    }
                    break;
                }
                out.write_row(row.record_path(), cells)?;
                unresolved = unresolved.saturating_add(row.unresolved_references());
            }
            Err(error) => {
                let row = FailedRow {
                    record_path: row.record_path().to_owned(),
                    name: row.record().name().unwrap_or_default().to_owned(),
                    error: chain(error),
                };
                let len = json_len(&row);
                if out.get_ref().len() + CLOSING_LEN + failed_len + len > MAX_RESPONSE_LEN {
                    if taken == 0 {
                        return Err(ToolError::TooLargeItem {
                            what: format!("failure report of {}", row.record_path),
                            limit: MAX_RESPONSE_LEN,
                            hint: "select fewer or narrower paths",
                        });
                    }
                    break;
                }
                failed_len += len;
                failed.push(row);
            }
        }
        taken += 1;
    }
    let column_positions = out.json_columns().collect();
    Ok(Written {
        json: out.finish()?,
        taken,
        failed,
        unresolved,
        column_positions,
    })
}
