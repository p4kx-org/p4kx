// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{VehicleImplementation, VehicleTotals};
use p4kx_extraction::RecordTree;

use super::{
    Hull, Mass, MassBasis, PARTS_DAMAGE, PARTS_MASS, PHYSICS_MASS_FIELD, Source, SourceKind,
    SourceShare, VehicleKind, VehicleKindPath, partial, rounded, shares, thresholds, unreadable,
};

/// Measurements defined by the effective Vehicle and the original Physics fallback.
pub(crate) struct VehicleMeasurements {
    #[cfg(test)]
    pub(crate) path_kind: VehicleKindPath,
    pub(crate) kind: Option<VehicleKind>,
    pub(crate) mass: Option<Mass>,
    pub(crate) hull: Option<Hull>,
}

impl VehicleMeasurements {
    pub(crate) fn read(
        tree: &RecordTree<'_>,
        vehicle: Option<&VehicleImplementation>,
        totals: Option<&VehicleTotals>,
        physics_mass: impl FnOnce() -> Option<f32>,
    ) -> Self {
        let path_kind = VehicleKindPath::of(tree.record_path());
        Self {
            #[cfg(test)]
            path_kind,
            kind: path_kind.classify(tree, vehicle),
            mass: mass(tree, vehicle, totals, physics_mass),
            hull: vehicle
                .zip(totals)
                .and_then(|(vehicle, totals)| hull(vehicle, totals)),
        }
    }
}

fn mass(
    tree: &RecordTree<'_>,
    vehicle: Option<&VehicleImplementation>,
    totals: Option<&VehicleTotals>,
    physics_mass: impl FnOnce() -> Option<f32>,
) -> Option<Mass> {
    if let (Some(vehicle), Some(totals)) = (vehicle, totals)
        && totals.mass_parts() > 0
        && totals.mass() > 0.0
        && let kg = rounded(totals.mass())
        && kg.is_finite()
    {
        return Some(Mass {
            kg,
            basis: MassBasis::Parts,
            parts: totals.mass_parts(),
            sources: shares(vehicle, totals.mass_origins(), PARTS_MASS),
            unreadable: unreadable(totals, &["mass"]),
            partial: partial(totals, &["mass"]),
            cleaned: totals.cleaned_mass(),
        });
    }
    let kg = physics_mass().filter(|kg| *kg > 0.0)?;
    let kg = rounded(f64::from(kg));
    if !kg.is_finite() {
        return None;
    }
    Some(Mass {
        kg,
        basis: MassBasis::Physics,
        parts: 0,
        sources: vec![SourceShare {
            source: Source {
                kind: SourceKind::Record,
                file: tree.record_path().to_owned(),
                field: PHYSICS_MASS_FIELD.to_owned(),
            },
            values: 1,
        }],
        unreadable: totals.map_or_else(Vec::new, |totals| unreadable(totals, &["mass"])),
        partial: Vec::new(),
        cleaned: 0,
    })
}

fn hull(vehicle: &VehicleImplementation, totals: &VehicleTotals) -> Option<Hull> {
    if totals.health_parts() == 0 {
        return None;
    }
    let health = rounded(totals.health());
    if !health.is_finite() {
        return None;
    }
    Some(Hull {
        health,
        parts: totals.health_parts(),
        sources: shares(vehicle, totals.health_origins(), PARTS_DAMAGE),
        destruction: thresholds(totals.destruction()),
        detach: thresholds(totals.detach()),
        unreadable: unreadable(totals, &["damageMax", "detachRatio"]),
        partial: partial(totals, &["damageMax", "detachRatio"]),
        cleaned: totals.cleaned_damage(),
    })
}
