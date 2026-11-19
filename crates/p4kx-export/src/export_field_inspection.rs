// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use p4kx_extraction::Guid;

use crate::catalog_read::original::RecordSubject;
use crate::catalog_reference::CatalogReference;
use crate::export_diagnostic::ExportSourceRole;
use crate::export_ledger::SourceObservation;
use crate::selection::Kind;
use crate::{ExportAccountingError, HealthText};

#[path = "inspection_findings.rs"]
mod findings;
#[path = "scan_inspection_fold.rs"]
mod scan_fold;

pub(crate) use findings::InspectionFindings;
pub(crate) use scan_fold::ScanInspectionFold;

/// Independently exported configuration fields on one physical Record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum ConfigurationInspection {
    MissionBoard,
    ResourceNetwork,
    ResourceComponent,
    ResourceConnection,
    FuelTank,
    FuelIntake,
    ResourceContainer,
    RadarOptions,
    LootGeneration,
    JumpDriveDefinition,
    JumpPoint,
    JumpPointGlobal,
    WeaponActionRecoilDefinition,
    WeaponProceduralDefinition,
    EntityAnimationController,
    FlightController,
    Ifcs,
    Thruster,
    HackableDevice,
    SuitArmor,
    VehicleArmor,
    Clothing,
    ItemRadar,
    DamageResistanceMacro,
    TurretHealth,
    TurretHealthModifier,
    ConsumableComponent,
    ConsumableDefinition,
    MiningLaser,
    MiningLaserGlobal,
    AttachableModifier,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum InspectionKey {
    Whole(u32),
    Scoped(u32, usize, ExportSourceRole),
    Configuration(u32, ConfigurationInspection),
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
    pub(crate) findings: InspectionFindings,
    pub(crate) catalog_references: Vec<CatalogReference>,
}

impl FieldInspection {
    /// Reuses the same immutable canonical inspection for an explicit source role or output kind.
    pub(crate) fn contextualize(&mut self, kind: Option<Kind>, role: ExportSourceRole) {
        self.kind = kind;
        self.role = role;
        self.findings.contextualize(kind, role);
    }
}

impl FieldInspection {
    const fn is_whole(&self) -> bool {
        self.root_field.is_none() && self.configuration.is_none()
    }

    fn canonicalize_whole(&mut self) {
        self.contextualize(None, ExportSourceRole::Dependency);
    }

    fn key(&self) -> Result<InspectionKey, ExportAccountingError> {
        if let Some(configuration) = self.configuration {
            if self.root_field.is_some()
                || self.kind.is_some()
                || self.role != ExportSourceRole::Dependency
            {
                return Err(ExportAccountingError::ConflictingFieldInspection);
            }
            return Ok(InspectionKey::Configuration(
                self.record_index,
                configuration,
            ));
        }
        if self.root_field.is_some() {
            let kind = self
                .kind
                .ok_or(ExportAccountingError::ConflictingFieldInspection)?;
            if self.role != ExportSourceRole::Dependency {
                return Err(ExportAccountingError::ConflictingFieldInspection);
            }
            return Ok(InspectionKey::Scoped(
                self.record_index,
                kind.index(),
                self.role,
            ));
        }
        Ok(InspectionKey::Whole(self.record_index))
    }

    fn absorbs_scan_duplicate(&self, other: &mut Self, key: InspectionKey) -> bool {
        if !self.findings.number_requirements().is_empty()
            || !other.findings.number_requirements().is_empty()
        {
            return false;
        }
        let context = (other.kind, other.role);
        if matches!(key, InspectionKey::Whole(_)) && context != (self.kind, self.role) {
            other.contextualize(self.kind, self.role);
        }
        let same = self
            .check_duplicate_shape(other)
            .and_then(|()| self.check_duplicate_references(other))
            .and_then(|()| self.check_sample_owners())
            .is_ok();
        if !same && context != (other.kind, other.role) {
            other.contextualize(context.0, context.1);
        }
        same
    }

    pub(crate) fn merge(&mut self, mut other: Self) -> Result<(), ExportAccountingError> {
        self.check_duplicate_shape(&other)?;
        self.findings.merge_numbers(&mut other.findings)?;
        self.check_duplicate_references(&other)?;
        self.check_sample_owners()
    }

    fn check_duplicate_shape(&self, other: &Self) -> Result<(), ExportAccountingError> {
        if self != other {
            return Err(ExportAccountingError::ConflictingFieldInspection);
        }
        let mut subject = self.subject.clone();
        subject.merge(other.subject.clone())
    }

    fn check_duplicate_references(&self, other: &Self) -> Result<(), ExportAccountingError> {
        for (reference, other) in self
            .findings
            .unresolved_references()
            .iter()
            .zip(other.findings.unresolved_references())
        {
            reference.check_duplicate(other)?;
        }
        for (reference, other) in self
            .catalog_references
            .iter()
            .zip(&other.catalog_references)
        {
            reference.proof.check_duplicate(&other.proof)?;
        }
        Ok(())
    }

    fn check_sample_owners(&self) -> Result<(), ExportAccountingError> {
        for diagnostic in self.findings.diagnostics() {
            for example in &diagnostic.examples {
                example.check_owner(&self.subject)?;
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
            && self.findings == other.findings
            && self.catalog_references == other.catalog_references
    }
}

impl Eq for FieldInspection {}

/// Matches whole inspections from sequential reads of the same physical Records.
pub(crate) struct WholeInspectionFold<'a> {
    earlier: &'a mut [FieldInspection],
    positions: BTreeMap<u32, usize>,
}

impl<'a> WholeInspectionFold<'a> {
    pub(crate) fn new(earlier: &'a mut [FieldInspection]) -> Self {
        let mut positions = BTreeMap::new();
        for (position, inspection) in earlier.iter().enumerate() {
            if inspection.is_whole() {
                positions.entry(inspection.record_index).or_insert(position);
            }
        }
        Self { earlier, positions }
    }

    pub(crate) fn merge_matching(
        &mut self,
        inspections: &mut Vec<FieldInspection>,
    ) -> Result<(), ExportAccountingError> {
        let positions = &self.positions;
        for mut inspection in inspections.extract_if(.., |inspection| {
            inspection.is_whole() && positions.contains_key(&inspection.record_index)
        }) {
            let retained = positions
                .get(&inspection.record_index)
                .and_then(|&position| self.earlier.get_mut(position))
                .ok_or(ExportAccountingError::ConflictingFieldInspection)?;
            retained.canonicalize_whole();
            inspection.canonicalize_whole();
            retained.merge(inspection)?;
        }
        Ok(())
    }
}

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
            match inspection.key()? {
                InspectionKey::Configuration(record, configuration) => {
                    Self::merge(
                        indexed.configurations.entry((record, configuration)),
                        inspection,
                    )?;
                }
                InspectionKey::Scoped(record, kind, role) => {
                    Self::merge(indexed.scoped.entry((record, kind, role)), inspection)?;
                }
                InspectionKey::Whole(record) => {
                    inspection.canonicalize_whole();
                    Self::merge(indexed.whole.entry(record), inspection)?;
                }
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
