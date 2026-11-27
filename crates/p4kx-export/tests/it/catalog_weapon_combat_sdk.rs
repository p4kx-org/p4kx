// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::equipment::{
    AmmoSelection, CombatModel, DistanceUnit, WeaponCombatBase, WeaponCombatOwner,
};
use p4kx_export::{CatalogPublicationMode, build_catalog};
use p4kx_extraction::{DataForge, SourceValueState};
use serde_json::{Value, json};

use super::{COMBAT_ROWS, fixture};
use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::health_origin_assertions as proof;
use crate::sdk_full_proof as full_proof;

#[test]
fn combat_sdk_retains_nominal_values_and_the_original_ammo_roles() -> TestResult {
    let database = fixture::cross_owner_ammo(false);
    for origin in [DcbOrigin::Plain, DcbOrigin::Encrypted] {
        let encrypted = matches!(origin, DcbOrigin::Encrypted);
        compare(database.build(), origin, |owner| {
            verify_nominal(owner, encrypted)
        })?;
    }
    Ok(())
}

fn verify_nominal(owner: &WeaponCombatOwner<'_>, encrypted: bool) -> TestResult {
    let [primary, secondary] = owner.combat_bases.as_slice() else {
        return Err("the two original projectile modes".into());
    };
    let [primary_ammo, secondary_ammo] = owner.projectile_ammo.ammo_definitions.as_slice() else {
        return Err("both reached Ammo definitions".into());
    };
    for (mode, ammo, role) in [
        (primary, primary_ammo, AmmoSelection::Selected),
        (secondary, secondary_ammo, AmmoSelection::Dependency),
    ] {
        assert_eq!(
            ammo.selection(),
            role,
            "the unfiltered original admission policy"
        );
        assert_eq!(
            mode.ammunition_record_index()
                .and_then(|index| index.value()),
            ammo.source().record_index().value(),
            "each original mode retains its physical Ammo definition"
        );
    }
    verify_primary(primary, encrypted)?;
    let Some(CombatModel::NominalBullet { values }) = secondary.model().value() else {
        return Err("the secondary route still proves its Ammo/projectile".into());
    };
    let damage = values.damage().value().ok_or("secondary damage channels")?;
    assert_eq!(
        damage.nominal_dps().physical().state(),
        SourceValueState::Invalid,
        "an authored zero rate does not discard otherwise valid alpha or distance"
    );
    Ok(())
}

fn verify_primary(primary: &WeaponCombatBase<'_>, encrypted: bool) -> TestResult {
    let Some(CombatModel::NominalBullet { values }) = primary.model().value() else {
        return Err("the primary direct mode has nominal bullet metrics".into());
    };
    let damage = values
        .damage()
        .value()
        .ok_or("per-channel damage metrics")?;
    assert_eq!(
        (
            primary.formula_version(),
            primary.units().nominal_flight_distance()
        ),
        (1, DistanceUnit::Metres),
        "the typed combat contract has the canonical version and unit"
    );
    assert_eq!(
        (
            values.projectiles_per_discharge().value().copied(),
            values
                .nominal_flight_distance()
                .value()
                .copied()
                .map(f64::to_bits),
            damage.alpha().physical().value().copied().map(f64::to_bits),
            damage
                .nominal_dps()
                .physical()
                .value()
                .copied()
                .map(f64::to_bits),
        ),
        (
            Some(3),
            Some(25.0_f64.to_bits()),
            Some(3.0_f64.to_bits()),
            Some(6.0_f64.to_bits())
        ),
        "the typed values preserve the canonical source formula"
    );
    assert_eq!(
        damage
            .nominal_dps()
            .physical()
            .origin()
            .contains_encrypted_source(),
        Some(encrypted),
        "the damage metric carries its original source encryption"
    );
    Ok(())
}

#[test]
fn combat_sdk_preserves_non_projectile_and_unreadable_action_slots() -> TestResult {
    compare(
        fixture::build(fixture::Case::MixedModes).build(),
        DcbOrigin::Encrypted,
        |owner| {
            let [
                primary,
                secondary,
                missing,
                burst,
                beam,
                charge,
                conditional,
            ] = owner.combat_bases.as_slice()
            else {
                return Err("all seven original mode slots".into());
            };
            assert!(
                matches!(
                    primary.model().value(),
                    Some(CombatModel::AmmoSelectionUnavailable)
                ),
                "an incomplete action inventory prevents proving the primary route"
            );
            assert!(
                matches!(
                    secondary.model().value(),
                    Some(CombatModel::AmmoSelectionUnavailable)
                ),
                "an incomplete action inventory prevents proving the secondary route"
            );
            assert!(
                matches!(
                    missing.model().value(),
                    Some(CombatModel::ActionUnavailable)
                ),
                "the unreadable slot remains in its original position"
            );
            assert!(
                matches!(burst.model().value(), Some(CombatModel::Burst)),
                "burst is an explicit model outcome"
            );
            assert!(
                matches!(beam.model().value(), Some(CombatModel::Beam)),
                "beam is an explicit model outcome"
            );
            assert!(
                matches!(charge.model().value(), Some(CombatModel::Charge)),
                "charge is an explicit model outcome"
            );
            assert!(
                matches!(conditional.model().value(), Some(CombatModel::Conditional)),
                "conditional is an explicit model outcome"
            );
            Ok(())
        },
    )
}

#[test]
fn a_beam_only_owner_has_a_combat_row_without_a_projectile_dependency() -> TestResult {
    compare(
        fixture::build(fixture::Case::BeamOnly).build(),
        DcbOrigin::Plain,
        |owner| {
            let [beam] = owner.combat_bases.as_slice() else {
                return Err("the original beam mode must be retained".into());
            };
            assert!(
                matches!(beam.model().value(), Some(CombatModel::Beam)),
                "the beam model requires no projectile role"
            );
            assert!(
                owner.projectile_ammo.selections.is_empty()
                    && owner.projectile_ammo.ammo_definitions.is_empty(),
                "the SDK only reads Ammo proved by a projectile route"
            );
            Ok(())
        },
    )
}

#[test]
fn a_proved_combat_route_keeps_unreadable_ammunition_distinct_from_no_selection() -> TestResult {
    let mut bytes = fixture::build(fixture::Case::Unique).build();
    let secondary = physical(&bytes, fixture::SECONDARY)?;
    super::super::resolution::break_instance(&mut bytes, secondary)?;
    compare(bytes, DcbOrigin::Plain, |owner| {
        let [_, mode] = owner.combat_bases.as_slice() else {
            return Err("both modes survive the unreadable Ammo body".into());
        };
        assert!(
            matches!(
                mode.model().value(),
                Some(CombatModel::AmmoDefinitionUnavailable)
            ) && mode
                .ammunition_record_index()
                .and_then(|index| index.value())
                == Some(&secondary),
            "the physical Ammo route is proved independently of its body"
        );
        assert_eq!(
            owner.projectile_ammo.dependency_coverage.unreadable_bodies,
            [secondary],
            "the dependency read failure remains visible in the SDK result"
        );
        Ok(())
    })
}

fn compare(
    bytes: Vec<u8>,
    origin: DcbOrigin,
    inspect: impl FnOnce(&WeaponCombatOwner<'_>) -> TestResult,
) -> TestResult {
    let weapon = physical(&bytes, fixture::WEAPON)?;
    let (_folder, archive) = common::open_bytes(bytes, origin)?;
    let mut options = common::options();
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    let catalog: Value =
        serde_json::from_slice(build_catalog(&archive, options.with_full_provenance())?.json())?;
    full_proof::open(&archive, full_proof::NO_LANGUAGES, None)?.with_weapon_combat_bases(
        weapon,
        None,
        |snapshot| -> TestResult {
            let [owner] = snapshot.rows() else {
                return Err("one physical combat owner".into());
            };
            let scoped = json!({
                "projections": {"weaponCombatBases": {"rows": owner.combat_bases}},
                "provenance": snapshot.source_inventory(),
            });
            assert_eq!(
                proof::rows(&scoped, COMBAT_ROWS)?,
                proof::rows(&catalog, COMBAT_ROWS)?,
                "all original mode rows, values, states and source references match the Catalog"
            );
            proof::closure(&scoped)?;
            inspect(owner)
        },
    )?
}

fn physical(bytes: &[u8], guid: u128) -> TestResult<u32> {
    DataForge::open(bytes)?
        .record_by_guid(common::guid(guid).parse()?)
        .map(|record| record.index())
        .ok_or_else(|| "the original physical fixture Record".into())
}
