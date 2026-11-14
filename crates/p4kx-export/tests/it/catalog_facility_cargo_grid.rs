// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::build_catalog;
use p4kx_extraction::{Archive, DataForgeEntry, Value as Original};
use p4kx_testkit::cargo_grid_facility_database;
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::health_origin_assertions as proof;

#[test]
fn cargo_grid_authored_values_and_colocated_components_use_their_original_slots() -> TestResult {
    for (origin, encrypted) in [(DcbOrigin::Plain, false), (DcbOrigin::Encrypted, true)] {
        let (_directory, archive) = common::open(&cargo_grid_facility_database(), origin)?;
        verify_original(&archive)?;
        let catalog: Value =
            serde_json::from_slice(build_catalog(&archive, &common::options())?.json())?;
        assert_eq!(
            proof::at(&catalog, "/ships/0/cargoScu")?,
            4.0,
            "ShipBase still sums the InventoryContainer capacity of both ports"
        );
        let [grid] = proof::rows(&catalog, "/configurations/facilities/rows")? else {
            return Err("one authored cargo grid configuration".into());
        };
        check_grid(grid, encrypted)?;
        check_additional_field(grid, encrypted)?;
        check_placements(&catalog, grid, encrypted)?;
        jsonschema::validator_for(&p4kx_export::catalog_schema())?
            .validate(&catalog)
            .map_err(|error| format!("schema: {error}"))?;
        proof::closure(&catalog)?;
    }
    Ok(())
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
    let [storage] = proof::rows(catalog, "/configurations/inventoryConfigurations/rows")? else {
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
