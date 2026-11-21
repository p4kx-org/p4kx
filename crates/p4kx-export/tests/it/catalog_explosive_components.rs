// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogError, CatalogPublicationMode, build_catalog};
use p4kx_extraction::{
    Archive, DataForge, DataType, RecordTree, StructValue, Value as Original, ValueError,
};
use p4kx_testkit::DataForgeBuilder;
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::health_origin_assertions as proof;

#[path = "catalog_explosive_components_fixture.rs"]
mod fixture;
#[path = "catalog_explosive_component_proof.rs"]
mod source_proof;

use fixture::Case;
use source_proof::{check_diagnostic, require_use};

const ROWS: &str = "/configurations/explosives/rows";
const TRIGGER: &str = "triggerType";
const EMISSIONS: &str = "emissionsParams";
const ZERO: &str = "00000000";

#[test]
fn explosive_components_export_distinct_original_trigger_and_ordnance_forms() -> TestResult {
    let source = fixture::database(Case::Valid);
    check_valid_original(&source)?;
    let (_folder, archive) = common::open(&source, DcbOrigin::Encrypted)?;
    let catalog: Value =
        serde_json::from_slice(build_catalog(&archive, &common::options())?.json())?;
    assert_eq!(
        catalog["exportHealth"]["status"], "complete",
        "all exact populated settings are retained"
    );
    assert_eq!(
        proof::at(&catalog, "/configurations/explosives/availability")?,
        "available",
        "the source types are declared"
    );
    let [
        laser,
        proximity,
        base,
        null,
        bomb,
        space_mine,
        missile,
        generic,
    ] = proof::rows(&catalog, ROWS)?
    else {
        return Err("eight original device occurrences with a separate Health component".into());
    };
    check_mines(&catalog, [laser, proximity, base, null])?;
    check_bomb(&catalog, bomb)?;
    let space = configuration(space_mine, 7, "SCItemSpaceMineParams", "spaceMine")?;
    proof::present(space, EMISSIONS, &json!([]), Some(true))?;
    configuration(missile, 8, "SCItemMissileParams", "missile")?;
    configuration(generic, 9, "SExplosiveOrdnanceParams", "ordnance")?;
    let [health] = proof::rows(&catalog, "/configurations/durability/rows")? else {
        return Err("one independent original Health occurrence".into());
    };
    proof::present(health, "ownerRecordIndex", &json!(0), Some(true))?;
    proof::present(health, "componentOrdinal", &json!(5), Some(true))?;
    proof::present(
        health,
        "deathExplosionParams",
        &json!({"type": "base"}),
        Some(true),
    )?;
    Ok(())
}

#[test]
fn explosive_components_keep_unknown_and_unreadable_trigger_states_at_their_actual_slots()
-> TestResult {
    let source = fixture::database(Case::BadTrigger);
    with_tree(&source, |tree| {
        let Some(Original::Struct(unknown)) = component(tree, 1)?.field("TriggerType") else {
            return Err("physically readable future trigger".into());
        };
        assert_eq!(
            unknown.type_name(),
            "FixtureFutureSensorTrigger",
            "actual concrete subtype"
        );
        assert!(
            matches!(
                component(tree, 2)?.field("TriggerType"),
                Some(Original::Error(ValueError::InvalidPointer { .. }))
            ),
            "the second original strong pointer is genuinely unreadable"
        );
        assert_eq!(
            tree.error_count(),
            1,
            "only one original field is unreadable"
        );
        Ok(())
    })?;
    let (_folder, archive) = common::open(&source, DcbOrigin::Encrypted)?;
    let catalog = diagnostic(&archive)?;
    let [unknown, unreadable, later] = proof::rows(&catalog, ROWS)? else {
        return Err("one row per actual bad/healthy owning component".into());
    };
    let unknown = proof::cell(
        configuration(unknown, 1, "SSensorMineComponentParams", "sensorMine")?,
        TRIGGER,
        "unsupported",
    )?;
    require_use(&catalog, unknown, "$type", "selector")?;
    let unreadable = proof::cell(
        configuration(unreadable, 2, "SSensorMineComponentParams", "sensorMine")?,
        TRIGGER,
        "unreadable",
    )?;
    assert_eq!(
        unreadable["sourceSetComplete"], false,
        "attempted reads do not complete the value proof"
    );
    require_use(&catalog, unreadable, "$dataType", "selector")?;
    check_diagnostic(&catalog, "unsupportedValue", "Components[1].TriggerType")?;
    check_diagnostic(&catalog, "unreadableSource", "Components[2].TriggerType")?;
    let later = configuration(later, 3, "SSensorMineComponentParams", "sensorMine")?;
    assert_eq!(
        proof::at(later, "/triggerType/value/type")?,
        "laser",
        "later original slot survives"
    );
    Ok(())
}

#[test]
fn explosive_components_require_the_original_trigger_and_emissions_storage() -> TestResult {
    let source = fixture::database(Case::WrongStorage);
    with_tree(&source, |tree| {
        let trigger = field(component(tree, 1)?, "TriggerType")?;
        assert_eq!(
            trigger.storage().data_type(),
            DataType::Class,
            "malformed declaration uses Class"
        );
        assert!(
            matches!(trigger.value(), Original::Struct(value) if value.type_name() == "SSensorMineLaserTrigger"),
            "the wrong storage still produces the same plausible concrete value"
        );
        let emissions = field(component(tree, 2)?, EMISSIONS)?;
        assert_eq!(
            emissions.storage().data_type(),
            DataType::StrongPointer,
            "wrong emissions storage"
        );
        assert!(
            emissions.storage().is_array(),
            "the array flag alone would allow this shape"
        );
        assert!(
            matches!(emissions.value(), Original::Array(values) if values.len() == 2),
            "actual array still holds both plausible entries"
        );
        Ok(())
    })?;
    let (_folder, archive) = common::open(&source, DcbOrigin::Encrypted)?;
    let catalog = diagnostic(&archive)?;
    let [mine, bomb] = proof::rows(&catalog, ROWS)? else {
        return Err("two original devices remain visible despite bad declarations".into());
    };
    let mine = configuration(mine, 1, "SSensorMineComponentParams", "sensorMine")?;
    let trigger = proof::cell(mine, TRIGGER, "invalid")?;
    require_use(&catalog, trigger, "$dataType", "selector")?;
    let bomb = configuration(bomb, 2, "SCItemBombParams", "bomb")?;
    let emissions = proof::cell(bomb, EMISSIONS, "invalid")?;
    require_use(&catalog, emissions, "$dataType", "selector")?;
    check_diagnostic(&catalog, "malformedSource", "Components[1].TriggerType")?;
    check_diagnostic(&catalog, "malformedSource", "Components[2].emissionsParams")
}

fn check_valid_original(source: &DataForgeBuilder) -> TestResult {
    with_tree(source, |tree| {
        let laser = field(component(tree, 1)?, "TriggerType")?;
        assert_eq!(
            laser.storage().data_type(),
            DataType::StrongPointer,
            "the authored owned trigger"
        );
        assert!(
            matches!(laser.value(), Original::Struct(value) if value.type_name() == "SSensorMineLaserTrigger"),
            "the authored laser variant"
        );
        let proximity = field(component(tree, 2)?, "TriggerType")?;
        assert!(
            matches!(proximity.value(), Original::Struct(value) if value.type_name() == "SSensorMineProximityTrigger"),
            "the authored proximity variant"
        );
        assert!(
            matches!(
                component(tree, 4)?.field("TriggerType"),
                Some(Original::Null)
            ),
            "an actual stored null"
        );
        let bomb = component(tree, 6)?;
        assert_eq!(
            bomb.type_name(),
            "SCItemBombParams",
            "the original derived component"
        );
        let emissions = field(bomb, EMISSIONS)?;
        assert_eq!(
            emissions.storage().data_type(),
            DataType::Class,
            "the original inline emissions class"
        );
        assert!(
            emissions.storage().is_array(),
            "the original emission multiplicity"
        );
        assert!(
            matches!(emissions.value(), Original::Array(values) if values.len() == 2),
            "two actual equal-valued emissions occur separately"
        );
        assert!(
            matches!(component(tree, 7)?.field(EMISSIONS), Some(Original::Array(values)) if values.is_empty()),
            "an actual empty emissions array"
        );
        Ok(())
    })
}

fn check_mines(catalog: &Value, rows: [&Value; 4]) -> TestResult {
    let [laser, proximity, base, null] = rows;
    let laser = configuration(laser, 1, "SSensorMineComponentParams", "sensorMine")?;
    proof::present(laser, "friendlyFire", &json!(false), Some(true))?;
    proof::present(
        proof::at(laser, "/laserVfx/value")?,
        "path",
        &json!("fixture_particle"),
        Some(true),
    )?;
    let trigger = proof::cell(laser, TRIGGER, "present")?;
    require_use(catalog, trigger, "$type", "selector")?;
    require_use(catalog, trigger, "$dataType", "selector")?;
    let laser = proof::at(trigger, "/value")?;
    assert_eq!(laser["type"], "laser", "the original laser form");
    proof::present(
        proof::at(laser, "/value")?,
        "laserLength",
        &raw(ZERO, Some(0.0)),
        Some(true),
    )?;
    let proximity = configuration(proximity, 2, "SSensorMineComponentParams", "sensorMine")?;
    let trigger = proof::at(proximity, "/triggerType/value")?;
    assert_eq!(trigger["type"], "proximity", "the original proximity form");
    proof::present(
        proof::at(trigger, "/value")?,
        "maxRaysPerMine",
        &json!(-1),
        Some(true),
    )?;
    let base = configuration(base, 3, "SSensorMineComponentParams", "sensorMine")?;
    proof::present(base, TRIGGER, &json!({"type": "base"}), Some(true))?;
    let null = configuration(null, 4, "SSensorMineComponentParams", "sensorMine")?;
    require_use(
        catalog,
        proof::cell(null, TRIGGER, "null")?,
        "$value",
        "selector",
    )
}

fn check_bomb(catalog: &Value, row: &Value) -> TestResult {
    let bomb = configuration(row, 6, "SCItemBombParams", "bomb")?;
    proof::present(bomb, "requiresLauncher", &json!(false), Some(true))?;
    proof::present(bomb, "armTime", &raw(ZERO, Some(0.0)), Some(true))?;
    proof::present(bomb, "maxArmableOverride", &json!(-3), Some(true))?;
    proof::present(
        bomb,
        "dragAreaRadius",
        &raw("40800000", Some(4.0)),
        Some(true),
    )?;
    let explosion = proof::at(bomb, "/explosionParams/value")?;
    proof::present(explosion, "pressure", &raw("7f800000", None), Some(true))?;
    let damage = proof::at(explosion, "/damage/value/value")?;
    proof::present(damage, "physical", &raw("3f800000", Some(1.0)), Some(true))?;
    proof::present(damage, "energy", &raw("40000000", Some(2.0)), Some(true))?;
    let emissions = proof::cell(bomb, EMISSIONS, "present")?;
    require_use(catalog, emissions, "$isArray", "selector")?;
    let [first, second] = proof::rows(emissions, "/value")? else {
        return Err("both original emissions are retained".into());
    };
    for entry in [first, second] {
        proof::present(
            proof::at(entry, "/value")?,
            "active",
            &json!(false),
            Some(true),
        )?;
    }
    assert_ne!(
        first["sourceUsesRefs"], second["sourceUsesRefs"],
        "equal array values retain distinct slots"
    );
    Ok(())
}

fn configuration<'row>(
    row: &'row Value,
    slot: u32,
    source: &str,
    kind: &str,
) -> TestResult<&'row Value> {
    proof::present(row, "ownerRecordIndex", &json!(0), Some(true))?;
    proof::present(row, "componentOrdinal", &json!(slot), Some(true))?;
    proof::present(row, "sourceType", &json!(source), Some(true))?;
    let carrier = proof::cell(row, "configuration", "present")?;
    assert_eq!(
        carrier["containsEncryptedSource"], true,
        "actual structural selection marker"
    );
    assert_eq!(
        proof::at(carrier, "/value/type")?,
        kind,
        "typed original component form"
    );
    proof::at(carrier, "/value/value")
}

fn diagnostic(archive: &Archive) -> TestResult<Value> {
    assert!(
        matches!(
            build_catalog(archive, &common::options()),
            Err(CatalogError::IncompleteExport(_))
        ),
        "normal publication refuses actual projection loss"
    );
    let mut options = common::options();
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    Ok(serde_json::from_slice(
        build_catalog(archive, &options)?.json(),
    )?)
}

fn with_tree(
    source: &DataForgeBuilder,
    check: impl FnOnce(&RecordTree<'_>) -> TestResult,
) -> TestResult {
    let bytes = source.build();
    let forge = DataForge::open(&bytes)?;
    let tree = forge.record(0).ok_or("physical device owner")?.tree()?;
    check(&tree)
}

fn component<'read, 'source>(
    tree: &'read RecordTree<'source>,
    slot: usize,
) -> TestResult<&'read StructValue<'source>> {
    let Some(Original::Array(components)) = tree.field("Components") else {
        return Err("actual Components array".into());
    };
    let Some(Original::Struct(value)) = components.get(slot) else {
        return Err("actual device component".into());
    };
    Ok(value)
}

fn field<'read, 'source>(
    parent: &'read StructValue<'source>,
    name: &str,
) -> TestResult<&'read p4kx_extraction::Field<'source>> {
    parent
        .fields()
        .iter()
        .find(|field| field.name() == name)
        .ok_or_else(|| "actual source field".into())
}

fn raw(bits: &str, finite: Option<f32>) -> Value {
    json!({"rawBits": bits, "finite": finite})
}
