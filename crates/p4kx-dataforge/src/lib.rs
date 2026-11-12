// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Reads DataForge, the typed object database of Star Citizen stored in `Data\Game2.dcb`.
//!
//! [`DataForge::open`] reads versions 6 and 8 without copying: the header, the struct, property,
//! enum, data mapping and Record tables, the value pools and the text and blob tables are checked
//! once and then read as typed views with explicit little-endian decoding. The versions differ
//! only in the Record table, whose version 8 rows carry a tag. Any other version, version 5 and
//! older included, is [`DataForgeError::UnsupportedVersion`], never a best-effort parse
//! (ADR-0136).
//!
//! [`Record::tree`] builds the canonical [`RecordTree`] of one Record, the single model every
//! output is made from: owned children nested, References as links (ADR-0004). [`RecordXml`]
//! writes it as Record XML (`docs/record-xml.md`), [`RecordJson`] as Record JSON
//! (`docs/record-json.md`), [`FlatValues`] as one row per leaf value (`docs/record-flat.md`).
//! [`query`] selects Records and values inside them with Queries
//! (`docs/query.md`). [`FieldStatistics`] counts the values at every field path of some Records.
//! [`TagTree`] reads the `TagDatabase` into the tree of tags with their tag paths, and
//! [`Annotations`] lets the two writers add those paths and localised texts to their output.

mod annotations;
mod build;
mod dataforge;
mod error;
mod field_escapes;
mod field_storage;
mod fields;
mod flat;
mod flat_cells;
mod guid;
mod header;
mod instance;
mod issues;
mod json;
mod json_text;
mod kinds;
mod layout;
mod layout_cursor;
mod location;
mod nested;
mod pointer_offsets;
mod positioned_record;
mod probe;
pub mod query;
mod read_observer;
mod record;
mod record_metadata;
mod record_table;
mod reference_index;
mod reference_origin;
mod rows;
mod scalar;
mod schema;
mod slot;
mod source_layout_allocation;
mod source_layout_index;
mod source_layout_properties;
#[cfg(test)]
mod source_layout_tests;
mod source_layout_validation;
mod strings;
mod survey;
mod table;
mod tag_build;
mod tags;
mod texts;
mod tree;
mod tree_cursor;
mod tree_owner;
mod value;
mod value_clone;
mod value_debug;
mod value_drain;
mod value_eq;
mod value_error;
mod value_json;
mod value_text;
mod weak;
mod xml;
mod xml_text;
mod xml_write;

pub use annotations::{Annotations, TAG_TYPE, TagResolver};
pub use build::TreeBuilder;
pub use dataforge::DataForge;
pub use error::DataForgeError;
pub use field_storage::FieldStorage;
pub use fields::{
    FieldKind, FieldStat, FieldStatistics, FieldStatisticsError, MAX_DISTINCT_VALUES,
    MAX_FIELD_PATHS, Number,
};
pub use flat::{FLAT_COLUMNS, FLAT_TAG_COLUMN, FLAT_TEXT_COLUMN, FlatFormat, FlatValues};
pub use flat_cells::{CSV_BYTE_ORDER_MARK, write_csv_cell};
pub use guid::{Guid, InvalidGuid};
pub use header::{SNIFF_LEN, SUPPORTED_VERSIONS, looks_like_dataforge};
pub use issues::{DataForgeIssue, MAX_DATAFORGE_ISSUES};
pub use json::{
    JsonLayout, RecordJson, RecordJsonBytes, RecordJsonObserver, RecordJsonRecordField,
    RecordJsonSource,
};
pub use kinds::{DataType, ValueKind};
pub use location::{Location, Step};
pub use nested::{Reference, StructValue};
pub use positioned_record::PositionedRecord;
pub use probe::PointerProbe;
pub use read_observer::CanonicalReadObserver;
pub use record::Record;
pub use record_metadata::{RecordMetadata, RecordText};
pub use reference_index::{
    FileGraph, IndexedReference, IndexedReferenceOrigin, MAX_INDEXED_REFERENCES, ObservedFileEdge,
    ObservedFileGraph, ObservedReferenceIndex, ReferenceIndex, ReferenceIndexError,
    ReferenceScanGap,
};
pub use reference_origin::ReferenceOrigin;
pub use schema::{Enum, Property, Struct};
pub use survey::{FormatSurvey, PointerSurvey, TrailingSurvey};
#[expect(
    deprecated,
    reason = "Public compatibility for the released Tag constants"
)]
pub use tags::{MAX_TAG_DEPTH, MAX_TAG_NAME_BYTES, MAX_TAG_PATH_BYTES, MAX_TAG_TEXT_BYTES};
pub use tags::{
    TAG_DATABASE_TYPE, TAG_PATH_SEPARATOR, Tag, TagBuildEvent, TagPath, TagPathEdgeKind,
    TagPathIncoming, TagRelationIssue, TagTree, TagTreeIssues,
};
pub use texts::{Resolution, TextResolver};
pub use tree::RecordTree;
pub use tree_cursor::ValueVisit;
pub use value::{Field, Value};
pub use value_error::ValueError;
pub use xml::{RECORD_XML_NAMESPACE, RecordXml};
