// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Comparison service of P4KX: the Entries added, removed and changed and the Records
//! added, removed, moved and changed between two Archives, as a queryable result and as a JSON
//! report with a generated schema, the line diff of one Entry pair, and the schema drift between
//! two DataForges.

mod changed_value;
mod compare;
mod comparison;
mod cryxml_lines;
mod decode;
mod entry_change;
mod entry_incomparable;
mod error;
mod field_change_evidence;
mod field_diff;
mod field_diff_inputs;
mod field_reference_evidence;
mod line_diff;
mod line_diff_hunks;
mod line_diff_origins;
mod line_diff_report;
mod line_diff_side;
mod line_match;
mod options;
mod owned;
mod path_key;
mod record_change;
mod record_compare;
mod record_report;
mod report;
mod report_entry_aggregates;
mod report_origins;
mod report_record_body;
mod schema_diff;
mod schema_report;
mod schema_side;
mod schema_types;
mod text_diff;
mod value_report;

pub use changed_value::ChangedValue;
pub use compare::{compare, compare_with_dataforge_entries};
pub use comparison::{Comparison, Side};
pub use decode::{Encoding, MAX_DIFF_LEN, MAX_DIFF_LINES};
pub use entry_change::{ChangeKind, DirectoryCounts, EntryChange, EntryCounts};
pub use entry_incomparable::{IncomparableEntry, IncomparableReason};
pub use error::{ComparisonError, LineDiffError};
pub use line_diff::{DiffBody, LineDiff, NotText, line_diff, line_diff_cancellable};
pub use options::{CompareOptions, ComparisonProgress, DataForges, Level, Levels};
pub use owned::{OwnedComparison, OwnedDataForges, RecompareError, RecordLookup};
pub use p4kx_archive::{Archive, ArchiveError, Build, Entry};
pub use p4kx_cryxml::Signature;
pub use p4kx_dataforge::{
    DataForge, DataForgeError, Field, Guid, Location, Record, Reference, Step, StructValue, Value,
    ValueError,
};
pub use p4kx_extraction::{CancelToken, DataForgeEntry, DataForgeLoadError, Filter};
pub use record_change::{
    FieldChange, RecordChange, RecordChangeKind, RecordCounts, RecordError, RecordKey, RecordSide,
    RecordState,
};
pub use report::{REPORT_FORMAT_VERSION, Report, report_schema};
pub use schema_diff::compare_schema;
pub use schema_report::{
    SCHEMA_REPORT_FORMAT_VERSION, SchemaReport, schema_report, schema_report_schema,
};
pub use schema_types::{
    EnumChange, KindCounts, OptionChange, OptionChangeKind, ParentChange, PropertyAspect,
    PropertyChange, PropertyShape, SchemaChangeKind, SchemaCounts, SchemaDiff, SideCounts,
    StructChange,
};
pub use text_diff::{DEFAULT_CONTEXT, DiffLine, Hunk, LineKind, TextDiff};
