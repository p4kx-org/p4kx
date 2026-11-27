// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{VehicleImplementation, VehiclePortSelection};
use p4kx_extraction::{
    Archive, FieldOrigin, ProvenanceMode, SharedFieldOrigin, SourceUseRole, SourceUses,
    SourceValue, SourcedValue,
};

use super::super::vehicle_stage::Contributions;
use super::VehicleFiles;
use crate::catalog_read::original::AuthoredEntryPath;
use crate::{CatalogError, ExportAccountingError};

/// Reads port proofs for one physical Entity node.
pub(crate) struct VehiclePortRead<'node> {
    archive: Option<&'node Archive>,
    record_index: Option<u32>,
    vehicle: Option<&'node VehicleImplementation>,
    markers: MarkerPortPrefix,
}

impl<'node> VehiclePortRead<'node> {
    pub(crate) const fn new(
        archive: Option<&'node Archive>,
        record_index: Option<u32>,
        vehicle: Option<&'node VehicleImplementation>,
    ) -> Self {
        Self {
            archive,
            record_index,
            vehicle,
            markers: MarkerPortPrefix::new(),
        }
    }

    pub(super) const fn record_index(&self) -> Option<u32> {
        self.record_index
    }

    pub(crate) fn select(&self, name: &str) -> Option<VehiclePortSelection<'node>> {
        self.vehicle?.item_port_with_selection(name)
    }

    pub(super) fn proof(
        &mut self,
        files: &VehicleFiles<'_>,
        selected: &VehiclePortSelection<'_>,
        name: Option<&AuthoredEntryPath>,
        sources: &SourceUses,
    ) -> Result<PortSelectorOrigin, CatalogError> {
        let (inputs, known) = name.map_or_else(
            || Ok((Vec::new(), false)),
            AuthoredEntryPath::selector_inputs,
        )?;
        let mut proof = Contributions::new(inputs, known && selected.is_complete());
        let archive = self.archive;
        if sources.provenance_mode() == ProvenanceMode::Full {
            files.select_files(archive, &mut proof)?;
            for site in selected.selectors() {
                files.retain(site, SourceUseRole::Selector, &mut proof)?;
            }
            return Ok(PortSelectorOrigin::Full(proof.shared(sources)));
        }
        let prefix = self.markers.through(
            selected.selector_count(),
            sources,
            || {
                let mut choices = Contributions::default();
                files.select_files(archive, &mut choices)?;
                Ok(choices)
            },
            |index| {
                let site = selected
                    .selector(index)
                    .ok_or(ExportAccountingError::UnobservedFailure)?;
                let mut proof = Contributions::default();
                files.retain(site, SourceUseRole::Selector, &mut proof)?;
                Ok(proof)
            },
        )?;
        let name = proof.origin(sources);
        let combined = sources.origin_with_origins([prefix, &name], true)?;
        Ok(PortSelectorOrigin::Markers(combined))
    }
}

pub(super) enum PortSelectorOrigin {
    Full(SharedFieldOrigin),
    Markers(FieldOrigin),
}

impl PortSelectorOrigin {
    pub(super) fn value<T>(
        &self,
        sources: &SourceUses,
        value: SourceValue<T>,
        local: Contributions,
    ) -> Result<SourcedValue<T>, CatalogError> {
        let (uses, complete) = local.into_parts();
        match self {
            Self::Full(shared) => {
                Ok(sources.value_with_shared_origin(value, shared, &uses, complete)?)
            }
            Self::Markers(shared) => {
                Ok(sources.add_value_origins(sources.value(value, &uses, complete), &[shared])?)
            }
        }
    }
}

struct MarkerPortPrefix {
    observed: usize,
    changes: Vec<(usize, FieldOrigin)>,
}

impl MarkerPortPrefix {
    const fn new() -> Self {
        Self {
            observed: 0,
            changes: Vec::new(),
        }
    }

    fn through(
        &mut self,
        count: usize,
        sources: &SourceUses,
        choices: impl FnOnce() -> Result<Contributions, CatalogError>,
        mut site: impl FnMut(usize) -> Result<Contributions, CatalogError>,
    ) -> Result<&FieldOrigin, CatalogError> {
        if self.changes.is_empty() {
            self.record(0, choices()?.origin(sources))?;
        }
        while self.observed < count {
            let (_, previous) = self
                .changes
                .last()
                .ok_or(ExportAccountingError::UnobservedFailure)?;
            let local = site(self.observed)?.origin(sources);
            let marker = sources.origin_with_origins([previous, &local], true)?;
            let next = self.observed + 1;
            if marker.source_set_complete() != previous.source_set_complete()
                || marker.contains_encrypted_source() != previous.contains_encrypted_source()
            {
                // A prefix can lose completeness and gain uncertainty or encryption only once.
                self.record(next, marker)?;
            }
            self.observed = next;
        }
        self.changes
            .iter()
            .rev()
            .find(|(through, _)| *through <= count)
            .map(|(_, origin)| origin)
            .ok_or_else(|| ExportAccountingError::UnobservedFailure.into())
    }

    fn record(&mut self, through: usize, marker: FieldOrigin) -> Result<(), CatalogError> {
        self.changes
            .try_reserve(1)
            .map_err(CatalogError::SourceUseAllocation)?;
        self.changes.push((through, marker));
        Ok(())
    }
}

#[cfg(test)]
#[path = "vehicle_port_prefix_tests.rs"]
mod tests;
