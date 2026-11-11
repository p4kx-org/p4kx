// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::LoadoutTargetGuid;
use std::collections::hash_map::Entry;
use std::sync::Arc;

use crate::loadout_file_cache::CachedDocument;

use crate::resolver::VehicleResult;
use crate::spec::EntitySpec;
use crate::{LandingGear, LoadoutError, LoadoutRequest, LoadoutResolver, vehicle};

impl LoadoutResolver<'_> {
    /// Returns the vehicle implementation `spec` names with its Modification, read once per
    /// resolver, or `None` when the Entity names none, its `vehicleDefinition` is empty, or no
    /// files were given. The class-name fallback applies to a definition the Archive lacks, not
    /// to an empty one.
    pub(crate) fn vehicle_of(
        &mut self,
        spec: &EntitySpec,
    ) -> Option<(VehicleResult, LoadoutRequest)> {
        let vehicle_spec = spec.vehicle.as_ref()?;
        let files = self.files?;
        let class_name = crate::entities::class_name_of(&spec.name).unwrap_or(&spec.name);
        if vehicle_spec.definition.is_empty() {
            return None;
        }
        let key = format!(
            "{}\0{}\0{}",
            vehicle_spec.definition.to_ascii_lowercase(),
            vehicle_spec.modification.to_ascii_lowercase(),
            class_name.to_ascii_lowercase()
        );
        let caller = LoadoutRequest::Vehicle {
            definition: vehicle_spec.definition.clone(),
            modification: vehicle_spec.modification.clone(),
            class_name: vehicle_spec.class_name.clone(),
        };
        let observation = self.observation.as_deref();
        let loaded = match self.vehicles.entry(key) {
            Entry::Occupied(entry) => entry.into_mut().reused(observation, &caller),
            Entry::Vacant(entry) => Ok(entry
                .insert(CachedDocument::read(|trace| {
                    vehicle::load_observed(files, class_name, &caller, observation, trace)
                        .map(Arc::new)
                }))
                .value()),
        };
        Some((loaded.map_or_else(Err, Clone::clone), caller))
    }

    /// Returns the gears of the landing gear Record the vehicle component of `spec` names, read
    /// once per resolver; empty when it names none, and then also when the Record is missing or
    /// cannot be read, which is pushed to `issues`.
    pub(crate) fn landing_gear_of(
        &mut self,
        spec: &EntitySpec,
        issues: &mut Vec<LoadoutError>,
    ) -> Arc<[LandingGear]> {
        let Some(reference) = spec
            .vehicle
            .as_ref()
            .and_then(|vehicle| vehicle.landing_system.as_ref())
        else {
            return Arc::from([]);
        };
        let guid = reference.guid();
        if !self.landing.contains_key(&guid) {
            let read = match self.entities.forge().record_by_guid(guid) {
                None => Err(LoadoutError::DanglingReference {
                    guid: LoadoutTargetGuid::unbound(guid),
                }),
                Some(record) => {
                    self.remember_read(crate::resolver_observation::Cache::LandingGear, record);
                    crate::resolver_observation::canonical(
                        &mut self.builder,
                        &record,
                        self.observation.as_deref(),
                    )
                    .map(|tree| vehicle::gears(&tree))
                }
            };
            self.landing.insert(guid, read);
        } else if let Err(error) =
            self.reuse_read(crate::resolver_observation::Cache::LandingGear, guid)
        {
            issues.push(error);
            return Arc::from([]);
        }
        match self.landing.get(&guid) {
            Some(Ok(gears)) => Arc::clone(gears),
            Some(Err(error)) => {
                issues.push(error.clone().with_reference(reference));
                Arc::from([])
            }
            None => Arc::from([]),
        }
    }
}
