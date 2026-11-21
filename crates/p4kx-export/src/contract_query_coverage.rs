// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Guid;

use crate::CatalogReadFailure;
use crate::catalog_queries::CatalogRecordCoverage;
use crate::catalog_read::original::RecordSubjectKey;
use crate::export_diagnostic::{
    ExportDiagnosticCode, ExportDiagnosticSeverity, ExportDiagnosticStage, ExportDiagnosticUnit,
    ExportSourceRole,
};
use crate::health_diagnostic_observation::DiagnosticObservation;
use crate::{HealthText, Kind};

/// Source scope of a selected Contract query and its outside-root joins.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct ContractQueryCoverage<'source> {
    /// Coverage of the explicitly selected physical Generator owners.
    pub selected: CatalogRecordCoverage,
    /// Source candidates read to resolve dependencies and canonical selection.
    pub dependencies: ContractDependencyCoverage<'source>,
    /// Generator or mission dependency projections that could not contribute rows.
    pub projection_failures: Vec<CatalogReadFailure<'source>>,
    /// Root candidate rows replaced by the canonical winner from another physical owner.
    pub shadowed_candidates: Vec<ShadowedContract>,
    /// Canonical source and projection diagnostics with original subject coordinates.
    pub diagnostics: Vec<ContractQueryDiagnostic>,
}

/// Actual source-only surveys and complete source transactions supporting the selected roots.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct ContractDependencyCoverage<'source> {
    /// Physical Generator owners examined for intro grants, logical duplicates and property aliases.
    pub surveyed_generators: Vec<u32>,
    /// Outside-root Generator owners selected for whole-source projection.
    pub related_generators: Vec<u32>,
    /// Forward-linked physical mission and starmap candidates considered in source order.
    pub forward_records: Vec<u32>,
    /// Forward candidates whose original header or body could not be read.
    pub forward_failures: Vec<CatalogReadFailure<'source>>,
    /// Headers that prevented a complete reverse survey.
    pub unreadable_headers: Vec<u32>,
    /// Original errors that prevented exclusion of potentially relevant Generator candidates.
    pub survey_failures: Vec<CatalogReadFailure<'source>>,
}

/// A selected logical row whose global canonical winner belongs to another physical source.
#[derive(Debug)]
#[non_exhaustive]
pub struct ShadowedContract {
    /// The parent contract identity.
    pub contract_guid: Guid,
    /// The sub-contract identity, absent on a parent row.
    pub sub_contract_guid: Option<Guid>,
    /// The explicitly selected physical source that lost canonical selection.
    pub selected_record_index: u32,
    /// The physical owner selected by the canonical duplicate ordering.
    pub winner_record_index: u32,
}

/// A canonical diagnostic before report-local metadata positions are assigned.
#[derive(Debug)]
pub struct ContractQueryDiagnostic(DiagnosticObservation);

/// Original source coordinates attached to one canonical diagnostic.
#[derive(Debug)]
#[non_exhaustive]
pub struct ContractDiagnosticExample<'read> {
    /// The actual owner position when available.
    pub record_index: Option<u32>,
    /// The actual owner GUID when available.
    pub guid: Option<Guid>,
    /// The canonical original field location.
    pub field: Option<&'read HealthText>,
    /// The original declaring source type.
    pub source_type: Option<&'read HealthText>,
    /// The observed target identity, when applicable.
    pub target_guid: Option<Guid>,
}

impl ContractQueryDiagnostic {
    pub(super) const fn new(value: DiagnosticObservation) -> Self {
        Self(value)
    }

    /// The affected canonical dataset.
    #[must_use]
    pub const fn kind(&self) -> Option<Kind> {
        self.0.kind
    }

    /// The canonical diagnostic stage.
    #[must_use]
    pub const fn stage(&self) -> ExportDiagnosticStage {
        self.0.stage
    }

    /// The canonical semantic diagnostic category.
    #[must_use]
    pub const fn code(&self) -> ExportDiagnosticCode {
        self.0.code
    }

    /// The canonical severity.
    #[must_use]
    pub const fn severity(&self) -> ExportDiagnosticSeverity {
        self.0.severity
    }

    /// The source's relationship to this query.
    #[must_use]
    pub const fn source_role(&self) -> Option<ExportSourceRole> {
        self.0.source_role
    }

    /// The stable warning code, when present.
    #[must_use]
    pub fn warning_code(&self) -> Option<&str> {
        self.0.warning_code.as_deref()
    }

    /// The observed count and its canonical unit.
    #[must_use]
    pub const fn count(&self) -> (usize, ExportDiagnosticUnit) {
        (self.0.count, self.0.count_unit)
    }

    /// The original diagnostic subjects without serialized report-local indirection.
    pub fn examples(&self) -> impl Iterator<Item = ContractDiagnosticExample<'_>> {
        self.0.examples.iter().map(|example| {
            let key = example.subject().key();
            ContractDiagnosticExample {
                record_index: key.map(RecordSubjectKey::index),
                guid: key.map(RecordSubjectKey::guid),
                field: example.field.as_ref(),
                source_type: example.source_type.as_ref(),
                target_guid: example.target_guid,
            }
        })
    }
}
