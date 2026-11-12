// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{FieldOrigins, SourceInventory};
use schemars::JsonSchema;
use serde::Serialize;
use serde_json::value::RawValue;

/// The Records with one GUID or one record path, each as Record JSON.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct RecordResult {
    /// The path of the Archive.
    pub(crate) archive: String,
    /// The number of Records with the GUID or the record path.
    pub(crate) total: usize,
    /// The Records on this page in definition order, each as Record JSON (`docs/record-json.md`):
    /// `$type`, `$guid`, `$path` and the fields; References are objects with `guid`, `path` and
    /// `type`, never the Record they link to.
    #[schemars(with = "Vec<serde_json::Value>")]
    pub(crate) records: Vec<Box<RawValue>>,
    /// The cursor of the next page; absent on the last page.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) next_cursor: Option<String>,
    #[serde(flatten)]
    pub(crate) origins: ForgeFieldOrigins,
}

/// One Record as a listing shows it.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct RecordShape {
    /// The GUID; it gets the Record with `get_record`.
    pub(crate) guid: String,
    /// The name, e.g. `EntityClassDefinition.AEGS_Avenger`.
    pub(crate) name: String,
    /// The type, the name of the Record's struct.
    #[serde(rename = "type")]
    pub(crate) type_name: String,
    /// The record path.
    pub(crate) path: String,
}

/// The Records a search matched.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct RecordSearchResult<Records = Vec<RecordShape>> {
    /// The path of the Archive.
    pub(crate) archive: String,
    /// The number of matching Records.
    pub(crate) total: usize,
    /// The matching Records on this page, in Record order.
    #[schemars(with = "Vec<RecordShape>")]
    pub(crate) records: Records,
    /// The cursor of the next page; absent on the last page.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) next_cursor: Option<String>,
    #[serde(flatten)]
    pub(crate) origins: ForgeFieldOrigins,
}

#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct ForgeFieldOrigins {
    #[serde(rename = "fieldOrigins")]
    pub(crate) fields: FieldOrigins,
    pub(crate) provenance: SourceInventory,
}

/// One field a struct declares.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct FieldShape {
    /// The field name, as a Query path names it.
    pub(crate) name: String,
    /// The data type, e.g. `Int32`, `String`, `Enum`, `Class`, `StrongPointer` or `Reference`.
    #[serde(rename = "type")]
    pub(crate) data_type: String,
    /// Whether the field holds an array of values.
    pub(crate) array: bool,
    /// The struct of a class or pointer field, or of the Records a typed Reference links to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) target: Option<String>,
    /// The enum of an enum field.
    #[serde(rename = "enum", skip_serializing_if = "Option::is_none")]
    pub(crate) enum_name: Option<String>,
}

/// One struct definition.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct StructShape {
    /// The struct name, a type name in Queries.
    pub(crate) name: String,
    /// The struct it inherits its first fields from; its fields come before these.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) parent: Option<String>,
    /// The fields the struct declares itself, in order, without the inherited ones.
    pub(crate) fields: Vec<FieldShape>,
}

/// Struct definitions.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct StructsResult {
    /// The path of the Archive.
    pub(crate) archive: String,
    /// The number of matching structs.
    pub(crate) total: usize,
    /// The matching structs on this page, by name ignoring case.
    pub(crate) structs: Vec<StructShape>,
    /// The cursor of the next page; absent on the last page.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) next_cursor: Option<String>,
    #[serde(flatten)]
    pub(crate) origins: ForgeFieldOrigins,
}

/// One enum definition.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct EnumShape {
    /// The enum name.
    pub(crate) name: String,
    /// The values in order.
    pub(crate) values: Vec<String>,
}

/// Enum definitions.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct EnumsResult {
    /// The path of the Archive.
    pub(crate) archive: String,
    /// The number of matching enums.
    pub(crate) total: usize,
    /// The matching enums on this page, by name ignoring case.
    pub(crate) enums: Vec<EnumShape>,
    /// The cursor of the next page; absent on the last page.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) next_cursor: Option<String>,
    #[serde(flatten)]
    pub(crate) origins: ForgeFieldOrigins,
}

/// One column of a Query result after the record path.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct ColumnShape {
    /// The heading: the path in canonical form, the key of the column in every row.
    pub(crate) name: String,
    /// Whether every cell is a list, for a path through `[*]` or `[TypeName]`.
    pub(crate) list: bool,
    /// The data types of the values the path ends at.
    pub(crate) types: Vec<String>,
}

/// A selected Record whose values cannot be read; it has no row.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct FailedRow {
    /// The record path of the Record.
    pub(crate) record_path: String,
    /// The name of the Record.
    pub(crate) name: String,
    /// Why its values cannot be read.
    pub(crate) error: String,
}

/// One page of a Query result.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct QueryResult {
    /// The path of the Archive.
    pub(crate) archive: String,
    /// The Query in canonical form.
    pub(crate) query: String,
    /// The columns after the record path.
    pub(crate) columns: Vec<ColumnShape>,
    /// The number of selected Records, one row each.
    pub(crate) total: usize,
    /// The rows on this page in Record order, each an object keyed `record path` and by column
    /// heading, as `p4kx dcb query --format json` writes them.
    #[schemars(with = "serde_json::Value")]
    pub(crate) rows: Box<RawValue>,
    /// The selected Records on this page whose values cannot be read.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) failed: Vec<FailedRow>,
    /// The null or dangling links `->` met in the rows of this page, once per row.
    pub(crate) unresolved_references: usize,
    /// The cursor of the next page; absent on the last page.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) next_cursor: Option<String>,
}
