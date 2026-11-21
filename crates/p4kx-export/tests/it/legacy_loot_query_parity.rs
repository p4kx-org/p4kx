// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::loot::LegacyLootSnapshot;
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, DatasetAvailability,
    ParsedCatalogQueries,
};
use serde::Serialize;
use serde_json::{Value, json};

use crate::health_origin_assertions as proof;
use crate::health_origin_fixture::{Fixture, Input, TestResult};
use crate::{loot_fixture, starmap_fixture};

#[test]
fn legacy_loot_queries_keep_both_physical_owners_and_their_actual_datum_origins() -> TestResult {
    let bytes = loot_fixture::database_with_legacy_loot_alias().build();
    for input in [Input::Plain, Input::Encrypted] {
        let fixture = loot_fixture::source_fixture(&bytes, input)?;
        let (_, catalog) = fixture.diagnostic()?;
        let session = CatalogQuerySession::from_entry(
            &fixture.archive,
            &fixture.entry,
            CatalogLanguageSelection::Selected(&[]),
            None,
        )?;
        let parsed = session.parsed(None)?;
        compare_tables(&fixture, &parsed, &catalog)?;
        parsed.with_legacy_loot_archetypes(CatalogRecordSelection::All, None, |snapshot| {
            compare(&snapshot, &catalog, "/legacyLootArchetypes", 1)
        })??;
        assert_term_origins(&catalog, input)?;
    }
    Ok(())
}

fn compare_tables(
    fixture: &Fixture,
    parsed: &ParsedCatalogQueries<'_>,
    catalog: &Value,
) -> TestResult {
    let guid = starmap_fixture::guid_text(loot_fixture::LEGACY_TABLE).parse()?;
    let all = parsed.with_legacy_loot_tables(
        CatalogRecordSelection::Guid(guid),
        None,
        |snapshot| compare(&snapshot, catalog, "/legacyLootTables", 2),
    )??;
    let [first, second] = all.as_slice() else {
        return Err("two reconstructed physical owners".into());
    };
    assert!(
        first < second,
        "equal nominal GUIDs sort on the physical Record index"
    );
    for index in all {
        let got = parsed.with_legacy_loot_tables(
            CatalogRecordSelection::RecordIndex(index),
            None,
            |snapshot| compare(&snapshot, catalog, "/legacyLootTables", 1),
        )??;
        assert_eq!(
            got,
            [index],
            "each reconstructed row resolves to its original slot"
        );
    }
    let forge = fixture.entry.open()?;
    let outside = forge
        .records()
        .find(|record| record.record_path().ok() == Some("unselected/loottables/legacy_outside"))
        .ok_or("wrong-path original owner")?;
    parsed.with_legacy_loot_tables(
        CatalogRecordSelection::RecordIndex(outside.index()),
        None,
        |snapshot| {
            assert!(
                snapshot.rows.is_empty(),
                "a legacy type outside the source path family stays excluded"
            );
        },
    )?;
    Ok(())
}

fn compare<Row: Serialize>(
    snapshot: &LegacyLootSnapshot<Row>,
    catalog: &Value,
    path: &str,
    expected: usize,
) -> TestResult<Vec<u32>> {
    assert!(
        matches!(snapshot.availability, DatasetAvailability::Available),
        "original family is available"
    );
    assert_eq!(
        snapshot.rows.len(),
        expected,
        "both SDK selection and cardinality are observed"
    );
    let original = proof::rows(catalog, path)?;
    let provenance = json!({"provenance": snapshot.source_inventory});
    let mut indices = Vec::new();
    for row in &snapshot.rows {
        let selected = serde_json::to_value(row)?;
        let index = proof::at(&selected, "/recordIndex/value")?;
        let expected_row = original
            .iter()
            .find(|row| row.pointer("/recordIndex/value") == Some(index))
            .ok_or("the same physical owner in normal Catalog production")?;
        indices.push(u32::try_from(
            index.as_u64().ok_or("physical index integer")?,
        )?);
        assert_eq!(
            proof::with_origins(selected, &provenance)?,
            proof::with_origins(expected_row.clone(), catalog)?,
            "the same-product rows match after resolving all source uses"
        );
    }
    Ok(indices)
}

fn assert_term_origins(catalog: &Value, input: Input) -> TestResult {
    let [original, alias] = proof::rows(catalog, "/legacyLootTables")? else {
        return Err("two physical legacy table owners".into());
    };
    let weight = proof::at(original, "/entries/value/0/value/weight")?;
    let missing = proof::at(original, "/entries/value/1/value/archetypeGuid")?;
    let malformed = proof::at(alias, "/entries/value/0/value/archetypeGuid")?;
    assert_eq!(
        proof::at(weight, "/containsEncryptedSource")?,
        &json!(input.marker()),
        "actual DataForge marker on a scalar"
    );
    assert_eq!(
        proof::at(missing, "/state")?,
        "null",
        "original null is distinguishable"
    );
    assert_dangling_reference(catalog, alias, malformed)?;
    let locations = proof::uses(catalog, weight)?;
    let owner = proof::at(original, "/recordIndex/value")?.to_string();
    let location = locations
        .iter()
        .find_map(|used| {
            let location = used.get("location")?;
            (location.get("namespace")?.as_str() == Some("dataforgeValue")
                && location.get("field")?.as_str() == Some("$value")
                && used.get("role")?.as_str() == Some("payload"))
            .then_some(location)
        })
        .ok_or("original weight value use")?;
    assert_eq!(
        proof::at(location, "/indices/0")?,
        &json!(owner),
        "operand location binds this physical Record"
    );
    assert_raw_weight(original, catalog, location)
}

fn assert_dangling_reference(catalog: &Value, row: &Value, cell: &Value) -> TestResult {
    let guid = json!(starmap_fixture::guid_text(u128::MAX));
    assert_eq!(
        proof::at(cell, "/state")?,
        "present",
        "the typed link retains the actual authored target"
    );
    assert_eq!(
        proof::at(cell, "/value")?,
        &guid,
        "the missing target has its authored GUID"
    );
    assert_eq!(
        proof::at(cell, "/sourceSetComplete")?,
        false,
        "the attempted reference resolution remains incomplete"
    );
    let uses = proof::uses(catalog, cell)?;
    let payload = uses
        .iter()
        .find(|used| {
            used.pointer("/location/field").and_then(Value::as_str) == Some("$error.guid")
                && used.get("role").and_then(Value::as_str) == Some("payload")
        })
        .ok_or("authored dangling GUID payload")?;
    let indices = proof::at(payload, "/location/indices")?;
    assert_lookup_attempt(catalog, &uses, indices)?;
    let owner = proof::at(row, "/recordIndex/value")?.to_string();
    assert_eq!(
        proof::at(indices, "/0")?,
        &json!(owner),
        "the dangling reference belongs to the second physical owner"
    );
    let ordinal = proof::at(indices, "/1")?
        .as_str()
        .ok_or("original reference occurrence ordinal")?;
    let node = proof::rows(row, "/values")?
        .get(ordinal.parse::<usize>()?)
        .ok_or("original dangling occurrence")?;
    assert_eq!(
        proof::at(node, "/value/state")?,
        "unreadable",
        "the canonical original Record lookup remains unresolved"
    );
    assert_eq!(
        proof::at(node, "/originalError/guid/value")?,
        &guid,
        "the original error retains its exact target GUID"
    );
    Ok(())
}

fn assert_lookup_attempt(catalog: &Value, uses: &[&Value], indices: &Value) -> TestResult {
    let is_attempt = |used: &Value| {
        used.pointer("/location/field").and_then(Value::as_str) == Some("$value")
            && used.get("role").and_then(Value::as_str) == Some("attempted")
            && used.pointer("/location/indices") == Some(indices)
    };
    assert!(
        proof::rows(catalog, "/provenance/uses")?
            .iter()
            .any(is_attempt),
        "the inventory keeps the attempted lookup at this occurrence"
    );
    assert!(
        !uses.iter().copied().any(is_attempt),
        "the attempted lookup is not a contributing source use"
    );
    Ok(())
}

fn assert_raw_weight(row: &Value, catalog: &Value, location: &Value) -> TestResult {
    let ordinal = proof::at(location, "/indices/1")?
        .as_str()
        .ok_or("original occurrence ordinal")?;
    let node = proof::rows(row, "/values")?
        .get(ordinal.parse::<usize>()?)
        .ok_or("arena occurrence")?;
    assert_eq!(
        proof::at(node, "/value/value/value")?,
        &proof::single_number(2.0)?,
        "exact location addresses the original weighted number"
    );
    assert!(
        proof::uses(catalog, proof::at(node, "/value")?)?
            .iter()
            .any(|used| { used.get("location") == Some(location) }),
        "the entered weighted cell rejoins the original arena occurrence"
    );
    Ok(())
}
