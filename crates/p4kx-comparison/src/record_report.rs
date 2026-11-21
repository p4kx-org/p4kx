// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

#[path = "report_record_origins.rs"]
mod origins;

use crate::report_origins::{ReportOriginFields, ReportOrigins};
use crate::value_report::{GuidSchema, ValueReport, display};
use crate::{
    Comparison, FieldChange, Guid, RecordChange, RecordChangeKind, RecordCounts, RecordError,
    RecordSide, RecordState,
};

/// The Records added, removed, moved, changed and unreadable.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct RecordLevel<'a> {
    /// The number of Records per change kind.
    counts: RecordCounts,
    /// Every added, removed, moved, changed or unreadable Record, ordered by record path ignoring
    /// case and separator kind, then by GUID.
    changes: Vec<RecordChangeReport<'a>>,
    #[serde(flatten)]
    #[schemars(with = "ReportOriginFields")]
    origins: ReportOrigins,
}

/// One added, removed, moved, changed or unreadable Record.
#[derive(Debug, Serialize, JsonSchema)]
struct RecordChangeReport<'a> {
    /// The GUID that joins both sides.
    #[serde(serialize_with = "display")]
    #[schemars(with = "GuidSchema")]
    guid: Guid,
    /// How the Record differs.
    change: RecordChangeKind,
    /// The Record on the old side, absent when it was added or cannot be read.
    #[serde(skip_serializing_if = "Option::is_none")]
    old: Option<RecordStateReport<'a>>,
    /// The Record on the new side, absent when it was removed or cannot be read.
    #[serde(skip_serializing_if = "Option::is_none")]
    new: Option<RecordStateReport<'a>>,
    /// The changed fields and array elements, nested ones included, depth first in field order
    /// of the new side with the fields only the old side has last in each struct; empty for an
    /// added, removed or unreadable Record.
    fields: Vec<FieldChangeReport<'a>>,
    /// Why the Record cannot be read, one entry per side; absent unless it is unreadable.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    errors: Vec<RecordErrorReport>,
}

/// Why a Record cannot be read on one side.
#[derive(Debug, Serialize, JsonSchema)]
struct RecordErrorReport {
    /// The side whose Record cannot be read.
    side: RecordSide,
    /// What cannot be read.
    reason: String,
}

impl From<&RecordError> for RecordErrorReport {
    fn from(error: &RecordError) -> Self {
        Self {
            side: error.side(),
            reason: error.error().to_string(),
        }
    }
}

/// What identifies a Record on one side.
#[derive(Debug, Serialize, JsonSchema)]
struct RecordStateReport<'a> {
    /// The record path, without `.xml`.
    record_path: &'a str,
    /// The Record name, e.g. `EntityClassDefinition.AEGS_Avenger`.
    name: &'a str,
    /// The Record tag, e.g. `SystemsDesign`; absent in a version 6 DataForge and for a Record
    /// without tag.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    tag: Option<&'a str>,
    /// The name of the Record's struct.
    #[serde(rename = "type")]
    type_name: &'a str,
}

/// One field or array element whose value differs, or that only one side has.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct FieldChangeReport<'a> {
    /// Where the value lies in the Record: the field names separated by `.`, with `[index]`
    /// after an array field, e.g. `hull.plates[1].armor`.
    path: &'a str,
    /// The old value, absent when the old side has no such field or element.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) old: Option<ValueReport<'a>>,
    /// The new value, absent when the new side has no such field or element.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) new: Option<ValueReport<'a>>,
}

impl<'a> From<RecordState<'a>> for RecordStateReport<'a> {
    fn from(state: RecordState<'a>) -> Self {
        Self {
            record_path: state.record_path(),
            name: state.name(),
            tag: state.tag(),
            type_name: state.type_name(),
        }
    }
}

impl<'a> From<&'a FieldChange<'_>> for FieldChangeReport<'a> {
    fn from(change: &'a FieldChange<'_>) -> Self {
        Self {
            path: change.path().as_str(),
            old: change.old_value().and_then(ValueReport::of),
            new: change.new_value().and_then(ValueReport::of),
        }
    }
}

impl<'a> From<&'a RecordChange<'_>> for RecordChangeReport<'a> {
    fn from(change: &'a RecordChange<'_>) -> Self {
        Self {
            guid: change.guid(),
            change: change.kind(),
            old: change.old_record().map(RecordStateReport::from),
            new: change.new_record().map(RecordStateReport::from),
            fields: change
                .fields()
                .iter()
                .map(FieldChangeReport::from)
                .collect(),
            errors: change
                .errors()
                .iter()
                .map(RecordErrorReport::from)
                .collect(),
        }
    }
}

pub(crate) fn record_level<'a>(comparison: &'a Comparison<'_>) -> RecordLevel<'a> {
    let changes = comparison
        .record_changes()
        .iter()
        .map(RecordChangeReport::from)
        .collect::<Vec<_>>();
    RecordLevel {
        counts: comparison.record_counts(),
        origins: ReportOrigins(origins::capture(comparison, &changes)),
        changes,
    }
}
