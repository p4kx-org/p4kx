// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{Kind, build_catalog};
use p4kx_extraction::{DataForge, Guid};
use serde_json::Value;

use super::{check_unique_guids, definition_identity};
use crate::catalog_v3_consumer as reader;
use crate::commodity_v3_fixture::{self as source, DcbOrigin, TestResult};

#[path = "blueprint_pool_identity_fixture.rs"]
mod fixture;

use fixture::Collision;

#[test]
fn pool_identity_accepts_actual_physical_guid_alternatives_but_not_a_repeated_owner() -> TestResult
{
    let catalog = catalog(Collision::None)?;
    let pools = reader::rows(&catalog, Kind::BlueprintPools)?;
    assert_eq!(pools.len(), 2, "both original physical pools survive");
    let positions = pools
        .iter()
        .map(|row| {
            reader::field(row, Kind::BlueprintPools, "recordIndex")
                .and_then(|value| value.as_u64().ok_or_else(|| "original pool index".into()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    assert_eq!(
        positions,
        [2, 3],
        "aliases with the same GUID and path retain original physical order"
    );
    definition_identity::check(&catalog, Kind::BlueprintPools)?;
    check_unique_guids(&catalog)?;
    let first = pools.first().ok_or("first original pool")?.clone();
    let mut repeated = catalog;
    repeated
        .pointer_mut("/definitions/blueprintPools/rows")
        .and_then(Value::as_array_mut)
        .ok_or("canonical pool rows")?
        .push(first);
    assert!(
        check_unique_guids(&repeated).is_err(),
        "legitimate aliases never allow a duplicated physical owner"
    );
    Ok(())
}

#[test]
fn pool_identity_rejects_an_actual_manufacturer_guid_collision() -> TestResult {
    reject_collision(Collision::Manufacturer)
}

#[test]
fn pool_identity_rejects_an_actual_collision_with_another_physical_definition_family() -> TestResult
{
    reject_collision(Collision::Quality)
}

fn reject_collision(collision: Collision) -> TestResult {
    let catalog = catalog(collision)?;
    let other = collision
        .other()
        .ok_or("a collision fixture requires another family")?;
    let pools = reader::rows(&catalog, Kind::BlueprintPools)?;
    let [foreign] = reader::rows(&catalog, other)?.as_slice() else {
        return Err("one actual original foreign-family owner".into());
    };
    assert_eq!(
        pools.len(),
        2,
        "both physical Pool aliases remain independently present"
    );
    let guid = reader::identity(foreign, other)?;
    for pool in pools {
        assert_eq!(
            reader::identity(pool, Kind::BlueprintPools)?,
            guid,
            "the collision is in actual produced canonical identities"
        );
    }
    definition_identity::check(&catalog, Kind::BlueprintPools)?;
    let error = check_unique_guids(&catalog)
        .err()
        .ok_or("a cross-family Pool collision must not pass identity acceptance")?;
    assert_eq!(
        error.to_string(),
        format!("a blueprintPools GUID also occurs in {other}"),
        "the exact Pool cross-family guard, not an unrelated failure, rejects the fixture"
    );
    Ok(())
}

fn catalog(collision: Collision) -> TestResult<Value> {
    let bytes = fixture::database(collision).build();
    original(&bytes, collision)?;
    let (_folder, archive) = source::open_bytes(bytes, DcbOrigin::Plain)?;
    let built = build_catalog(&archive, &source::options())?;
    Ok(serde_json::from_slice(built.json())?)
}

fn original(bytes: &[u8], collision: Collision) -> TestResult {
    let forge = DataForge::open(bytes)?;
    let guid: Guid = source::guid(fixture::GUID).parse()?;
    let mut pools = Vec::new();
    let mut foreign = 0_usize;
    for record in forge.records() {
        if record.guid() != guid {
            continue;
        }
        match record.type_name()? {
            "BlueprintPoolRecord" => pools.push(record.index()),
            "SCItemManufacturer" if matches!(collision, Collision::Manufacturer) => foreign += 1,
            "CraftingQualityQuantizationRecord" if matches!(collision, Collision::Quality) => {
                foreign += 1;
            }
            _ => return Err("unexpected original owner of the collision GUID".into()),
        }
    }
    assert_eq!(pools, [2, 3], "actual original DataForge Pool owners");
    assert_eq!(
        foreign,
        usize::from(collision.other().is_some()),
        "the collision is original source data rather than a forged wire row"
    );
    Ok(())
}
