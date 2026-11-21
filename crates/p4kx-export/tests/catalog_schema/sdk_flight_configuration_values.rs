// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::ResourceFloat32;
use p4kx_export::catalog_queries::equipment::flight::{FlightController, Ifcs, Thruster};
use p4kx_export::catalog_queries::equipment::{ComponentOwner, EquipmentSnapshot};
use p4kx_extraction::{Language, SourceValueState, SourcedValue};

use super::{TestResult, fixture};

pub(super) fn controllers(
    snapshot: &EquipmentSnapshot<'_, FlightController<'_>>,
) -> TestResult<()> {
    let [first, second] = snapshot.rows() else {
        return Err("two physical flight-controller rows".into());
    };
    assert_eq!(
        first.owner().guid().value(),
        second.owner().guid().value(),
        "a shared GUID keeps both physical controllers"
    );
    assert_eq!(
        owner_position(first.owner()),
        Some((0, 0)),
        "the first controller retains its original owner and Components ordinal"
    );
    assert_eq!(
        owner_position(second.owner()),
        Some((1, 1)),
        "the aliased controller retains its own owner and Components ordinal"
    );
    assert_eq!(
        first.source().source_type().value(),
        Some(&fixture::CONTROLLER),
        "the typed row identifies the original controller declaration"
    );
    controller_values(first)?;
    controller_origin(snapshot, &first.settings.fuel_warning_display_time)
}

fn controller_values(controller: &FlightController<'_>) -> TestResult<()> {
    assert_eq!(
        controller.interactions.disable_esp.state(),
        SourceValueState::Null,
        "the declared empty interaction remains null"
    );
    assert_eq!(
        controller.interactions.enable_esp.state(),
        SourceValueState::Absent,
        "an undeclared interaction remains absent"
    );
    assert_eq!(
        controller
            .settings
            .power_on_effect_tag
            .value()
            .and_then(|reference| reference.resolved_index()),
        Some(3),
        "the power-on effect preserves its physical target index"
    );
    let collision = controller
        .collision_detection
        .value()
        .ok_or("original collision settings")?;
    assert_eq!(
        collision
            .collision_warn_speed
            .value()
            .and_then(ResourceFloat32::finite),
        Some(3.0),
        "the nested authored warning speed keeps its value"
    );
    assert_eq!(
        controller
            .settings
            .fuel_warning_display_time
            .value()
            .map(ResourceFloat32::raw_bits),
        Some(0),
        "the authored warning duration retains positive zero bits"
    );
    Ok(())
}

fn controller_origin(
    snapshot: &EquipmentSnapshot<'_, FlightController<'_>>,
    warning: &SourcedValue<ResourceFloat32>,
) -> TestResult<()> {
    assert_eq!(
        snapshot.requested_languages(),
        &[] as &[Language],
        "controller queries require no requested translations"
    );
    assert_eq!(
        snapshot.available_languages(),
        &[] as &[Language],
        "controller queries read no translation payloads"
    );
    let origin = warning.origin();
    assert_eq!(
        origin.contains_encrypted_source(),
        Some(true),
        "the warning duration retains its encrypted DataForge origin"
    );
    assert!(
        origin.source_set_complete(),
        "the warning duration has complete source evidence"
    );
    let uses = origin.source_uses_refs();
    uses.first()
        .ok_or("the warning duration has a source contribution")?;
    for reference in uses {
        assert!(
            snapshot
                .source_inventory()
                .iter_uses()
                .any(|entry| entry.id() == reference),
            "each origin reference resolves within the same snapshot"
        );
    }
    Ok(())
}

pub(super) fn ifcs(snapshot: &EquipmentSnapshot<'_, Ifcs<'_>>) -> TestResult<()> {
    let [first, second] = snapshot.rows() else {
        return Err("two physical IFCS rows".into());
    };
    assert_eq!(
        owner_position(first.owner()),
        Some((0, 2)),
        "the primary IFCS keeps its original ordinal after a null slot"
    );
    assert_eq!(
        owner_position(second.owner()),
        Some((1, 3)),
        "the alias IFCS keeps its own physical owner and ordinal"
    );
    assert_eq!(
        first.source().source_type().value(),
        Some(&fixture::IFCS),
        "the typed row identifies the original IFCS declaration"
    );
    assert_eq!(
        first
            .limits
            .scm_speed
            .value()
            .map(ResourceFloat32::raw_bits),
        Some(0x8000_0000),
        "SCM speed preserves its original negative zero bits"
    );
    let max = first
        .limits
        .max_speed
        .value()
        .ok_or("present non-finite maximum")?;
    assert_eq!(
        max.raw_bits(),
        fixture::RAW_NAN,
        "maximum speed preserves the original NaN payload bits"
    );
    assert_eq!(
        max.finite(),
        None,
        "a non-finite maximum has no finite projection"
    );
    assert_eq!(
        first.limits.boost_speed_forward.state(),
        SourceValueState::Absent,
        "an undeclared boost speed remains absent"
    );
    afterburners(first)
}

fn afterburners(ifcs: &Ifcs<'_>) -> TestResult<()> {
    let old = ifcs
        .dynamics
        .afterburner
        .value()
        .ok_or("original afterburner")?;
    let new = ifcs
        .dynamics
        .afterburner_new
        .value()
        .ok_or("new afterburner")?;
    assert_eq!(
        old.capacitor.max.value().and_then(ResourceFloat32::finite),
        Some(10.0),
        "the original afterburner has its own capacitor setting"
    );
    assert_eq!(
        new.capacitor.max.value().and_then(ResourceFloat32::finite),
        Some(20.0),
        "the newer afterburner retains its independent capacitor setting"
    );
    let curve = old
        .timing
        .ramp_mapping
        .value()
        .ok_or("original ordered ramp")?;
    let points = curve.points().value().ok_or("original points")?;
    let second = points
        .get(1)
        .and_then(SourcedValue::value)
        .ok_or("second original point")?;
    assert_eq!(
        second.x().value().and_then(ResourceFloat32::finite),
        Some(0.75),
        "the original ramp retains the second authored point in order"
    );
    Ok(())
}

pub(super) fn thrusters(snapshot: &EquipmentSnapshot<'_, Thruster<'_>>) -> TestResult<()> {
    let [main, maneuver, alias] = snapshot.rows() else {
        return Err("three physical thruster rows".into());
    };
    assert_eq!(
        [
            owner_position(main.owner()),
            owner_position(maneuver.owner()),
            owner_position(alias.owner()),
        ],
        [Some((0, 3)), Some((0, 4)), Some((1, 0))],
        "both original primary thrusters and the alias remain in physical order"
    );
    assert_ne!(
        maneuver
            .owner()
            .component_ordinal()
            .origin()
            .source_uses_refs(),
        alias
            .owner()
            .component_ordinal()
            .origin()
            .source_uses_refs(),
        "equal maneuvering values from separate owners retain distinct origins"
    );
    thruster_values(main, maneuver);
    Ok(())
}

fn thruster_values(main: &Thruster<'_>, maneuver: &Thruster<'_>) {
    assert_eq!(
        main.source().source_type().value(),
        Some(&fixture::THRUSTER),
        "the typed row identifies the original thruster declaration"
    );
    assert_eq!(
        main.operating.thruster_type.value(),
        Some(&"Main"),
        "the first authored role is the main thruster"
    );
    assert_eq!(
        maneuver.operating.thruster_type.value(),
        Some(&"Maneuver"),
        "the second authored role is a maneuvering thruster"
    );
    assert_eq!(
        main.operating
            .thrust_capacity
            .value()
            .map(ResourceFloat32::raw_bits),
        Some(0x8000_0000),
        "the original thrust capacity retains negative zero bits"
    );
    assert_eq!(
        main.operating
            .thrust_capacity_new
            .value()
            .and_then(ResourceFloat32::finite),
        Some(7.5),
        "the newer authored thrust capacity retains its finite value"
    );
    assert_eq!(
        maneuver
            .operating
            .thrust_capacity_new
            .value()
            .map(ResourceFloat32::raw_bits),
        Some(fixture::RAW_NAN),
        "the maneuvering capacity retains its original NaN payload"
    );
}

fn owner_position(owner: &ComponentOwner) -> Option<(u32, u32)> {
    owner
        .record_index()
        .value()
        .copied()
        .zip(owner.component_ordinal().value().copied())
}
