// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use crate::merge::merge_by_port;
use crate::spec::{EntitySpec, EntrySpec, LoadoutSpec};
use crate::{LandingGear, LoadoutError, LoadoutResolver, PortSource, VehicleImplementation};

/// Resolved vehicle, landing gear and loadout inputs for a node.
pub(crate) struct NodeInputs {
    pub(crate) vehicle: Option<Arc<VehicleImplementation>>,
    pub(crate) landing_gear: Arc<[LandingGear]>,
    pub(crate) entries: Vec<(EntrySpec, PortSource)>,
}

impl LoadoutResolver<'_> {
    /// Resolves the node inputs while preserving the first observer stop.
    pub(crate) fn node_inputs(
        &mut self,
        spec: &EntitySpec,
        overrides: Option<&LoadoutSpec>,
        issues: &mut Vec<LoadoutError>,
    ) -> Option<NodeInputs> {
        let vehicle = self.vehicle_with_issues(spec, issues);
        if self.observation_stopped() {
            return None;
        }
        let landing_gear = self.landing_gear_of(spec, issues);
        if self.observation_stopped() {
            return None;
        }
        let entries = self.merged_entries(spec, overrides, issues);
        if self.observation_stopped() {
            return None;
        }
        Some(NodeInputs {
            vehicle,
            landing_gear,
            entries,
        })
    }

    /// Returns the vehicle implementation of `spec`, pushing its issues to `issues`.
    fn vehicle_with_issues(
        &mut self,
        spec: &EntitySpec,
        issues: &mut Vec<LoadoutError>,
    ) -> Option<Arc<VehicleImplementation>> {
        match self.vehicle_of(spec) {
            Some((Ok(vehicle), caller)) => {
                if vehicle.issues.iter().any(LoadoutError::is_missing_file) {
                    let caller = Arc::new(caller);
                    issues.extend(vehicle.issues.iter().cloned().map(|mut issue| {
                        issue.used_by(&caller);
                        issue
                    }));
                } else {
                    issues.extend(vehicle.issues.iter().cloned());
                }
                Some(vehicle)
            }
            Some((Err(mut error), caller)) => {
                if error.is_missing_file() {
                    error.used_by(&Arc::new(caller));
                }
                issues.push(error);
                None
            }
            None => None,
        }
    }

    /// Returns the entries of the Entity's own Loadout, each replaced in place by the entry of
    /// the same port in `overrides`, with the other overriding entries appended.
    fn merged_entries(
        &mut self,
        spec: &EntitySpec,
        overrides: Option<&LoadoutSpec>,
        issues: &mut Vec<LoadoutError>,
    ) -> Vec<(EntrySpec, PortSource)> {
        let mut entries = Vec::new();
        match self.entries(spec.loadout.as_ref()) {
            Ok(own) => {
                issues.extend(own.issues.iter().cloned());
                entries.extend(
                    own.specs()
                        .map(|entry| (entry.into_owned(), PortSource::Loadout)),
                );
            }
            Err(error) => issues.push(error),
        }
        if self.observation_stopped() {
            return entries;
        }
        if overrides.is_some() {
            match self.entries(overrides) {
                Ok(given) => {
                    issues.extend(given.issues.iter().cloned());
                    let given = given
                        .specs()
                        .map(|entry| (entry.into_owned(), PortSource::Override));
                    merge_by_port(&mut entries, given, |(entry, _)| &entry.port);
                }
                Err(error) => issues.push(error),
            }
        }
        entries
    }
}
