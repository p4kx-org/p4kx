// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::ptr;

use anyhow::Result;
use p4kx_assembly::{
    EntityNode, LoadoutDocumentKind, LoadoutFileObservation, LoadoutPath, LoadoutRequest,
    LoadoutSelection, ValueOrigin, VehicleImplementation,
};
use p4kx_extraction::{FieldOrigin, InputScope, SourceInput, SourceLocation, SourceUseRole};

use super::Output;

#[derive(Default)]
pub(super) struct VehicleFacts {
    callers: HashMap<(u32, usize), LoadoutRequest>,
    reads: HashMap<LoadoutSelection, Vec<Read>>,
}

struct Read {
    file: LoadoutFileObservation,
    request: LoadoutPath,
    reused: bool,
}

const fn key(caller: &LoadoutRequest) -> Option<&LoadoutSelection> {
    let LoadoutRequest::Vehicle { definition, .. } = caller else {
        return None;
    };
    definition.selection()
}

impl VehicleFacts {
    pub(super) fn use_vehicle(
        &mut self,
        record: u32,
        vehicle: &VehicleImplementation,
        caller: &LoadoutRequest,
    ) {
        self.callers
            .insert((record, ptr::from_ref(vehicle).addr()), caller.clone());
    }

    pub(super) fn read(
        &mut self,
        file: &LoadoutFileObservation,
        request: &LoadoutPath,
        caller: &LoadoutRequest,
        reused: bool,
    ) {
        let Some(key) = key(caller) else { return };
        let reads = self.reads.entry(key.clone()).or_default();
        if reads.iter().any(|read| read.file.is_same_operation(file)) {
            return;
        }
        reads.push(Read {
            file: file.clone(),
            request: request.clone(),
            reused,
        });
    }

    fn current(
        &self,
        entity: &EntityNode,
        vehicle: &VehicleImplementation,
    ) -> Option<(&LoadoutRequest, &[Read])> {
        let caller = self
            .callers
            .get(&(entity.record_index()?, ptr::from_ref(vehicle).addr()))?;
        let reads = self.reads.get(key(caller)?).map_or(&[][..], Vec::as_slice);
        Some((caller, reads))
    }
}

impl Output<'_, '_> {
    pub(super) fn vehicle_selection(
        &mut self,
        entity: &EntityNode,
        address: &str,
    ) -> Result<Option<FieldOrigin>> {
        let Some(vehicle) = entity.vehicle() else {
            return Ok(None);
        };
        let selected = self.facts.vehicles.current(entity, vehicle);
        let conversion = SourceUseRole::Conversion;
        let base = self.vehicle_path(vehicle, selected, ValueOrigin::Base, conversion)?;
        let patch_file = self.vehicle_path(vehicle, selected, ValueOrigin::Patch, conversion)?;
        let patch = if vehicle.patch_path().is_some() {
            self.uses.origin_with_origins([&base, &patch_file], false)?
        } else {
            patch_file
        };
        let modification = self.vehicle_modification(vehicle, selected, conversion)?;
        let selectors = [
            self.vehicle_path(
                vehicle,
                selected,
                ValueOrigin::Base,
                SourceUseRole::Selector,
            )?,
            self.vehicle_path(
                vehicle,
                selected,
                ValueOrigin::Patch,
                SourceUseRole::Selector,
            )?,
            self.vehicle_modification(vehicle, selected, SourceUseRole::Selector)?,
        ];
        let origin = self.uses.origin_with_origins(&selectors, false)?;
        for (name, value) in [
            ("path", base),
            ("patch_path", patch),
            ("modification", modification),
        ] {
            self.put(&format!("{address}/vehicle/{name}"), value)?;
        }
        Ok(Some(origin))
    }

    fn vehicle_modification(
        &self,
        vehicle: &VehicleImplementation,
        chosen: Option<(&LoadoutRequest, &[Read])>,
        role: SourceUseRole,
    ) -> Result<FieldOrigin> {
        let Some((LoadoutRequest::Vehicle { modification, .. }, _)) = chosen else {
            return Ok(self.uses.origin(&[], false));
        };
        let observed = self.path_origin(Some(modification), role)?;
        let matches = (!modification.value().is_empty()).then_some(modification.value())
            == vehicle.modification();
        Ok(self.uses.origin_with_origins([&observed], matches)?)
    }

    fn vehicle_path(
        &self,
        vehicle: &VehicleImplementation,
        chosen: Option<(&LoadoutRequest, &[Read])>,
        kind: ValueOrigin,
        role: SourceUseRole,
    ) -> Result<FieldOrigin> {
        let (Some((caller, reads)), Some(path), Some(entry)) = (
            chosen,
            vehicle.source_path(kind),
            vehicle.declaration_origin(kind),
        ) else {
            return Ok(self.uses.origin(&[], false));
        };
        let document = match kind {
            ValueOrigin::Patch => LoadoutDocumentKind::VehiclePatch,
            ValueOrigin::Base | ValueOrigin::Elem => LoadoutDocumentKind::VehicleImplementation,
        };
        let mut inputs = Vec::new();
        let mut first = true;
        for read in reads.iter().filter(|read| read.file.kind() == document) {
            let selected = read.request.value() == path
                && read.file.read_outcome().is_ok()
                && read.file.projection_outcome() == Some(Ok(()))
                && read
                    .file
                    .source()
                    .entry_origin()
                    .is_some_and(|source| source.binding().is_same_entry(&entry.binding()));
            if selected {
                let request = self.path_origin(Some(&read.request), role)?;
                let directory = self.vehicle_directory(&read.file)?;
                inputs.extend([request, directory]);
                if read.reused {
                    self.vehicle_alias(caller, &mut inputs)?;
                }
                return Ok(self.uses.origin_with_origins(&inputs, first)?);
            }
            inputs.push(self.path_origin(Some(&read.request), SourceUseRole::Selector)?);
            first = false;
        }
        Ok(self.uses.origin_with_origins(&inputs, false)?)
    }

    fn vehicle_alias(&self, caller: &LoadoutRequest, inputs: &mut Vec<FieldOrigin>) -> Result<()> {
        let LoadoutRequest::Vehicle {
            definition,
            modification,
            class_name,
        } = caller
        else {
            return Ok(());
        };
        for actual in [Some(definition), Some(modification), class_name.as_ref()] {
            inputs.push(self.path_origin(actual, SourceUseRole::Selector)?);
        }
        Ok(())
    }

    fn vehicle_directory(&self, file: &LoadoutFileObservation) -> Result<FieldOrigin> {
        let Some(entry) = file.source().entry_origin() else {
            return Ok(self.uses.origin(&[], false));
        };
        let scope = self.uses.observe(
            SourceInput::Archive(entry),
            InputScope::ArchiveDirectoryMetadata,
        )?;
        let matched = scope.at(
            SourceLocation::Metadata {
                field: "path".to_owned(),
            },
            SourceUseRole::Selector,
        )?;
        Ok(self.uses.origin(
            &[matched],
            entry.ancestors().all(|ancestor| ancestor.is_some()),
        ))
    }
}
