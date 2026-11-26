// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::hash_map::Entry;
use std::sync::Arc;

use p4kx_dataforge::{DataForge, Record, RecordTree};

use crate::entities::is_entity;
use crate::loadout_file_cache::CachedDocument;
use crate::loadout_file_trace::FileTrace;

use crate::resolver::VehicleResult;
use crate::resolver_observation::{self, LoadoutReadEvent, Observation};
use crate::resolver_path_read;
use crate::spec::{EntitySpec, VehicleSource, read_vehicle_source, vehicle_component};
use crate::vehicle_cache_lifetime::{VehicleRetention, vehicle_cache_class};
use crate::{
    LandingGear, LoadoutError, LoadoutFiles, LoadoutReadObserver, LoadoutRequest, LoadoutResolver,
    LoadoutTargetGuid, VehicleImplementation, vehicle,
};

#[derive(PartialEq, Eq, Hash)]
pub(crate) struct VehicleCacheKey {
    definition: String,
    modification: Arc<str>,
    class_name: String,
    capture_paint: bool,
}

/// Reads one observed vehicle implementation from an exact original Entity Record and tree.
///
/// Returns the effective vehicle and selected Modification without constructing a fleet index
/// or retaining vehicle state for another Record.
///
/// # Errors
/// Returns a Record ownership or type error, a vehicle read error, or an observer stop.
pub fn read_observed_vehicle_tree<'source>(
    forge: &DataForge<'source>,
    record: &Record<'source>,
    tree: &RecordTree<'source>,
    files: &dyn LoadoutFiles,
    observer: Arc<dyn LoadoutReadObserver + '_>,
) -> Result<Option<Arc<VehicleImplementation>>, LoadoutError> {
    let observation = Observation::new(observer);
    let Some(source) = tree_vehicle_source(forge, record, tree, Some(&observation))? else {
        return Ok(None);
    };
    if source.definition.is_empty() || !files.reads_vehicle_implementations() {
        return Ok(None);
    }
    let name = tree.name();
    let class_name = vehicle_cache_class(name);
    let caller = vehicle_request(&source);
    let read = vehicle::load_observed(
        files,
        class_name,
        &caller,
        Some(&observation),
        &mut FileTrace::default(),
    )
    .map(Arc::new);
    let result = bind_vehicle_result(read, Some(&observation), &caller, Some(record.index()));
    if observation.stopped() {
        return Err(LoadoutError::ObservationStopped);
    }
    result.map(Some)
}

fn tree_vehicle_source<'source>(
    forge: &DataForge<'source>,
    record: &Record<'source>,
    tree: &RecordTree<'source>,
    observation: Option<&Observation<'_>>,
) -> Result<Option<VehicleSource>, LoadoutError> {
    if !record.is_from(forge) || !tree.is_from(record) {
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
    resolver_observation::notify(observation, LoadoutReadEvent::ProvidedTree { record, tree })?;
    resolver_path_read::with_record_paths(record, tree, observation, |paths| {
        read_vehicle_source(component, tree, paths)
    })
    .map(Some)
}

fn vehicle_request(source: &VehicleSource) -> LoadoutRequest {
    LoadoutRequest::Vehicle {
        definition: source.definition.clone(),
        modification: source.modification.clone(),
        class_name: source.class_name.clone(),
    }
}

fn bind_vehicle_result(
    result: VehicleResult,
    observation: Option<&Observation<'_>>,
    caller: &LoadoutRequest,
    record_index: Option<u32>,
) -> VehicleResult {
    let result = result.map_err(|mut error| {
        if error.is_missing_file() {
            error.used_by(&Arc::new(caller.clone()));
        }
        error
    });
    if let (Ok(vehicle), Some(observer), Some(record)) = (&result, observation, record_index) {
        observer.vehicle_used(record, vehicle, caller);
    }
    result
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
        let Some(source) = tree_vehicle_source(
            self.entities.forge(),
            record,
            tree,
            self.observation.as_deref(),
        )?
        else {
            return Ok(None);
        };
        let result = self.vehicle_of(
            &source,
            tree.name(),
            Some(record.index()),
            VehicleRetention::Resolver,
        );
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
        retention: VehicleRetention,
    ) -> Option<(VehicleResult, LoadoutRequest)> {
        let files = self
            .files
            .filter(|files| files.reads_vehicle_implementations())?;
        let class_name = vehicle_cache_class(record_name);
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
        let release = matches!(retention, VehicleRetention::Owner)
            && record_index
                .zip(self.fleet_vehicle_classes.as_ref())
                .is_some_and(|(index, classes)| classes.is_unique(index, &key.class_name));
        let caller = vehicle_request(source);
        let observation = self.observation.as_deref();
        let loaded = match self.vehicles.entry(key) {
            Entry::Occupied(entry) => entry
                .get()
                .reused(observation, &caller)
                .map_or_else(Err, Clone::clone),
            Entry::Vacant(entry) => {
                let document = CachedDocument::read(|trace| {
                    let capture = capture_paint.then(Observation::capture_vehicle_port_attributes);
                    let observation = observation.or(capture.as_ref());
                    vehicle::load_observed(files, class_name, &caller, observation, trace)
                        .map(Arc::new)
                });
                let value = document.value().clone();
                if !release || value.is_err() {
                    entry.insert(document);
                }
                value
            }
        };
        let result = bind_vehicle_result(loaded, observation, &caller, record_index);
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
