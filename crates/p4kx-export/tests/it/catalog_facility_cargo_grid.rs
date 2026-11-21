// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::build_catalog;
use p4kx_extraction::{Archive, DataForgeEntry, Value as Original};
use p4kx_testkit::{
    TestCargoGridLayout, cargo_grid_facility_database, cargo_grid_facility_database_for,
};
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::health_origin_assertions as proof;

#[path = "catalog_facility_cargo_grid_filter.rs"]
mod filter;
#[path = "catalog_facility_cargo_grid_sdk.rs"]
mod sdk;

const FACILITIES: &str = "/configurations/facilities/rows";
const INVENTORY_CONFIGURATIONS: &str = "/configurations/inventoryConfigurations/rows";
const INVENTORY_CONTAINERS: &str = "/definitions/inventoryContainers/rows";
const INTERIOR_DIMENSIONS: [f32; 3] = [1.25, 1.25, 2.5];
const GRID_SCU: f64 = 2.0;

#[test]
fn cargo_grid_authored_values_and_colocated_components_use_their_original_slots() -> TestResult {
    for (origin, encrypted) in [(DcbOrigin::Plain, false), (DcbOrigin::Encrypted, true)] {
        let (_directory, archive) = common::open(&cargo_grid_facility_database(), origin)?;
        verify_original(&archive)?;
        let catalog: Value =
            serde_json::from_slice(build_catalog(&archive, &common::options())?.json())?;
        let grid = only_grid(&catalog)?;
        check_grid(grid, encrypted)?;
        check_additional_field(grid, encrypted)?;
        check_placements(&catalog, grid, encrypted)?;
        check_available_capacity(&catalog, grid, encrypted)?;
        jsonschema::validator_for(&p4kx_export::catalog_schema())?
            .validate(&catalog)
            .map_err(|error| format!("schema: {error}"))?;
        proof::closure(&catalog)?;
    }
    Ok(())
}

#[test]
fn cargo_grid_visible_mining_capacity_remains_available_at_both_placements() -> TestResult {
    let database = cargo_grid_facility_database_for(TestCargoGridLayout::MiningOnly);
    let (_directory, archive) = common::open(&database, DcbOrigin::Encrypted)?;
    verify_original(&archive)?;
    let catalog: Value =
        serde_json::from_slice(build_catalog(&archive, &common::options())?.json())?;
    let grid = only_grid(&catalog)?;
    proof::present(grid, "miningOnly", &json!(true), Some(true))?;
    proof::present(grid, "invisible", &json!(false), Some(true))?;
    check_placements(&catalog, grid, true)?;
    check_available_capacity(&catalog, grid, true)?;
    proof::closure(&catalog)
}

#[test]
fn cargo_grid_without_a_unique_supported_storage_has_no_scu() -> TestResult {
    use TestCargoGridLayout::{DuplicateStorage, MissingStorage, UnsupportedStorage};

    for (layout, status, state, bound) in [
        (MissingStorage, "missingStorageComponent", "absent", false),
        (DuplicateStorage, "ambiguousComponents", "invalid", false),
        (
            UnsupportedStorage,
            "unsupportedCapacity",
            "unsupported",
            true,
        ),
    ] {
        let database = cargo_grid_facility_database_for(layout);
        let (_directory, archive) = common::open(&database, DcbOrigin::Plain)?;
        let catalog: Value =
            serde_json::from_slice(build_catalog(&archive, &common::options())?.json())?;
        let capacity = proof::at(only_grid(&catalog)?, "/capacity")?;
        assert_eq!(proof::at(capacity, "/status")?, status, "{layout:?}");
        let scu = proof::cell(capacity, "scu", state)?;
        if layout == DuplicateStorage {
            let [first, second] = proof::rows(&catalog, INVENTORY_CONFIGURATIONS)? else {
                return Err("both original competing storage configurations".into());
            };
            let evidence = proof::rows(scu, "/sourceUsesRefs")?;
            for storage in [first, second] {
                let selected = proof::rows(proof::at(storage, "/sourceType")?, "/sourceUsesRefs")?;
                assert!(
                    !selected.is_empty() && selected.iter().all(|used| evidence.contains(used)),
                    "both original storage types contribute to the ambiguous selection"
                );
            }
        }
        if bound {
            proof::present(capacity, "storageComponentOrdinal", &json!(1), Some(false))?;
            proof::present(capacity, "containerRecordIndex", &json!(2), Some(false))?;
        } else {
            for field in ["storageComponentOrdinal", "containerRecordIndex"] {
                assert_eq!(
                    capacity.get(field),
                    Some(&Value::Null),
                    "{layout:?} cannot establish a unique {field}"
                );
            }
        }
        proof::closure(&catalog)?;
    }
    Ok(())
}

fn only_grid(catalog: &Value) -> TestResult<&Value> {
    let [grid] = proof::rows(catalog, FACILITIES)? else {
        return Err("one authored cargo grid configuration".into());
    };
    Ok(grid)
}

fn verify_original(archive: &Archive) -> TestResult {
    let dcb = DataForgeEntry::read(archive)?;
    let forge = dcb.open()?;
    let owner = forge.record(1).ok_or("original grid owner")?.tree()?;
    let Some(Original::Array(components)) = owner.field("Components") else {
        return Err("original Components array".into());
    };
    let (Some(Original::Struct(storage)), Some(Original::Struct(grid))) =
        (components.get(1), components.get(2))
    else {
        return Err("two original physical component slots".into());
    };
    assert_eq!(
        storage.type_name(),
        "SCItemInventoryContainerComponentParams",
        "storage owns the first selected component slot"
    );
    assert_eq!(
        grid.type_name(),
        "SCItemCargoGridParams",
        "grid owns the second selected component slot"
    );
    assert!(
        matches!(storage.field("containerParams"), Some(Original::Reference(reference)) if reference.record_index() == 2),
        "the actual storage component refers to the independent InventoryContainer"
    );
    assert!(
        matches!(grid.field("damageConfiguration"), Some(Original::Reference(reference)) if reference.record_index() == 3),
        "the actual grid component refers to the original damage Record"
    );
    let Some(Original::Struct(dimensions)) = grid.field("dimensions") else {
        return Err("original Vec3".into());
    };
    assert!(
        matches!(dimensions.field("x"), Some(Original::Single(value)) if value.to_bits() == 1_250.0_f32.to_bits()),
        "original grid dimensions exceed the independent ShipBase limit"
    );
    let own_storage = forge
        .record(2)
        .ok_or("original storage definition")?
        .tree()?;
    let Some(Original::Struct(interior)) = own_storage.field("interiorDimensions") else {
        return Err("original storage interior Vec3".into());
    };
    for (axis, expected) in ["x", "y", "z"].into_iter().zip(INTERIOR_DIMENSIONS) {
        assert!(
            matches!(interior.field(axis), Some(Original::Single(actual)) if actual.to_bits() == expected.to_bits()),
            "capacity operand {axis} is a native Single on the storage Record"
        );
    }
    Ok(())
}

fn check_available_capacity(catalog: &Value, grid: &Value, encrypted: bool) -> TestResult {
    assert_eq!(
        proof::at(catalog, "/ships/0/cargoScu")?,
        GRID_SCU * 2.0,
        "legacy ShipBase capacity includes both placements with the original grid flags"
    );
    let capacity = proof::at(grid, "/capacity")?;
    assert_eq!(
        proof::at(capacity, "/status")?,
        "available",
        "unique storage"
    );
    let scu = proof::present(capacity, "scu", &json!(GRID_SCU), Some(encrypted))?;
    let [storage] = proof::rows(catalog, INVENTORY_CONFIGURATIONS)? else {
        return Err("one observed storage component".into());
    };
    let [inventory] = proof::rows(catalog, INVENTORY_CONTAINERS)? else {
        return Err("one original inventory container".into());
    };
    for (field, original) in [
        (
            "storageComponentOrdinal",
            proof::at(storage, "/componentOrdinal")?,
        ),
        (
            "containerRecordIndex",
            proof::at(storage, "/containerParams/value/targetRecordIndex")?,
        ),
    ] {
        assert_eq!(
            capacity.get(field),
            Some(original),
            "the exact original {field} is reused"
        );
    }
    check_operand_origins(catalog, scu, grid, storage, inventory)
}

fn check_operand_origins(
    catalog: &Value,
    scu: &Value,
    grid: &Value,
    storage: &Value,
    inventory: &Value,
) -> TestResult {
    let contributors = proof::rows(scu, "/sourceUsesRefs")?;
    for (owner, fields) in [
        (storage, &["/containerParams"][..]),
        (
            inventory,
            &[
                "/inventoryType",
                "/interiorDimensions",
                "/interiorDimensions/value/x",
                "/interiorDimensions/value/y",
                "/interiorDimensions/value/z",
            ][..],
        ),
    ] {
        for field in fields {
            let expected = proof::rows(proof::at(owner, field)?, "/sourceUsesRefs")?;
            assert!(
                !expected.is_empty() && expected.iter().all(|used| contributors.contains(used)),
                "the SCU calculation includes the exact source evidence for {field}"
            );
        }
    }
    let used_operands = proof::uses(catalog, scu)?;
    for field in ["/dimensions/value/x", "/miningOnly", "/invisible"] {
        let field_uses = proof::uses(catalog, proof::at(grid, field)?)?;
        let payload = field_uses
            .iter()
            .find(|used| {
                used.get("role").and_then(Value::as_str) == Some("payload")
                    && used.pointer("/location/field").and_then(Value::as_str) == Some("$value")
            })
            .ok_or_else(|| format!("original payload for {field}"))?;
        let location = proof::at(payload, "/location")?;
        assert!(
            used_operands
                .iter()
                .all(|used| used.get("location") != Some(location)),
            "the SCU calculation does not use the original value of {field}"
        );
    }
    Ok(())
}

fn check_grid(grid: &Value, encrypted: bool) -> TestResult {
    assert_eq!(
        proof::at(grid, "/kind")?,
        "cargoGrid",
        "the facility discriminator follows the original component"
    );
    for (field, value) in [
        ("ownerRecordIndex", json!(1)),
        ("componentOrdinal", json!(2)),
        ("sourceType", json!("SCItemCargoGridParams")),
        (
            "crateGenPercentageOnDestroy",
            json!({"rawBits":"7fc00042", "finite":null}),
        ),
        ("crateMaxOnDestroy", json!(i32::MAX)),
        ("invisible", json!(true)),
        ("miningOnly", json!(false)),
        ("transformDependant", json!(true)),
        (
            "minVolatilePowerToExplode",
            json!({"rawBits":"80000000", "finite":-0.0}),
        ),
    ] {
        proof::present(grid, field, &value, Some(encrypted))?;
    }
    let dimensions = proof::at(grid, "/dimensions/value")?;
    for (axis, value) in [
        ("x", json!({"rawBits":"449c4000", "finite":1250.0})),
        ("y", json!({"rawBits":"80000000", "finite":-0.0})),
        ("z", json!({"rawBits":"7f800000", "finite":null})),
    ] {
        proof::present(dimensions, axis, &value, Some(encrypted))?;
    }
    let reference = proof::at(grid, "/damageConfiguration/value")?;
    proof::present(reference, "targetRecordIndex", &json!(3), Some(encrypted))?;
    proof::present(
        reference,
        "targetType",
        &json!("CommodityDamageConfiguration"),
        Some(encrypted),
    )?;
    Ok(())
}

fn check_additional_field(grid: &Value, encrypted: bool) -> TestResult {
    let [extra] = proof::rows(grid, "/additionalFields/roots")? else {
        return Err("the extra original field without repeating typed members".into());
    };
    proof::present(extra, "name", &json!("fixtureExtra"), Some(encrypted))?;
    let index = proof::at(extra, "/value/valueIndex")?
        .as_str()
        .ok_or("original cargo grid value index")?
        .parse::<usize>()?;
    let value = proof::rows(grid, "/additionalFields/values")?
        .get(index)
        .ok_or("original cargo grid value")?;
    proof::present(
        value,
        "value",
        &json!({"type":"number", "value":{"type":"uint16", "value":14}}),
        Some(encrypted),
    )?;
    Ok(())
}

fn check_placements(catalog: &Value, grid: &Value, encrypted: bool) -> TestResult {
    let [left, right] = proof::rows(catalog, "/placements/facilityInstances/rows")? else {
        return Err("one physical facility instance at each of two ports".into());
    };
    let [storage] = proof::rows(catalog, INVENTORY_CONFIGURATIONS)? else {
        return Err("one authored storage configuration".into());
    };
    for (placement, ordinal) in [(left, 0), (right, 1)] {
        let position = proof::at(placement, "/source/position")?;
        assert_eq!(
            proof::at(position, "/value/portOrdinals")?,
            &json!([ordinal]),
            "each resolved original port keeps its own placement"
        );
        assert_eq!(
            proof::at(position, "/containsEncryptedSource")?,
            encrypted,
            "placement uses the observed attachment origin"
        );
        let [inventory, facility] = proof::rows(placement, "/configurations")? else {
            return Err("two links in original component order at each port".into());
        };
        assert_eq!(
            proof::at(inventory, "/kind")?,
            "inventory",
            "storage is the first original component"
        );
        assert_eq!(
            proof::at(facility, "/kind")?,
            "facility",
            "cargo grid is the second original component"
        );
        for (link, declaration) in [(inventory, storage), (facility, grid)] {
            let slot = proof::at(link, "/owner/componentOrdinal")?;
            assert_eq!(
                slot,
                proof::at(declaration, "/componentOrdinal")?,
                "the link preserves the same sourced slot as its declaration"
            );
        }
        assert_ne!(
            proof::at(inventory, "/owner/componentOrdinal/sourceUsesRefs")?,
            proof::at(facility, "/owner/componentOrdinal/sourceUsesRefs")?,
            "co-located components retain independent physical provenance"
        );
    }
    assert_ne!(
        proof::at(left, "/source/position/sourceUsesRefs")?,
        proof::at(right, "/source/position/sourceUsesRefs")?,
        "the two placements consume distinct original attachment selectors"
    );
    Ok(())
}

#[path = "catalog_facility_cargo_grid_closed.rs"]
mod closed;
