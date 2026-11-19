// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::equipment::{
    AlphaUnit, DistanceUnit, DpsUnit, FireRateUnit, WeaponCombatSnapshot,
    with_weapon_combat_bases_from,
};
use p4kx_extraction::{Archive, DataForgeEntry};

use super::original::Witness;
use super::{TestResult, compare_modes, proof, with_original_traces, with_sdk_samples};

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn weapon_combat_sdk_matches_original_sources() -> TestResult {
    with_sdk_samples(
        "combat",
        "original action slots, six independent damage channels and one flight distance",
        verify,
    )
}

fn verify(archive: &Archive, entry: &DataForgeEntry, witness: &Witness<'_>) -> TestResult {
    with_weapon_combat_bases_from(
        archive,
        entry,
        witness.owner.record_index(),
        None,
        |snapshot| {
            if snapshot.build() != archive.build() {
                return Err("the combat SDK must preserve the exact input Build".into());
            }
            inspect(&snapshot, entry, witness)
        },
    )
    .map_err(|_| "finish Archive-bound combat SDK read")?
}

fn inspect(
    snapshot: &WeaponCombatSnapshot<'_>,
    entry: &DataForgeEntry,
    witness: &Witness<'_>,
) -> TestResult {
    let [owner] = snapshot.rows() else {
        return Err("one physical weapon in the combat SDK result".into());
    };
    let coverage = snapshot.coverage();
    let ammo_index = witness.ammo.record_index();
    if !coverage.unreadable_headers.is_empty()
        || !coverage.unreadable_bodies.is_empty()
        || owner
            .projectile_ammo
            .dependency_coverage
            .unreadable_bodies
            .contains(&ammo_index)
    {
        return Err("the original combat source and selected Ammo must be readable".into());
    }
    let component = u32::try_from(witness.component)?;
    for row in &owner.combat_bases {
        let units = row.units();
        if row.owner().record_index().value() != Some(&witness.owner.record_index())
            || row.owner().component_ordinal().value() != Some(&component)
            || row.formula_version() != 1
            || units.fire_rate() != FireRateUnit::DischargesPerMinute
            || units.alpha() != AlphaUnit::DamagePoints
            || units.nominal_dps() != DpsUnit::DamagePointsPerSecond
            || units.nominal_flight_distance() != DistanceUnit::Metres
        {
            return Err(
                "the combat result must describe the original owner and formula contract".into(),
            );
        }
    }
    let rows = serde_json::to_value(&owner.combat_bases)?;
    let modes = rows.as_array().ok_or("serialized combat SDK rows")?;
    let projected: Vec<_> = modes.iter().collect();
    let routes = serde_json::to_value(&owner.projectile_ammo.selections)?;
    let routes = routes
        .as_array()
        .ok_or("serialized projectile route rows")?;
    let origin = serde_json::to_value(snapshot.source_inventory())?;
    let uses = proof::indexed(&origin, "/uses", "id")?;
    with_original_traces(entry, witness, &uses, |traces| {
        compare_modes(&projected, routes, witness, traces)
    })
}
