// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::collections::hash_map::Entry;

use p4kx_dataforge::DataForge;

use crate::entities::{self, is_entity};
use crate::resolver::LoadoutResolver;
use crate::spec::EntitySpec;

#[derive(Clone, Copy)]
pub(crate) enum VehicleRetention {
    Resolver,
    Owner,
}

pub(crate) struct FleetVehicleClasses {
    unique: HashMap<Box<str>, Option<u32>>,
}

impl FleetVehicleClasses {
    fn read(forge: &DataForge<'_>) -> Self {
        let mut unique = HashMap::new();
        for record in forge.records().filter(is_entity) {
            let Ok(name) = record.name() else {
                continue;
            };
            let key = vehicle_cache_class(name)
                .to_ascii_lowercase()
                .into_boxed_str();
            match unique.entry(key) {
                Entry::Vacant(first) => {
                    first.insert(Some(record.index()));
                }
                Entry::Occupied(mut duplicate) => {
                    duplicate.insert(None);
                }
            }
        }
        Self { unique }
    }

    pub(crate) fn is_unique(&self, index: u32, cache_class: &str) -> bool {
        matches!(self.unique.get(cache_class), Some(Some(only)) if *only == index)
    }
}

pub(crate) fn vehicle_cache_class(name: &str) -> &str {
    entities::class_name_of(name).unwrap_or(name)
}

impl LoadoutResolver<'_> {
    /// Releases materialized vehicle payloads after their only possible fleet use.
    /// Supply each physical root at most once and stop resolution at docked vehicles.
    #[doc(hidden)]
    #[must_use]
    pub fn retiring_single_use_fleet_vehicles(mut self) -> Self {
        self.fleet_vehicle_classes = Some(FleetVehicleClasses::read(self.entities.forge()));
        self
    }

    pub(crate) const fn vehicle_retention(
        &self,
        spec: &EntitySpec,
        is_root: bool,
    ) -> VehicleRetention {
        if self.stop_at_docked && is_root && spec.docked_vehicle {
            VehicleRetention::Owner
        } else {
            VehicleRetention::Resolver
        }
    }
}

#[cfg(test)]
#[path = "vehicle_cache_lifetime_tests.rs"]
mod tests;
