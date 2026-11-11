// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::build_catalog;
use p4kx_extraction::{DataForge, DataType, RecordTree, StructValue, Value as Raw};
use serde_json::{Value, json};

use super::source;
use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::health_origin_assertions as proof;

#[path = "catalog_weapon_action_selector_fixture.rs"]
mod fixture;
#[path = "catalog_weapon_action_selector_states.rs"]
mod states;
#[path = "catalog_weapon_action_wrappers.rs"]
mod wrappers;

use fixture::{ALPHA, BEAM, BETA, BURST, ITEM, ITEM_INDEX, RAPID, WEAPON};

struct OriginalAction {
    ordinal: u64,
    selector: u64,
}

struct OriginalSelectors {
    first: OriginalAction,
    second: OriginalAction,
    repeated: OriginalAction,
}

#[test]
fn weapon_launchers_preserve_authored_selectors_by_physical_action_owner() -> TestResult {
    let builder = fixture::database();
    let bytes = builder.build();
    let forge = DataForge::open(&bytes)?;
    let original = original_selectors(&forge)?;
    let (_folder, archive) = common::open(&builder, DcbOrigin::Encrypted)?;
    let built = build_catalog(&archive, &common::options())?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    check_legacy(&catalog)?;
    check_ammo_binding(&catalog)?;
    let [first, repeated] = proof::rows(&catalog, "/configurations/weaponActions/rows")? else {
        return Err("one action row per original weapon component occurrence".into());
    };
    check_owner(first, 1)?;
    check_owner(repeated, 3)?;
    check_first(&catalog, first, &original)?;
    let [node] = proof::rows(repeated, "/actionNodes")? else {
        return Err("one original action in the second weapon component".into());
    };
    check_launcher(&catalog, node, &original.repeated, RAPID, "rapid", ALPHA)?;
    let schema = p4kx_export::catalog_schema();
    let validator = jsonschema::validator_for(&schema)?;
    validator
        .validate(&catalog)
        .map_err(|error| error.to_string())?;
    proof::closure(&catalog)
}

fn check_owner(row: &Value, ordinal: u32) -> TestResult {
    proof::present(row, "ownerGuid", &json!(common::guid(ITEM)), Some(true))?;
    proof::present(row, "ownerRecordIndex", &json!(ITEM_INDEX), Some(true))?;
    proof::present(row, "componentOrdinal", &json!(ordinal), Some(true))?;
    Ok(())
}

fn check_first(catalog: &Value, row: &Value, original: &OriginalSelectors) -> TestResult {
    let [rapid, null, burst, beam] = proof::rows(row, "/fireActions/value")? else {
        return Err("all four original action slots including null".into());
    };
    for (slot, index) in [(rapid, 0), (burst, 1), (beam, 2)] {
        assert_eq!(
            proof::at(slot, "/value")?,
            &json!(index),
            "the stored source slot addresses its owner-local action node"
        );
    }
    assert_eq!(
        proof::at(null, "/state")?,
        "null",
        "the null source slot stays in order"
    );
    let [rapid, burst, beam] = proof::rows(row, "/actionNodes")? else {
        return Err("three readable authored action nodes".into());
    };
    check_launcher(catalog, rapid, &original.first, RAPID, "rapid", ALPHA)?;
    check_launcher(catalog, burst, &original.second, BURST, "burst", BETA)?;
    check_heat(rapid, burst, beam)
}

fn check_launcher(
    catalog: &Value,
    node: &Value,
    original: &OriginalAction,
    source_type: &str,
    kind: &str,
    selector: &str,
) -> TestResult {
    proof::present(node, "sourceType", &json!(source_type), Some(true))?;
    let occurrence = proof::present(
        node,
        "sourceValueOrdinal",
        &json!(original.ordinal.to_string()),
        Some(true),
    )?;
    check_use(catalog, occurrence, original.ordinal, "conversion")?;
    assert_eq!(
        proof::at(node, "/kind/type")?,
        kind,
        "the actual action alternative"
    );
    let launcher = proof::at(node, "/kind/value/launchParams/value")?;
    assert_eq!(
        proof::at(launcher, "/type")?,
        "projectile",
        "the actual launcher alternative"
    );
    let selector = proof::present(launcher, "projectileType", &json!(selector), Some(true))?;
    check_use(catalog, selector, original.selector, "payload")
}

fn check_use(catalog: &Value, cell: &Value, ordinal: u64, role: &str) -> TestResult {
    let indices = json!([ITEM_INDEX.to_string(), ordinal.to_string()]);
    assert!(
        proof::uses(catalog, cell)?.iter().any(|used| {
            used.get("role").and_then(Value::as_str) == Some(role)
                && used.pointer("/location/namespace").and_then(Value::as_str)
                    == Some("dataforgeValue")
                && used.pointer("/location/indices") == Some(&indices)
                && used.pointer("/location/field").and_then(Value::as_str) == Some("$value")
        }),
        "the cell cites its actual source occurrence with role {role}"
    );
    Ok(())
}

fn check_heat(rapid: &Value, burst: &Value, beam: &Value) -> TestResult {
    let rapid = proof::at(rapid, "/kind/value")?;
    let burst = proof::at(burst, "/kind/value")?;
    let beam = proof::at(beam, "/kind/value")?;
    check_single(
        rapid,
        "/launchParams/value/heatCost/value/rawBits",
        "3e800000",
    )?;
    check_single(rapid, "/heatPerShot/value/rawBits", "3f000000")?;
    check_single(
        burst,
        "/launchParams/value/heatCost/value/rawBits",
        "3f800000",
    )?;
    check_single(burst, "/heatPerShot/value/rawBits", "3fa00000")?;
    check_single(beam, "/heatCostPerSecond/value/rawBits", "40c00000")?;
    check_single(
        rapid,
        "/launchParams/value/damageMultiplier/value/rawBits",
        "00000000",
    )?;
    proof::present(rapid, "fireDuringSpinUp", &json!(false), Some(true))?;
    proof::present(burst, "shotCount", &json!(0), Some(true))?;
    Ok(())
}

fn check_single(node: &Value, path: &str, raw_bits: &str) -> TestResult {
    assert_eq!(
        proof::at(node, path)?,
        raw_bits,
        "stored Single bits at {path}"
    );
    Ok(())
}

fn check_legacy(catalog: &Value) -> TestResult {
    let legacy = proof::at(catalog, "/items/0/stats/weapon")?;
    let [rapid, burst, beam] = proof::rows(legacy, "/fireActions")? else {
        return Err("the legacy list projects the first physical weapon only".into());
    };
    for (row, original) in [(rapid, RAPID), (burst, BURST), (beam, BEAM)] {
        assert_eq!(
            proof::at(row, "/kind")?,
            original,
            "unchanged legacy action type/order"
        );
    }
    assert_eq!(
        proof::at(rapid, "/heatPerShot")?,
        &json!(0.5),
        "legacy heat retains its input"
    );
    assert_eq!(
        proof::at(legacy, "/ammo/speed")?,
        &json!(1.25),
        "legacy selects the original primary slot"
    );
    Ok(())
}

fn check_ammo_binding(catalog: &Value) -> TestResult {
    let bindings = proof::rows(catalog, "/declarations/ammoBindings/rows")?;
    let [first, container, repeated] = bindings else {
        return Err("two physical weapon declarations and their shared Item container".into());
    };
    check_owner(proof::at(first, "/value")?, 1)?;
    check_owner(proof::at(repeated, "/value")?, 3)?;
    let container = proof::at(container, "/value")?;
    check_owner(container, 2)?;
    let [primary, secondary] = proof::rows(catalog, "/definitions/ammoDefinitions/rows")? else {
        return Err("both original Ammo definitions".into());
    };
    for (slot, definition) in [("primaryAmmo", primary), ("secondaryAmmo", secondary)] {
        assert_eq!(
            proof::at(container, &format!("/{slot}/value/targetRecordIndex/value"))?,
            proof::at(definition, "/recordIndex/value")?,
            "an authored container Reference identifies the exact {slot} Ammo Record"
        );
    }
    let [resolution] = proof::rows(catalog, "/projections/weaponMagazineResolutions/rows")? else {
        return Err("one legacy magazine selection".into());
    };
    check_owner(resolution, 1)?;
    assert_eq!(
        proof::at(resolution, "/outcome/value/containerOrdinal/value")?,
        &json!(2),
        "the legacy own-container projection identifies the same physical container"
    );
    Ok(())
}

fn original_selectors(forge: &DataForge<'_>) -> TestResult<OriginalSelectors> {
    let tree = forge
        .record(ITEM_INDEX)
        .ok_or("original owning Item Record")?
        .tree()?;
    let Some(Raw::Array(components)) = tree.field("Components") else {
        return Err("original Item Components".into());
    };
    let first = original_actions(forge, components.get(1).ok_or("first physical Weapon")?)?;
    let repeated = original_actions(forge, components.get(3).ok_or("second physical Weapon")?)?;
    let [rapid, null, burst, beam] = first else {
        return Err("four original action slots in the first physical Weapon".into());
    };
    let [duplicate] = repeated else {
        return Err("one action slot in the second physical Weapon".into());
    };
    assert!(
        matches!(null, Raw::Null),
        "the source stores a real null pointer slot"
    );
    let beam = owned(beam, BEAM)?;
    assert!(
        matches!(beam.field("heatCostPerSecond"), Some(Raw::Single(value)) if value.to_bits() == 0x40c0_0000),
        "the original beam has its distinct authored heat input"
    );
    Ok(OriginalSelectors {
        first: original_action(&tree, rapid, RAPID, ALPHA)?,
        second: original_action(&tree, burst, BURST, BETA)?,
        repeated: original_action(&tree, duplicate, RAPID, ALPHA)?,
    })
}

fn original_actions<'value, 'source>(
    forge: &DataForge<'source>,
    weapon: &'value Raw<'source>,
) -> TestResult<&'value [Raw<'source>]> {
    let weapon = owned(weapon, WEAPON)?;
    let field = weapon
        .fields()
        .iter()
        .find(|field| field.name() == "fireActions")
        .ok_or("original fireActions declaration")?;
    assert_eq!(
        field.storage().data_type(),
        DataType::StrongPointer,
        "original action pointer storage"
    );
    assert!(
        field.storage().is_array(),
        "the original action declaration is an array"
    );
    let target = forge
        .structs()
        .nth(usize::from(field.storage().target()))
        .ok_or("declared action base")?;
    assert_eq!(
        target.name()?,
        "SWeaponActionParams",
        "the original array targets the action base"
    );
    let Raw::Array(actions) = field.value() else {
        return Err("readable original action slots".into());
    };
    Ok(actions)
}

fn original_action<'source>(
    tree: &RecordTree<'source>,
    action: &Raw<'source>,
    source_type: &str,
    expected: &str,
) -> TestResult<OriginalAction> {
    let launcher = owned(action, source_type)?
        .field("launchParams")
        .ok_or("original launcher")?;
    let launcher = owned(launcher, "SProjectileLauncher")?;
    let selector = launcher
        .field("projectileType")
        .ok_or("actual original launcher Enum")?;
    assert!(
        matches!(selector, Raw::Enum(name) if *name == expected),
        "the original launcher stores the literal selector {expected}"
    );
    Ok(OriginalAction {
        ordinal: source::ordinal(tree, action)?,
        selector: source::ordinal(tree, selector)?,
    })
}

fn owned<'value, 'source>(
    value: &'value Raw<'source>,
    expected: &str,
) -> TestResult<&'value StructValue<'source>> {
    let Raw::Struct(node) = value else {
        return Err(format!("original {expected} Struct").into());
    };
    assert_eq!(node.type_name(), expected, "actual original Struct type");
    Ok(node)
}
