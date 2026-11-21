// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::hash_map::Entry;
use std::sync::Arc;

use p4kx_dataforge::{Record, RecordTree};

use crate::entities::is_entity;
use crate::loadout_file_cache::CachedDocument;
use crate::resolver::VehicleResult;
use crate::resolver_observation::{self, LoadoutReadEvent, Observation};
use crate::resolver_path_read;
use crate::spec::{EntitySpec, VehicleSource, read_vehicle_source, vehicle_component};
use crate::{
    LandingGear, LoadoutError, LoadoutRequest, LoadoutResolver, LoadoutTargetGuid,
    VehicleImplementation, vehicle,
};

#[derive(PartialEq, Eq, Hash)]
pub(crate) struct VehicleCacheKey {
    definition: String,
    modification: Arc<str>,
    class_name: String,
    capture_paint: bool,
}

impl<'f> LoadoutResolver<'f> {
    /// Reads only the vehicle implementation selected by the exact original Entity tree.
    ///
    /// Includes the selected Modification and its patch. Returns no implementation when the
    /// Entity provides no definition or the resolver has no vehicle file provider.
    ///
    /// # Errors
    /// Returns a Record ownership or type error, a vehicle read error, or an observer stop.
    pub fn read_vehicle_tree(
        &mut self,
        record: &Record<'f>,
        tree: &RecordTree<'f>,
    ) -> Result<Option<Arc<VehicleImplementation>>, LoadoutError> {
        if self.stopped() {
            return Err(LoadoutError::ObservationStopped);
        }
        if !record.is_from(self.entities.forge()) || !tree.is_from(record) {
            return Err(LoadoutError::UnreadableRecord {
                guid: LoadoutTargetGuid::unbound(record.guid()),
                reason: "the vehicle tree does not belong to the selected Record".to_owned(),
            });
        }
        if !is_entity(record) {
            return Err(LoadoutError::NotAnEntity {
                guid: LoadoutTargetGuid::unbound(record.guid()),
                type_name: tree.type_name().to_owned(),
            });
        }
        let Some((_, component)) = vehicle_component(tree) else {
            return Ok(None);
        };
        let observer = self.observation.as_deref();
        resolver_observation::notify(observer, LoadoutReadEvent::ProvidedTree { record, tree })?;
        let source = resolver_path_read::with_record_paths(record, tree, observer, |paths| {
            read_vehicle_source(component, tree, paths)
        })?;
        let result = self.vehicle_of(&source, tree.name(), Some(record.index()));
        if self.stopped() {
            return Err(LoadoutError::ObservationStopped);
        }
        result.map(|(read, _)| read).transpose()
    }

    /// Loads the selected implementation and reports its actual caller, including cache reuse.
    pub(crate) fn vehicle_of(
        &mut self,
        source: &VehicleSource,
        record_name: &str,
        record_index: Option<u32>,
    ) -> Option<(VehicleResult, LoadoutRequest)> {
        let files = self
            .files
            .filter(|files| files.reads_vehicle_implementations())?;
        let class_name = crate::entities::class_name_of(record_name).unwrap_or(record_name);
        if source.definition.is_empty() {
            return None;
        }
        let capture_paint = self.captures_vehicle_appearance();
        let key = VehicleCacheKey {
            definition: source.definition.to_ascii_lowercase(),
            modification: Arc::clone(source.modification.text()),
            class_name: class_name.to_ascii_lowercase(),
            capture_paint,
        };
        let caller = LoadoutRequest::Vehicle {
            definition: source.definition.clone(),
            modification: source.modification.clone(),
            class_name: source.class_name.clone(),
        };
        let observation = self.observation.as_deref();
        let loaded = match self.vehicles.entry(key) {
            Entry::Occupied(entry) => entry.into_mut().reused(observation, &caller),
            Entry::Vacant(entry) => Ok(entry
                .insert(CachedDocument::read(|trace| {
                    let capture = capture_paint.then(Observation::capture_vehicle_port_attributes);
                    let observation = observation.or(capture.as_ref());
                    vehicle::load_observed(files, class_name, &caller, observation, trace)
                        .map(Arc::new)
                }))
                .value()),
        };
        let result = loaded.map_or_else(Err, Clone::clone).map_err(|mut error| {
            if error.is_missing_file() {
                error.used_by(&Arc::new(caller.clone()));
            }
            error
        });
        if let (Ok(vehicle), Some(observer), Some(record)) = (&result, observation, record_index) {
            observer.vehicle_used(record, vehicle, &caller);
        }
        Some((result, caller))
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
                    self.remember_read(resolver_observation::Cache::LandingGear, record);
                    resolver_observation::canonical(
                        &mut self.builder,
                        &record,
                        self.observation.as_deref(),
                    )
                    .map(|tree| vehicle::gears(&tree))
                }
            };
            self.landing.insert(guid, read);
        } else if let Err(error) = self.reuse_read(resolver_observation::Cache::LandingGear, guid) {
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

#[cfg(test)]
#[path = "resolver_vehicle_read_tests.rs"]
mod tests;
