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
mod location;
mod nested;
mod probe;
pub mod query;
mod read_observer;
mod record;
mod record_table;
mod reference_index;
mod rows;
mod scalar;
mod schema;
mod slot;
mod strings;
mod survey;
mod table;
mod tag_build;
mod tags;
mod texts;
mod tree;
mod value;
mod value_error;
mod value_json;
mod value_text;
mod weak;
mod xml;
mod xml_text;
mod xml_write;

pub use annotations::{Annotations, TAG_TYPE, TagResolver};
pub use build::{MAX_NESTING_DEPTH, MAX_RECORD_VALUES, TreeBuilder};
pub use dataforge::DataForge;
pub use error::DataForgeError;
pub use fields::{
    FieldKind, FieldStat, FieldStatistics, FieldStatisticsError, MAX_DISTINCT_VALUES,
    MAX_FIELD_PATHS, Number,
};
pub use flat::{FLAT_COLUMNS, FLAT_TAG_COLUMN, FLAT_TEXT_COLUMN, FlatFormat, FlatValues};
pub use flat_cells::{CSV_BYTE_ORDER_MARK, write_csv_cell};
pub use guid::{Guid, InvalidGuid};
pub use header::{SNIFF_LEN, SUPPORTED_VERSIONS, looks_like_dataforge};
pub use issues::{DataForgeIssue, MAX_DATAFORGE_ISSUES};
pub use json::{JsonLayout, RecordJson};
pub use kinds::{DataType, ValueKind};
pub use layout::{MAX_INHERITANCE_DEPTH, MAX_RECORD_FIELDS};
pub use location::{Location, Step};
pub use nested::{Reference, StructValue};
pub use probe::PointerProbe;
pub use read_observer::CanonicalReadObserver;
pub use record::Record;
pub use reference_index::{FileGraph, IndexedReference, MAX_INDEXED_REFERENCES, ReferenceIndex};
pub use schema::{Enum, Property, Struct};
pub use survey::{FormatSurvey, PointerSurvey, TrailingSurvey};
pub use tags::{
    MAX_TAG_DEPTH, MAX_TAG_NAME_BYTES, MAX_TAG_PATH_BYTES, MAX_TAG_TEXT_BYTES, TAG_DATABASE_TYPE,
    TAG_PATH_SEPARATOR, Tag, TagBuildEvent, TagProjectionFailure, TagRelationIssue, TagTree,
    TagTreeIssues,
};
pub use texts::{Resolution, TextResolver};
pub use tree::RecordTree;
pub use value::{Field, Value};
pub use value_error::ValueError;
pub use xml::{RECORD_XML_NAMESPACE, RecordXml};
