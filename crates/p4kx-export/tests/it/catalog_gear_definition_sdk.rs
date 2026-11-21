// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::equipment::component_definitions::{
    DamageResistanceMacroBody, DurabilityResistanceEntry, MiningLaserGlobalDefinition,
};
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, ParsedCatalogQueries,
    ResourceFloat32,
};
use p4kx_export::{CatalogPublicationMode, build_catalog_from};
use p4kx_extraction::{Archive, DataForgeEntry, SourcedValue};
use serde_json::Value;

use crate::canonical_header::{originals, wire::Wire};
use crate::catalog_resources::resource_consumer_walk;
use crate::commodity_v3_fixture::{self as common, TestResult};
use crate::gear_fixture;
use crate::health_origin_assertions as proof;

#[test]
fn gear_definition_queries_keep_typed_catalog_values_and_origins() -> TestResult {
    let folder = tempfile::tempdir()?;
    let archive = Archive::open_with_key(
        gear_fixture::write_archive(folder.path())?,
        Some(p4kx_testkit::test_key()),
    )?;
    let entry = DataForgeEntry::read(&archive)?;
    let mut options = common::options();
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    let built = build_catalog_from(&archive, &entry, &options)?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let session = CatalogQuerySession::from_entry(
        &archive,
        &entry,
        CatalogLanguageSelection::Selected(&[]),
        None,
    )?;
    let parsed = session.parsed(None)?;
    damage_macros(&parsed, &catalog)?;
    mining_global(&parsed, &catalog)
}

fn damage_macros(parsed: &ParsedCatalogQueries<'_>, catalog: &Value) -> TestResult {
    let expected = proof::rows(catalog, "/definitions/damageResistanceMacros/rows")?;
    assert_eq!(
        expected.len(),
        2,
        "both physical fixture records share the macro GUID"
    );
    parsed.with_damage_resistance_macros(CatalogRecordSelection::All, None, |snapshot| {
        let rows = snapshot.rows();
        assert_eq!(
            rows.len(),
            expected.len(),
            "the SDK keeps both physical records"
        );
        for ((row, expected), multiplier) in rows.iter().zip(expected).zip([0.6, 0.25]) {
            let wire = Wire::new(expected, snapshot.source_inventory());
            wire.raw(row)?;
            resource_consumer_walk::visit_header(row.source(), wire.field("source")?)?;
            let body = row.damage_resistance();
            assert!(
                body.value().is_some(),
                "the original inline resistance is present"
            );
            wire.field("damageResistance")?
                .cell(body, |value, wire| macro_body(value, multiplier, wire))?;
            originals::visit_arena(row.additional_fields(), wire.field("additionalFields")?)?;
        }
        Ok(())
    })?
}

fn macro_body(
    value: &DamageResistanceMacroBody<'_>,
    multiplier: f32,
    wire: Wire<'_>,
) -> TestResult {
    let channels = value.channels();
    let physical = channels
        .physical()
        .value()
        .ok_or("authored physical channel")?;
    encrypted_bits(physical.multiplier(), multiplier);
    wire.field("ignoreMeleeDamage")?
        .leaf(channels.ignore_melee_damage())?;
    for (name, cell) in [
        ("physical", channels.physical()),
        ("energy", channels.energy()),
        ("distortion", channels.distortion()),
        ("thermal", channels.thermal()),
        ("biochemical", channels.biochemical()),
        ("stun", channels.stun()),
    ] {
        wire.field(name)?.cell(cell, resistance_entry)?;
    }
    wire.field("sourceType")?
        .leaf(value.source().source_type())?;
    originals::visit_arena(
        value.source().additional_fields(),
        wire.field("additionalFields")?,
    )
}

fn resistance_entry(value: &DurabilityResistanceEntry, wire: Wire<'_>) -> TestResult {
    for (name, cell) in [
        ("multiplier", value.multiplier()),
        ("threshold", value.threshold()),
        ("damageCap", value.damage_cap()),
        ("damageCapLimit", value.damage_cap_limit()),
    ] {
        wire.field(name)?
            .cell(cell, |value, wire| wire.float32(value))?;
    }
    Ok(())
}

fn mining_global(parsed: &ParsedCatalogQueries<'_>, catalog: &Value) -> TestResult {
    let [expected] = proof::rows(catalog, "/definitions/miningLaserGlobals/rows")? else {
        return Err("one canonical mining laser global".into());
    };
    let index = expected
        .pointer("/source/recordIndex/value")
        .and_then(Value::as_u64)
        .ok_or("physical mining laser global index")?;
    let selected = CatalogRecordSelection::RecordIndex(u32::try_from(index)?);
    parsed.with_mining_laser_globals(selected, None, |snapshot| {
        let [row] = snapshot.rows() else {
            return Err("one selected mining laser global".into());
        };
        let wire = Wire::new(expected, snapshot.source_inventory());
        wire.raw(row)?;
        resource_consumer_walk::visit_header(row.source(), wire.field("source")?)?;
        throttle(row, wire)?;
        originals::visit_arena(row.additional_fields(), wire.field("additionalFields")?)
    })?
}

fn throttle(row: &MiningLaserGlobalDefinition<'_>, wire: Wire<'_>) -> TestResult {
    encrypted_bits(row.throttle_change_per_action(), 0.25);
    wire.field("blockThrottleChangeWhenNotFiring")?
        .leaf(row.block_throttle_change_when_not_firing())?;
    wire.field("throttleResetOnStopFire")?
        .leaf(row.throttle_reset_on_stop_fire())?;
    for (name, cell) in [
        ("throttleChangePerAction", row.throttle_change_per_action()),
        ("throttleAccPeriod", row.throttle_acc_period()),
        ("throttleAccFactor", row.throttle_acc_factor()),
        ("throttleHoldAccFactor", row.throttle_hold_acc_factor()),
    ] {
        wire.field(name)?
            .cell(cell, |value, wire| wire.float32(value))?;
    }
    let rtpc = row
        .throttle_rtpc()
        .value()
        .ok_or("authored throttle RTPC")?;
    assert_eq!(
        rtpc.rtpc().value().map(String::as_str),
        Some("FixtureThrottle"),
        "the nested getter preserves the authored RTPC identifier",
    );
    for (name, cell) in [
        ("throttleRtpc", row.throttle_rtpc()),
        (
            "mineableGlowStrengthRtpc",
            row.mineable_glow_strength_rtpc(),
        ),
        (
            "mineableOptimalBreakZoneRtpc",
            row.mineable_optimal_break_zone_rtpc(),
        ),
    ] {
        wire.field(name)?
            .cell(cell, |value, wire| wire.field("rtpc")?.leaf(value.rtpc()))?;
    }
    Ok(())
}

fn encrypted_bits(cell: &SourcedValue<ResourceFloat32>, expected: f32) {
    assert_eq!(
        cell.value().map(ResourceFloat32::raw_bits),
        Some(expected.to_bits()),
        "the typed getter preserves the original Single bits",
    );
    assert_eq!(
        cell.origin().contains_encrypted_source(),
        Some(true),
        "the numeric value originates in encrypted DataForge payload",
    );
}
