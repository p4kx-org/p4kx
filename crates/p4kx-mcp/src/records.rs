// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForge, Guid, JsonLayout, Record, RecordJson, TreeBuilder};
use serde_json::value::RawValue;

use crate::budget::{Budget, Capped, MAX_RESPONSE_LEN, json_len};
use crate::error::ToolError;
use crate::forge_shapes::RecordResult;
use crate::page::Page;

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
    archive: String,
    selector: &Selector,
    page: Page,
) -> Result<RecordResult, ToolError> {
    let records = select(forge, selector)?;
    let total = records.len();
    let mut budget = Budget::new();
    budget.take(json_len(&(&archive, total)).saturating_add(128));
    let mut builder = TreeBuilder::new();
    let mut writer = RecordJson::new(JsonLayout::Compact);
    let mut taken = Vec::new();
    for record in records.iter().skip(page.offset()).take(page.limit()) {
        let tree = builder.build(record)?;
        let mut out = Capped::new(budget.left());
        let fits = writer.write(&tree, &mut out).is_ok();
        let bytes = out.into_bytes();
        if !fits || !budget.take(bytes.len()) {
            if taken.is_empty() {
                return Err(ToolError::TooLargeItem {
                    what: format!("Record {}", record.name()?),
                    limit: MAX_RESPONSE_LEN,
                    hint: "run_query selects single values of it",
                });
            }
            break;
        }
        taken.push(RawValue::from_string(String::from_utf8(bytes)?)?);
    }
    let next_cursor = page.next_cursor_after(taken.len(), total)?;
    Ok(RecordResult {
        archive,
        total,
        records: taken,
        next_cursor,
    })
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
