// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

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

pub use crate::catalog_read::original::{
    ExportDiagnostic, ExportDiagnosticExample, FailedExportRecord,
};
