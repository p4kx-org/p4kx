// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use crate::resolver::VehicleResult;
use crate::spec::EntitySpec;
use crate::{LandingGear, LoadoutError, LoadoutResolver, vehicle};

impl LoadoutResolver<'_> {
    /// Returns the vehicle implementation `spec` names with its Modification, read once per
    /// resolver, or `None` when the Entity names none, its `vehicleDefinition` is empty, or no
    /// files were given. The class-name fallback applies to a definition the Archive lacks, not
    /// to an empty one.
    pub(crate) fn vehicle_of(&mut self, spec: &EntitySpec) -> Option<VehicleResult> {
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
        let loaded = self.vehicles.entry(key).or_insert_with(|| {
            vehicle::load_observed(
                files,
                class_name,
                &vehicle_spec.definition,
                &vehicle_spec.modification,
                self.observation.as_deref(),
            )
            .map(Arc::new)
        });
        Some(loaded.clone())
    }

    /// Returns the gears of the landing gear Record the vehicle component of `spec` names, read
    /// once per resolver; empty when it names none, and then also when the Record is missing or
    /// cannot be read, which is pushed to `issues`.
    pub(crate) fn landing_gear_of(
        &mut self,
        spec: &EntitySpec,
        issues: &mut Vec<LoadoutError>,
    ) -> Arc<[LandingGear]> {
        let Some(guid) = spec
            .vehicle
            .as_ref()
            .and_then(|vehicle| vehicle.landing_system)
        else {
            return Arc::from([]);
        };
        if !self.landing.contains_key(&guid) {
            let read = match self.entities.forge().record_by_guid(guid) {
                None => Err(LoadoutError::DanglingReference { guid }),
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
                issues.push(error.clone());
                Arc::from([])
            }
            None => Arc::from([]),
        }
    }
}
