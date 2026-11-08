// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::contract::every_field_required;
use crate::guid_text::CatalogGuid;
use crate::selection::Kind;

/// The maximum number of deterministic examples retained per diagnostic aggregate.
pub const EXPORT_DIAGNOSTIC_EXAMPLE_LIMIT: usize = 8;

/// The source or projection stage described by a diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(extend("x-p4kx-vocabularyClass" = "control"))]
pub enum ExportDiagnosticStage {
    /// Deciding whether unreadable metadata belongs to the selection.
    Selection,
    /// Reading the canonical Record tree or a required Archive Entry.
    SourceRead,
    /// Mapping a readable source into typed catalog rows.
    Projection,
    /// Deriving secondary data while the original row may remain present.
    DerivedProjection,
    /// Interpreting one optional field or configured Reference.
    SourceField,
}

/// The stable semantic category of a source diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = crate::schema_vocabulary::open_string_vocabulary)]
pub enum ExportDiagnosticCode {
    /// Optional source data is absent.
    OptionalAbsent,
    /// A source field explicitly contains an empty value.
    EmptyValue,
    /// A configured Reference names no available target.
    DanglingReference,
    /// A weak pointer identifies an instance outside its original owner.
    OutsideOwner,
    /// A populated concrete value is unsupported.
    UnsupportedValue,
    /// A present source value has an invalid shape.
    MalformedSource,
    /// A present source field could not be decoded.
    UnreadableSource,
    /// Source-field inspection reached a declared depth or value budget.
    FieldWalkLimit,
    /// Canonical source reading failed.
    SourceReadFailure,
    /// Typed source projection failed.
    ProjectionFailure,
    /// A secondary derived projection failed.
    DerivedProjectionFailure,
    /// Source metadata could not establish selection membership.
    SelectionFailure,
    /// The required source inputs could not all be enumerated.
    IncompleteInventory,
    /// One exported identity names incompatible physical source payloads.
    AmbiguousIdentity,
    /// A resolved source Reference has no available target row in this catalog selection.
    CatalogTargetUnavailable,
    /// A source warning retains its established warning code.
    SourceWarning,
}

/// Whether a diagnostic blocks normal publication.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(extend("x-p4kx-vocabularyClass" = "control"))]
pub enum ExportDiagnosticSeverity {
    /// A represented source state that does not make the export incomplete.
    Information,
    /// A source warning that makes the export incomplete.
    Warning,
    /// A failed required operation.
    Error,
}

/// The relationship of a source identity to the affected kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(extend("x-p4kx-vocabularyClass" = "control"))]
pub enum ExportSourceRole {
    /// A source selected by the affected kind.
    Selected,
    /// An original source selected as a derived kind's input.
    DerivedInput,
    /// A required source that is not a selected output row of the affected kind.
    Dependency,
    /// Unreadable metadata prevented a definitive selection decision.
    SelectionCandidate,
}

/// The unit counted by an aggregate diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(extend("x-p4kx-vocabularyClass" = "control"))]
pub enum ExportDiagnosticUnit {
    /// Distinct source Record identities.
    Records,
    /// Distinct Archive Entries identified by the producing warning.
    ArchiveEntries,
    /// Distinct required operations on Archive Entries.
    EntryOperations,
    /// Stored fields or values.
    Values,
    /// Derived projection operations.
    Projections,
    /// Emitted or intentionally skipped rows.
    Rows,
    /// Warning occurrences whose source has no more specific count.
    Occurrences,
}

/// One exact failed source identity, retained independently of bounded diagnostic examples.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct FailedExportRecord {
    /// The affected kind, null for a catalog-wide source or dependency.
    pub kind: Option<Kind>,
    /// The physical Record-table index in this source build.
    pub record_index: u32,
    /// The original source Record GUID.
    #[schemars(with = "CatalogGuid")]
    pub guid: String,
    /// The source Record path, null when its metadata cannot be read.
    pub record_path: Option<String>,
    /// The relationship of this source to the affected kind.
    pub role: ExportSourceRole,
    /// The failed stage.
    pub stage: ExportDiagnosticStage,
    /// The stable failure category.
    pub code: ExportDiagnosticCode,
}

/// A bounded diagnostic example containing identities and field metadata without raw values.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct ExportDiagnosticExample {
    /// The physical Record-table index in this source build when available.
    pub record_index: Option<u32>,
    /// The source Record GUID when available.
    #[schemars(with = "Option<CatalogGuid>")]
    pub record_guid: Option<String>,
    /// The source Record path when available.
    pub record_path: Option<String>,
    /// The canonical source field location when available.
    pub field: Option<String>,
    /// The technical source struct name when available.
    pub source_type: Option<String>,
    /// A configured target GUID when available.
    #[schemars(with = "Option<CatalogGuid>")]
    pub target_guid: Option<String>,
}

/// One aggregate diagnostic with deterministic bounded examples.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct ExportDiagnostic {
    /// The affected kind, null for a catalog-wide dependency.
    pub kind: Option<Kind>,
    /// The source or projection stage.
    pub stage: ExportDiagnosticStage,
    /// The stable semantic category.
    pub code: ExportDiagnosticCode,
    /// Whether this diagnostic blocks normal publication.
    pub severity: ExportDiagnosticSeverity,
    /// The source relationship, null for a source warning without Record identity.
    pub source_role: Option<ExportSourceRole>,
    /// The established warning code, null for a non-warning diagnostic.
    pub warning_code: Option<String>,
    /// The total number of affected units, independently of the bounded example list.
    pub count: usize,
    /// What the count measures.
    pub count_unit: ExportDiagnosticUnit,
    /// Deterministically ordered examples without raw confidential source values.
    pub examples: Vec<ExportDiagnosticExample>,
}
