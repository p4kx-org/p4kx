// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;

use p4kx_extraction::{
    DataForge, DataForgeOrigin, Guid, JsonLayout, Record, RecordJson, TreeBuilder,
};
use serde_json::value::RawValue;

use crate::budget::{Budget, Capped, MAX_RESPONSE_LEN, fit_prefix, json_len};
use crate::error::ToolError;
use crate::forge_shapes::RecordResult;
use crate::page::Page;
use crate::record_origins::{self, Capture, CapturedRecord, Navigation};

/// How `get_record` names its Records.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum Selector {
    /// The Record with this GUID.
    Guid(String),
    /// The Records with this record path.
    Path(String),
}

impl Selector {
    /// Returns the selector of a call that passes exactly one of `guid` and `path`.
    pub(crate) fn new(guid: Option<String>, path: Option<String>) -> Result<Self, ToolError> {
        match (guid, path) {
            (Some(guid), None) => Ok(Self::Guid(guid)),
            (None, Some(path)) => Ok(Self::Path(path)),
            _ => Err(ToolError::RecordSelector),
        }
    }
}

/// Returns one page of the Records `selector` names as Record JSON, as many as fit the response
/// cap.
pub(crate) fn get(
    forge: &DataForge<'_>,
    archive: &str,
    origin: &DataForgeOrigin,
    selector: &Selector,
    page: Page,
) -> Result<RecordResult, ToolError> {
    let records = select(forge, selector)?;
    let navigation = Navigation::new(forge, selector);
    let captured = capture_page(&records, archive, page)?;
    let first = records.get(page.offset());
    let (_, response) = fit_prefix(
        captured.rows.len(),
        |count| {
            let selected = captured.rows.split_at(count).0;
            let next_cursor = page.next_cursor_after(count, records.len())?;
            let origins = record_origins::project(
                origin,
                &navigation,
                selected,
                next_cursor.as_ref().map(|_| captured.examined.as_slice()),
            )?;
            Ok(RecordResult {
                archive: archive.to_owned(),
                total: records.len(),
                records: selected.iter().map(|item| item.value.clone()).collect(),
                next_cursor,
                origins,
            })
        },
        || first.map_or_else(|| "get_record".to_owned(), record_description),
    )
    .map_err(with_query_hint)?;
    Ok(response)
}

#[derive(Default)]
struct CapturedPage {
    rows: Vec<CapturedRecord>,
    examined: Vec<u32>,
}

fn capture_page(
    records: &[Record<'_>],
    archive: &str,
    page: Page,
) -> Result<CapturedPage, ToolError> {
    let mut budget = Budget::new();
    budget.take(json_len(&(archive, records.len())));
    let mut builder = TreeBuilder::new();
    let mut writer = RecordJson::new(JsonLayout::Compact);
    let mut captured = CapturedPage::default();
    for record in page.slice(records) {
        captured.examined.push(record.index());
        let tree = builder.build_positioned(record)?;
        let mut out = Capped::new(budget.left());
        let mut capture = Capture::new(budget.left());
        match writer.write_observed(&tree, &mut out, &mut capture) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::FileTooLarge => {
                if captured.rows.is_empty() {
                    return Err(oversized_record(record));
                }
                break;
            }
            Err(error) => return Err(error.into()),
        }
        let bytes = out.into_bytes();
        if !budget.take(bytes.len().saturating_add(capture.minimum_wire_len())) {
            if captured.rows.is_empty() {
                return Err(oversized_record(record));
            }
            break;
        }
        captured.rows.push(CapturedRecord {
            index: record.index(),
            value: RawValue::from_string(String::from_utf8(bytes)?)?,
            capture,
        });
    }
    Ok(captured)
}

fn oversized_record(record: &Record<'_>) -> ToolError {
    ToolError::TooLargeItem {
        what: record_description(record),
        limit: MAX_RESPONSE_LEN,
        hint: "run_query selects single values of it",
    }
}

fn record_description(record: &Record<'_>) -> String {
    record.name().map_or_else(
        |_| format!("Record {}", record.index()),
        |name| format!("Record {name}"),
    )
}

fn with_query_hint(error: ToolError) -> ToolError {
    match error {
        ToolError::TooLargeItem { what, limit, .. } => ToolError::TooLargeItem {
            what,
            limit,
            hint: "run_query selects single values of it",
        },
        other => other,
    }
}

fn select<'f>(forge: &'f DataForge<'f>, selector: &Selector) -> Result<Vec<Record<'f>>, ToolError> {
    match selector {
        Selector::Guid(text) => {
            let guid: Guid = text
                .trim()
                .parse()
                .map_err(|_| ToolError::BadGuid(text.clone()))?;
            let record = forge
                .record_by_guid(guid)
                .ok_or_else(|| ToolError::UnknownGuid(guid.to_string()))?;
            Ok(vec![record])
        }
        Selector::Path(path) => {
            let records: Vec<Record<'f>> = forge.records_at_path(path).collect();
            if records.is_empty() {
                return Err(ToolError::UnknownRecordPath(path.clone()));
            }
            Ok(records)
        }
    }
}
