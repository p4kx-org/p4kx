// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use p4kx_extraction::Guid;

use crate::catalog_number_health::NumberRequirement;
use crate::catalog_read::original::RecordSubject;
use crate::catalog_read::original::ReferenceObservation;
use crate::catalog_reference::CatalogReference;
use crate::export_diagnostic::ExportSourceRole;
use crate::export_ledger::SourceObservation;
use crate::health_diagnostic_observation::DiagnosticObservation as ExportDiagnostic;
use crate::selection::Kind;
use crate::{ExportAccountingError, HealthText};

/// Independently exported configuration fields on one physical Record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum ConfigurationInspection {
    MissionBoard,
    ResourceNetwork,
    LootGeneration,
    JumpDriveDefinition,
    WeaponActionRecoilDefinition,
    WeaponProceduralDefinition,
    EntityAnimationController,
}

/// Inspection of the canonical values relevant to one source scope.
#[derive(Debug, Clone)]
#[must_use]
pub(crate) struct FieldInspection {
    pub(crate) kind: Option<Kind>,
    pub(crate) record_index: u32,
    pub(crate) guid: Guid,
    pub(crate) record_path: HealthText,
    pub(crate) subject: RecordSubject,
    pub(crate) role: ExportSourceRole,
    pub(crate) complete: bool,
    pub(crate) root_field: Option<&'static str>,
    pub(crate) configuration: Option<ConfigurationInspection>,
    pub(crate) diagnostics: Vec<ExportDiagnostic>,
    /// Conditional failures until the exact physical source has an emitted lossless row.
    pub(crate) number_requirements: Vec<NumberRequirement>,
    pub(crate) unresolved_references: Vec<ReferenceObservation>,
    pub(crate) catalog_references: Vec<CatalogReference>,
}

impl FieldInspection {
    /// Reuses the same immutable canonical inspection for an explicit source role or output kind.
    pub(crate) fn contextualize(&mut self, kind: Option<Kind>, role: ExportSourceRole) {
        self.kind = kind;
        self.role = role;
        for diagnostic in &mut self.diagnostics {
            diagnostic.kind = kind;
            diagnostic.source_role = Some(role);
        }
        for reference in &mut self.unresolved_references {
            reference.contextualize(kind, role);
        }
        for reference in &mut self.catalog_references {
            reference.original.contextualize(kind, role);
        }
    }
}

impl FieldInspection {
    pub(crate) fn merge(&mut self, other: Self) -> Result<(), ExportAccountingError> {
        if self != &other {
            return Err(ExportAccountingError::ConflictingFieldInspection);
        }
        self.subject.merge(other.subject)?;
        for (required, other) in self
            .number_requirements
            .iter_mut()
            .zip(other.number_requirements)
        {
            required.merge(other)?;
        }
        for (reference, other) in self
            .unresolved_references
            .iter()
            .zip(other.unresolved_references)
        {
            reference.merge(other)?;
        }
        for (reference, other) in self.catalog_references.iter().zip(other.catalog_references) {
            reference.original.merge(other.original)?;
        }
        for diagnostic in &mut self.diagnostics {
            for example in &mut diagnostic.examples {
                example.merge_owner(&self.subject)?;
            }
        }
        Ok(())
    }
}

// Canonical value equality is checked before the separate original-subject merge.
impl PartialEq for FieldInspection {
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind
            && self.record_index == other.record_index
            && self.guid == other.guid
            && self.record_path == other.record_path
            && self.role == other.role
            && self.complete == other.complete
            && self.root_field == other.root_field
            && self.configuration == other.configuration
            && self.diagnostics == other.diagnostics
            && self.number_requirements == other.number_requirements
            && self.unresolved_references == other.unresolved_references
            && self.catalog_references == other.catalog_references
    }
}

impl Eq for FieldInspection {}

/// Immutable field receipts indexed by whole Record or the exact field consumed by a reader.
#[derive(Default)]
pub(crate) struct FieldInspections {
    whole: BTreeMap<u32, FieldInspection>,
    scoped: BTreeMap<(u32, usize, ExportSourceRole), FieldInspection>,
    configurations: BTreeMap<(u32, ConfigurationInspection), FieldInspection>,
}

impl FieldInspections {
    pub(crate) fn index(inspections: Vec<FieldInspection>) -> Result<Self, ExportAccountingError> {
        let mut indexed = Self::default();
        for mut inspection in inspections {
            if let Some(configuration) = inspection.configuration {
                if inspection.root_field.is_some()
                    || inspection.kind.is_some()
                    || inspection.role != ExportSourceRole::Dependency
                {
                    return Err(ExportAccountingError::ConflictingFieldInspection);
                }
                let key = (inspection.record_index, configuration);
                Self::merge(indexed.configurations.entry(key), inspection)?;
            } else if inspection.root_field.is_some() {
                let kind = inspection
                    .kind
                    .ok_or(ExportAccountingError::ConflictingFieldInspection)?;
                if inspection.role != ExportSourceRole::Dependency {
                    return Err(ExportAccountingError::ConflictingFieldInspection);
                }
                let key = (inspection.record_index, kind.index(), inspection.role);
                Self::merge(indexed.scoped.entry(key), inspection)?;
            } else {
                inspection.contextualize(None, ExportSourceRole::Dependency);
                Self::merge(indexed.whole.entry(inspection.record_index), inspection)?;
            }
        }
        Ok(indexed)
    }

    fn merge<Key>(
        entry: std::collections::btree_map::Entry<'_, Key, FieldInspection>,
        inspection: FieldInspection,
    ) -> Result<(), ExportAccountingError>
    where
        Key: Ord,
    {
        match entry {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(inspection);
                Ok(())
            }
            std::collections::btree_map::Entry::Occupied(mut entry) => {
                entry.get_mut().merge(inspection)
            }
        }
    }

    pub(crate) const fn whole(&self) -> &BTreeMap<u32, FieldInspection> {
        &self.whole
    }

    pub(crate) fn all(&self) -> impl Iterator<Item = &FieldInspection> {
        self.whole
            .values()
            .chain(self.scoped.values())
            .chain(self.configurations.values())
    }

    pub(crate) fn for_source(
        &self,
        source: &SourceObservation,
    ) -> Result<&FieldInspection, ExportAccountingError> {
        if let Some(configuration) = source.configuration {
            return self
                .configurations
                .get(&(source.record_index, configuration))
                .ok_or(ExportAccountingError::MissingFieldInspection);
        }
        source
            .kind
            .and_then(|kind| {
                self.scoped
                    .get(&(source.record_index, kind.index(), source.role))
            })
            .or_else(|| self.whole.get(&source.record_index))
            .ok_or(ExportAccountingError::MissingFieldInspection)
    }
}
