// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::health_diagnostic_subject::DiagnosticSubject;
use super::health_text_capture::HealthTextCell;
use crate::guid_text::CatalogGuid;
use crate::{
    ExportReferenceStatus, ExportSourceRole, HealthText, HealthTextRef, HealthTextView, Kind,
    schema_fields,
};

/// Exact original pointer error parameters with their separate source contributions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = schema_fields::every_field_required)]
pub struct ExportSourcePointer {
    #[serde(rename = "structIndex")]
    structure: SourcedValue<u32>,
    #[serde(rename = "instanceIndex")]
    instance: SourcedValue<u32>,
    #[serde(rename = "declaredTypeIndex")]
    declared_type: SourcedValue<u32>,
}

impl ExportSourcePointer {
    pub(super) const fn new(
        structure: SourcedValue<u32>,
        instance: SourcedValue<u32>,
        declared_type: SourcedValue<u32>,
    ) -> Self {
        Self {
            structure,
            instance,
            declared_type,
        }
    }

    /// Borrows the exact retained source struct-index parameter.
    #[must_use]
    pub const fn struct_index(&self) -> &SourcedValue<u32> {
        &self.structure
    }

    /// Borrows the exact retained source instance-index parameter.
    #[must_use]
    pub const fn instance_index(&self) -> &SourcedValue<u32> {
        &self.instance
    }

    /// Borrows the declared source type-index parameter when applicable.
    #[must_use]
    pub const fn declared_type_index(&self) -> &SourcedValue<u32> {
        &self.declared_type
    }
}

/// Independently sourced body facts belonging to one original unresolved occurrence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = schema_fields::every_field_required)]
pub(super) struct ReferenceBodyCells {
    #[schemars(with = "SourcedValue<HealthTextRef>")]
    pub(super) field: HealthTextCell,
    #[schemars(with = "SourcedValue<HealthTextRef>")]
    pub(super) source_type: HealthTextCell,
    #[schemars(with = "SourcedValue<CatalogGuid>")]
    pub(super) target_guid: SourcedValue<HealthText>,
    pub(super) target_record_index: SourcedValue<u32>,
    #[schemars(with = "SourcedValue<HealthTextRef>")]
    pub(super) target_record_path: HealthTextCell,
    #[schemars(with = "SourcedValue<HealthTextRef>")]
    pub(super) target_type: HealthTextCell,
    #[schemars(with = "SourcedValue<HealthTextRef>")]
    pub(super) target_location: HealthTextCell,
    pub(super) source_pointer: Option<ExportSourcePointer>,
}

/// One unsampled unresolved link with immutable directly paired source cells and text views.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = schema_fields::every_field_required)]
pub struct ExportUnresolvedReference {
    kind: Option<Kind>,
    source_role: ExportSourceRole,
    #[serde(flatten)]
    subject: DiagnosticSubject,
    #[serde(flatten)]
    body: ReferenceBodyCells,
    status: ExportReferenceStatus,
}

impl ExportUnresolvedReference {
    pub(super) const fn new(
        kind: Option<Kind>,
        source_role: ExportSourceRole,
        subject: DiagnosticSubject,
        body: ReferenceBodyCells,
        status: ExportReferenceStatus,
    ) -> Self {
        Self {
            kind,
            source_role,
            subject,
            body,
            status,
        }
    }

    /// Returns this occurrence's catalog-kind context.
    #[must_use]
    pub const fn kind(&self) -> Option<Kind> {
        self.kind
    }

    /// Returns this occurrence's source relationship to its catalog kind.
    #[must_use]
    pub const fn source_role(&self) -> ExportSourceRole {
        self.source_role
    }

    /// Returns the retained source-link state.
    #[must_use]
    pub const fn status(&self) -> ExportReferenceStatus {
        self.status
    }

    /// Borrows the physical original owner position.
    #[must_use]
    pub const fn record_index(&self) -> &SourcedValue<u32> {
        self.subject.record_index()
    }

    /// Borrows the original owning Record GUID.
    #[must_use]
    pub const fn record_guid(&self) -> &SourcedValue<HealthText> {
        self.subject.record_guid()
    }

    /// Borrows this occurrence's owner-path storage cell.
    #[must_use]
    pub const fn record_path(&self) -> &SourcedValue<HealthTextRef> {
        self.subject.record_path()
    }

    /// Borrows the owner path with this occurrence's own original contributions.
    #[must_use]
    pub fn record_path_text(&self) -> HealthTextView<'_> {
        self.subject.record_path_text()
    }

    /// Borrows this occurrence's canonical source-field storage cell.
    #[must_use]
    pub const fn field(&self) -> &SourcedValue<HealthTextRef> {
        self.body.field.cell()
    }

    /// Borrows the complete source-field path with its own original contributions.
    #[must_use]
    pub fn field_text(&self) -> HealthTextView<'_> {
        self.body.field.view()
    }

    /// Borrows this occurrence's source-body type storage cell.
    #[must_use]
    pub const fn source_type(&self) -> &SourcedValue<HealthTextRef> {
        self.body.source_type.cell()
    }

    /// Borrows the original source-body type with its own original contributions.
    #[must_use]
    pub fn source_type_text(&self) -> HealthTextView<'_> {
        self.body.source_type.view()
    }

    /// Borrows the exact target GUID datum when available.
    #[must_use]
    pub const fn target_guid(&self) -> &SourcedValue<HealthText> {
        &self.body.target_guid
    }

    /// Borrows the physical target Record-position datum when available.
    #[must_use]
    pub const fn target_record_index(&self) -> &SourcedValue<u32> {
        &self.body.target_record_index
    }

    /// Borrows this occurrence's target-path storage cell.
    #[must_use]
    pub const fn target_record_path(&self) -> &SourcedValue<HealthTextRef> {
        self.body.target_record_path.cell()
    }

    /// Borrows the complete target path with this occurrence's own original contributions.
    #[must_use]
    pub fn target_record_path_text(&self) -> HealthTextView<'_> {
        self.body.target_record_path.view()
    }

    /// Borrows this occurrence's target-type storage cell.
    #[must_use]
    pub const fn target_type(&self) -> &SourcedValue<HealthTextRef> {
        self.body.target_type.cell()
    }

    /// Borrows the actual target type with this occurrence's own original contributions.
    #[must_use]
    pub fn target_type_text(&self) -> HealthTextView<'_> {
        self.body.target_type.view()
    }

    /// Borrows this occurrence's target-location storage cell.
    #[must_use]
    pub const fn target_location(&self) -> &SourcedValue<HealthTextRef> {
        self.body.target_location.cell()
    }

    /// Borrows the complete target location with its own original contributions.
    #[must_use]
    pub fn target_location_text(&self) -> HealthTextView<'_> {
        self.body.target_location.view()
    }

    /// Borrows exact original pointer error parameters when applicable.
    #[must_use]
    pub const fn source_pointer(&self) -> Option<&ExportSourcePointer> {
        self.body.source_pointer.as_ref()
    }
}
