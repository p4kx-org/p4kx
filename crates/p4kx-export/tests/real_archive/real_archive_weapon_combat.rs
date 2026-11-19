// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::time::UNIX_EPOCH;

use p4kx_export::{CatalogOptions, CatalogPublicationMode, build_catalog_from};
use p4kx_extraction::{
    Archive, ArchiveWatch, DataForgeEntry, DataForgeOrigin, InputScope, RecordTree,
    SourceEncryption, SourceInput, SourceUses, Value as Original,
};
use p4kx_testkit::{real_archive_key, real_archive_pair};
use serde_json::Value;

use crate::real_archive_item_components::{cell, require_state};
use crate::real_archive_item_durability::{self as proof, Trace};
use crate::real_archive_shop_inventory_v3::verify_build;
use crate::real_archive_verification;

#[path = "weapon_combat_dependencies.rs"]
mod dependencies;
#[path = "weapon_combat_original.rs"]
pub(super) mod original;
#[path = "weapon_combat_projectile_proof.rs"]
mod projectile;

use original::{OriginalAction, OriginalAmmo, Witness};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn weapon_combat_base_matches_original_action_and_ammunition_inputs() -> TestResult {
    let Some((first, second)) = real_archive_pair()? else {
        return Ok(());
    };
    let key = real_archive_key().map_err(|_| "configured Archive key")?;
    for (sample, path) in [(1, first), (2, second)] {
        let archive =
            Archive::open_with_key(path, Some(key.clone())).map_err(|_| "open input Archive")?;
        with_original_witness(&archive, compare)?;
        real_archive_verification::print_line(&format!(
            "weapon combat source witness {sample}: one original mode, six independent channels, one flight distance"
        ));
    }
    Ok(())
}

fn with_original_witness(
    archive: &Archive,
    inspect: impl FnOnce(&Archive, &DataForgeEntry, &Witness<'_>) -> TestResult,
) -> TestResult {
    let watch = ArchiveWatch::new(archive);
    let entry = DataForgeEntry::read(archive).map_err(|_| "read actual DataForge")?;
    let forge = entry.open().map_err(|_| "open actual DataForge")?;
    let witness = original::find(&forge).ok_or("bounded combat owner plus Ammo definition")?;
    watch
        .check()
        .map_err(|_| "Archive changed during source selection")?;
    inspect(archive, &entry, &witness)?;
    watch
        .check()
        .map_err(|_| "Archive changed during weapon verification".into())
}

fn compare(archive: &Archive, entry: &DataForgeEntry, witness: &Witness<'_>) -> TestResult {
    let options = CatalogOptions {
        filter: Some(&witness.filter),
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..CatalogOptions::new(UNIX_EPOCH)
    };
    let built =
        build_catalog_from(archive, entry, &options).map_err(|_| "build bounded combat Catalog")?;
    let catalog: Value =
        serde_json::from_slice(built.json()).map_err(|_| "read bounded Catalog JSON")?;
    verify_build(archive, &catalog)?;
    dependencies::check(&catalog, witness)?;
    let owner = &witness.owner;
    let ammo = &witness.ammo;
    let uses = proof::indexed(&catalog, "/provenance/uses", "id")?;
    let DataForgeOrigin::Archive(origin) = entry.origin() else {
        return Err("combat DataForge must have Archive origin".into());
    };
    let encrypted = match entry.origin().encryption() {
        SourceEncryption::Encrypted => Some(true),
        SourceEncryption::Unencrypted => Some(false),
        _ => None,
    };
    let sources = SourceUses::new();
    let scope = sources.observe(SourceInput::Archive(origin), InputScope::Payload)?;
    let traces = Traces {
        owner: Trace::of(owner, &uses, &scope, encrypted),
        ammo: Trace::of(ammo, &uses, &scope, encrypted),
    };
    compare_modes(&catalog, owner, ammo, witness, &traces)
}

struct Traces<'read, 'source> {
    owner: Trace<'read, 'source>,
    ammo: Trace<'read, 'source>,
}

fn compare_modes<'tree, 'source>(
    catalog: &Value,
    owner: &'tree RecordTree<'source>,
    ammo: &'tree RecordTree<'source>,
    witness: &Witness<'source>,
    traces: &Traces<'_, 'source>,
) -> TestResult {
    let (_, owner_weapon) =
        original::unique(owner, original::WEAPON).ok_or("original weapon component")?;
    let weapon = original::structure(owner_weapon).ok_or("read original weapon component")?;
    let actions_raw = weapon
        .field("fireActions")
        .ok_or("original fireActions field")?;
    let actions = original::array(actions_raw).ok_or("original fireActions array")?;
    let projected = modes(catalog, witness)?;
    assert!(
        projected.len() == actions.len(),
        "one row per actual top-level slot"
    );
    for (index, (original, row)) in actions.iter().zip(projected).enumerate() {
        assert!(
            proof::number(row, "/modeIndex/value")? == u64::try_from(index)?,
            "physical mode order survives"
        );
        let position = cell(row, "modeIndex")?;
        traces.owner.check(position, actions_raw, "selector")?;
        let part = if original::structure(original).is_some() {
            "$type"
        } else {
            "$value"
        };
        traces.owner.part(position, original, part, "selector")?;
        if let Some(expected) = fixed_outcome(original) {
            assert_eq!(
                proof::text(row, "/model/value/type")?,
                expected,
                "unavailable mode classification"
            );
        }
        if index == witness.mode {
            compare_nominal(catalog, row, traces, witness, original, ammo)?;
        }
    }
    Ok(())
}

fn fixed_outcome(raw: &Original<'_>) -> Option<&'static str> {
    let Some(body) = original::structure(raw) else {
        return Some("actionUnavailable");
    };
    match body.type_name() {
        "SWeaponActionFireBurstParams" => Some("burst"),
        "SWeaponActionFireBeamParams" => Some("beam"),
        "SWeaponActionFireChargedParams" => Some("charge"),
        "SWeaponActionConditionParams" | "SWeaponActionDynamicConditionParams" => {
            Some("conditional")
        }
        "SWeaponActionSequenceParams"
        | "SWeaponActionParallelParams"
        | "SWeaponActionToggleParams" => Some("unsupportedAction"),
        _ => None,
    }
}

fn modes<'a>(catalog: &'a Value, witness: &Witness<'_>) -> TestResult<Vec<&'a Value>> {
    let mut selected = Vec::new();
    for row in proof::rows(catalog, "/projections/weaponCombatBases/rows")? {
        if proof::number(row, "/ownerRecordIndex/value")? == u64::from(witness.owner.record_index())
            && proof::number(row, "/componentOrdinal/value")? == u64::try_from(witness.component)?
        {
            selected.push(row);
        }
    }
    Ok(selected)
}

fn compare_nominal<'source>(
    catalog: &Value,
    row: &Value,
    traces: &Traces<'_, 'source>,
    witness: &Witness<'source>,
    raw: &Original<'source>,
    ammo: &RecordTree<'source>,
) -> TestResult {
    let action = original::action_inputs(raw).ok_or("exact owned action inputs")?;
    let ammo = original::ammo_inputs(ammo).ok_or("exact selected generic Bullet inputs")?;
    assert_eq!(
        proof::text(row, "/model/value/type")?,
        "nominalBullet",
        "admitted physical bullet mode"
    );
    assert_eq!(
        proof::text(row, "/actionSourceType/value")?,
        witness.source_type,
        "original action type"
    );
    let ordinal = traces.owner.ordinal(action.action)?.to_string();
    assert!(
        proof::text(row, "/actionSourceValueOrdinal/value")? == ordinal,
        "original action ordinal"
    );
    traces.owner.check(
        cell(row, "actionSourceValueOrdinal")?,
        action.action,
        "conversion",
    )?;
    assert!(
        proof::number(row, "/ammunitionRecordIndex/value")?
            == u64::from(witness.ammo.record_index()),
        "actual resolved ammo Record position"
    );
    let route = projectile::route_row(catalog, witness, &ordinal)?;
    projectile::check_original_route(route, witness, &action, &traces.owner)?;
    let result = row.pointer("/model/value/values").ok_or("mode values")?;
    compare_direct_inputs(result, &action, traces)?;
    let distance = cell(result, "nominalFlightDistance")?;
    assert_value(
        distance,
        original::single(ammo.speed)
            .zip(original::single(ammo.lifetime))
            .map(|(speed, lifetime)| speed * lifetime),
    )?;
    traces.ammo.check(distance, ammo.speed, "payload")?;
    traces.ammo.check(distance, ammo.lifetime, "payload")?;
    compare_channels(result, &action, &ammo, traces)
}

fn compare_direct_inputs<'source>(
    values: &Value,
    action: &OriginalAction<'_, 'source>,
    traces: &Traces<'_, 'source>,
) -> TestResult {
    let rate = cell(values, "fireRateRpm")?;
    let pellets = cell(values, "projectilesPerDischarge")?;
    let multiplier = cell(values, "launcherDamageMultiplier")?;
    assert_value(rate, Some(f64::from(original::positive(action.rate))))?;
    assert_value(pellets, Some(f64::from(original::positive(action.pellets))))?;
    let expected = original::single(action.multiplier).ok_or("original multiplier")?;
    let Original::Single(multiplier_bits) = action.multiplier else {
        return Err("original launcher multiplier is a Single".into());
    };
    assert!(
        proof::text(multiplier, "/value/rawBits")? == format!("{:08x}", multiplier_bits.to_bits()),
        "the exact original launcher multiplier word survives"
    );
    assert_single_finite(multiplier, expected)?;
    traces.owner.check(rate, action.rate, "payload")?;
    traces.owner.check(pellets, action.pellets, "payload")?;
    traces
        .owner
        .check(multiplier, action.multiplier, "payload")?;
    Ok(())
}

fn compare_channels<'source>(
    values: &Value,
    action: &OriginalAction<'_, 'source>,
    ammo: &OriginalAmmo<'_, 'source>,
    traces: &Traces<'_, 'source>,
) -> TestResult {
    let damage = values
        .pointer("/damage/value")
        .ok_or("sourced typed damage channels")?;
    let alpha = cell(damage, "alpha")?;
    let dps = cell(damage, "nominalDps")?;
    for &(name, key) in original::CHANNELS {
        let alpha = cell(alpha, key)?;
        let dps = cell(dps, key)?;
        if let Some(raw) = ammo.channels.field(name) {
            channel(raw, alpha, dps, action, ammo, traces)?;
        } else {
            require_state(alpha, "absent")?;
            require_state(dps, "absent")?;
        }
    }
    Ok(())
}

fn channel<'source>(
    raw: &Original<'source>,
    alpha: &Value,
    dps: &Value,
    action: &OriginalAction<'_, 'source>,
    ammo: &OriginalAmmo<'_, 'source>,
    traces: &Traces<'_, 'source>,
) -> TestResult {
    let expected = original::single(raw)
        .zip(original::single(action.multiplier))
        .map(|(damage, multiplier)| {
            damage * f64::from(original::positive(action.pellets)) * multiplier
        });
    if expected.is_none() {
        let state = match raw {
            Original::Null => "null",
            Original::Error(_) => "unreadable",
            _ => "invalid",
        };
        require_state(alpha, state)?;
        return require_state(dps, state);
    }
    assert_value(alpha, expected)?;
    assert_value(
        dps,
        expected.map(|alpha| alpha * f64::from(original::positive(action.rate)) / 60.0),
    )?;
    for value in [alpha, dps] {
        traces.ammo.check(value, raw, "payload")?;
        traces.owner.check(value, action.pellets, "payload")?;
        traces.owner.check(value, action.multiplier, "payload")?;
        traces.owner.check(value, action.role, "payload")?;
        traces
            .ammo
            .part(value, ammo.projectile, "$type", "selector")?;
        traces.ammo.part(value, ammo.damage, "$type", "selector")?;
    }
    traces.owner.check(dps, action.rate, "payload")?;
    Ok(())
}

fn assert_value(cell: &Value, expected: Option<f64>) -> TestResult {
    if let Some(expected) = expected {
        require_state(cell, "present")?;
        assert_exact(cell.get("value").and_then(Value::as_f64), expected);
        Ok(())
    } else {
        require_state(cell, "invalid")
    }
}

fn assert_single_finite(cell: &Value, expected: f64) -> TestResult {
    let number = cell.pointer("/value/finite").ok_or("projected Single")?;
    let value = <f32 as serde::Deserialize>::deserialize(number)?;
    assert_exact(Some(f64::from(value)), expected);
    Ok(())
}

fn assert_exact(actual: Option<f64>, expected: f64) {
    assert!(
        actual
            .is_some_and(|value| (value - expected).abs() <= 1e-12_f64.max(1e-12 * expected.abs())),
        "modeled value matches its exact original source calculation"
    );
}
