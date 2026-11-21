// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::Kind;
use serde_json::{Value, json};

use crate::hauling_fixture::{
    CRATE, CRATES, GOLD, MANIFEST_CUSTOM, MANIFEST_EVEN, MANIFEST_RANDOM, SCRAP, guid,
};

use super::{TestResult, build, json_array, json_at};

pub(super) fn verify() -> TestResult {
    let (built, catalog) = build()?;
    assert!(
        catalog.get("cargoManifests").is_none(),
        "no duplicate root catalog"
    );
    let table = json_at(&catalog, "/definitions/cargoManifests")?;
    assert_eq!(
        json_at(table, "/availability")?,
        "available",
        "manifest family is declared"
    );
    let rows = json_array(table, "/rows")?;
    assert_eq!(rows.len(), 4, "each physical manifest is emitted");
    let even_guid = guid(MANIFEST_EVEN);
    let custom_guid = guid(MANIFEST_CUSTOM);
    let even = rows
        .iter()
        .find(|row| row.pointer("/guid/value").is_some_and(|v| v == &even_guid))
        .ok_or("expected even manifest")?;
    let custom = rows
        .iter()
        .find(|row| {
            row.pointer("/guid/value")
                .is_some_and(|v| v == &custom_guid)
        })
        .ok_or("expected custom manifest")?;
    verify_even_manifest(even)?;
    verify_custom_manifest(custom)?;
    verify_physical_duplicates(rows, &catalog)?;
    let kind = built
        .summary()
        .kind(Kind::CargoManifests)
        .ok_or("manifest summary")?;
    assert_eq!(
        (kind.count, kind.skipped),
        (4, 0),
        "summary counts physical manifests"
    );
    Ok(())
}

fn verify_physical_duplicates(rows: &[Value], catalog: &Value) -> TestResult {
    let random_guid = guid(MANIFEST_RANDOM);
    let duplicates: Vec<_> = rows
        .iter()
        .filter(|row| {
            row.pointer("/guid/value")
                .is_some_and(|v| v == &random_guid)
        })
        .collect();
    let [first, second] = duplicates.as_slice() else {
        return Err("both physical copies of the manifest must survive".into());
    };
    assert_eq!(
        json_at(first, "/recordPath/value")?,
        json_at(second, "/recordPath/value")?,
        "duplicate GUID records retain the same authored path"
    );
    assert_ne!(
        json_at(first, "/recordIndex/value")?,
        json_at(second, "/recordIndex/value")?,
        "duplicate GUID records have distinct physical indices"
    );
    verify_random_manifest(first, catalog)?;
    verify_manifest_text_domains(first)?;
    Ok(())
}

fn verify_even_manifest(row: &Value) -> TestResult {
    assert_eq!(
        json_at(row, "/className/value")?,
        "LegalCargo_Even",
        "authored Class name"
    );
    assert!(
        json_at(row, "/recordIndex/value")?.is_u64(),
        "physical record index"
    );
    assert_eq!(
        json_at(row, "/lockCargoGridsOnSpawn/value")?,
        false,
        "authored lock flag"
    );
    let fill = json_at(row, "/cargoFillCapacity/value")?;
    assert_eq!(
        json_at(fill, "/sourceType/value")?,
        "CargoFillCapacityValue_Even",
        "concrete even fill type"
    );
    assert_eq!(
        json_at(fill, "/minCapacityRange/value/finite")?,
        &json!(0.45),
        "even fill lower bound"
    );
    assert_eq!(
        json_at(fill, "/maxCapacityRange/value/finite")?,
        &json!(0.5),
        "even fill upper bound"
    );
    let [scrap, gold] = json_array(fill, "/resources/value")? else {
        return Err("two resource occurrences in original order".into());
    };
    assert_eq!(
        json_at(scrap, "/value/resource/value/guid/value")?,
        &guid(SCRAP),
        "first authored resource"
    );
    assert_eq!(
        json_at(gold, "/value/resource/value/guid/value")?,
        &guid(GOLD),
        "second authored resource"
    );
    assert_eq!(
        json_at(gold, "/value/probability/value/finite")?,
        &json!(0.25),
        "second resource probability"
    );
    verify_even_spawn_rules(json_array(row, "/cargoSpawnRules/value")?)
}

fn verify_even_spawn_rules(rules: &[Value]) -> TestResult {
    let [entities, resources] = rules else {
        return Err("ordered entity and resource spawn occurrences".into());
    };
    let entity = json_at(entities, "/value")?;
    let resource = json_at(resources, "/value")?;
    assert_eq!(
        json_at(entity, "/sourceType/value")?,
        "CargoSpawnEntityDef",
        "first concrete spawn rule"
    );
    assert_eq!(
        json_at(resource, "/sourceType/value")?,
        "CargoSpawnResourceDef",
        "second concrete spawn rule"
    );
    for (rule, member) in [(entity, "entities"), (resource, "resources")] {
        let [first, second] = json_array(rule, &format!("/{member}/value"))? else {
            return Err(format!("two ordered original {member} references").into());
        };
        assert_eq!(
            json_at(first, "/value/guid/value")?,
            &guid(CRATE),
            "first authored {member} reference"
        );
        assert_eq!(
            json_at(second, "/value/guid/value")?,
            &guid(CRATES),
            "second authored {member} reference"
        );
    }
    assert_eq!(
        json_at(entity, "/resources/state")?,
        "absent",
        "entity rule has no declared resource list"
    );
    let fixed = json_at(entity, "/quantityAlgorithmType/value")?;
    assert_eq!(
        json_at(fixed, "/sourceType/value")?,
        "CargoManifestFixedQuantityDef",
        "fixed algorithm keeps original type"
    );
    assert_eq!(
        json_at(fixed, "/quantity/value")?,
        &json!({"type":"int32","value":30}),
        "fixed quantity preserves Int32 storage"
    );
    let proportion = json_at(
        resource,
        "/quantityAlgorithmType/value/proportion/value/finite",
    )?;
    assert_eq!(
        proportion,
        &json!(0.5),
        "proportion algorithm retains authored value"
    );
    Ok(())
}

fn verify_custom_manifest(row: &Value) -> TestResult {
    let fill = json_at(row, "/cargoFillCapacity/value")?;
    assert_eq!(
        json_at(fill, "/sourceType/value")?,
        "CargoFillCapacityValue_Custom",
        "concrete custom fill type"
    );
    assert_eq!(
        json_at(fill, "/minCapacityRange/state")?,
        "absent",
        "custom fill only declares allocations"
    );
    let [allocation] = json_array(fill, "/resources/value")? else {
        return Err("one original custom allocation".into());
    };
    let allocation = json_at(allocation, "/value")?;
    for (field, expected) in [
        ("minResourceAllocationPercentage", 0.03),
        ("maxResourceAllocationPercentage", 0.06),
    ] {
        assert_eq!(
            json_at(allocation, &format!("/{field}/value/finite"))?,
            &json!(expected),
            "original allocation {field}"
        );
    }
    let resource = json_at(allocation, "/resources/value")?;
    assert_eq!(
        json_at(resource, "/resource/value/guid/value")?,
        &guid(GOLD),
        "allocated resource identity"
    );
    assert_eq!(
        json_at(resource, "/probability/value/finite")?,
        &json!(1.0),
        "allocated resource probability"
    );
    assert!(
        json_array(row, "/cargoSpawnRules/value")?.is_empty(),
        "declared spawn-rule array is present and empty"
    );
    Ok(())
}

fn verify_manifest_text_domains(row: &Value) -> TestResult {
    let values = json_array(row, "/authoredFields/values")?;
    for (storage, source_text) in [
        ("string", "synthetic-authored-tag"),
        ("locale", "@synthetic_manifest_description"),
    ] {
        let protected = json!({"type": "gameText", "sourceText": source_text});
        assert!(
            values.iter().any(|node| {
                node.pointer("/value/value/type").and_then(Value::as_str) == Some(storage)
                    && node.pointer("/value/value/value") == Some(&protected)
                    && node
                        .pointer("/value/containsEncryptedSource")
                        .and_then(Value::as_bool)
                        == Some(true)
            }),
            "authored {storage} keeps exact encrypted text in the protected domain"
        );
    }
    Ok(())
}

fn verify_random_manifest(row: &Value, catalog: &Value) -> TestResult {
    let fill = json_at(row, "/cargoFillCapacity/value")?;
    assert_eq!(
        json_at(fill, "/sourceType/value")?,
        "CargoFillCapacityValue_Random",
        "concrete random fill type"
    );
    assert_eq!(
        json_at(fill, "/minCapacityRange/value/rawBits")?,
        "80000000",
        "negative zero retains its bit pattern"
    );
    assert_eq!(
        json_at(fill, "/maxCapacityRange/value/rawBits")?,
        "7fc0002a",
        "non-finite value retains its exact NaN payload"
    );
    assert!(
        json_at(fill, "/maxCapacityRange/value/finite")?.is_null(),
        "non-finite source has no decimal"
    );
    let [future] = json_array(row, "/cargoSpawnRules/value")? else {
        return Err("an unrecognized authored spawn rule must be retained".into());
    };
    assert_eq!(
        json_at(future, "/value/sourceType/value")?,
        "CargoSpawnFutureDef",
        "unrecognized rule preserves the concrete type"
    );
    assert_eq!(
        json_at(future, "/value/authoredFields/roots/0/name/value")?,
        "futureQuota",
        "unrecognized authored field keeps its exact name"
    );
    let value = json_at(future, "/value/authoredFields/values/0/value")?;
    assert_eq!(
        json_at(value, "/value/value")?,
        &json!({"type":"uint64","value":"18446744073709551615"}),
        "opaque quota preserves its unsigned 64-bit value"
    );
    assert_eq!(
        json_at(value, "/containsEncryptedSource")?,
        true,
        "quota provenance marks encrypted DataForge"
    );
    let uses = json_array(value, "/sourceUsesRefs")?;
    let inventory = json_array(catalog, "/provenance/uses")?;
    assert!(
        !uses.is_empty()
            && uses
                .iter()
                .all(|id| inventory.iter().any(|entry| entry.get("id") == Some(id))),
        "the retained original quota has immediate encrypted provenance and resolvable uses"
    );
    Ok(())
}
