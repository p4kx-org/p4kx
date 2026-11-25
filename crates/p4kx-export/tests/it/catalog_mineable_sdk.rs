// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection,
    with_mineable_entities_from,
};
use p4kx_export::{CatalogOptions, CatalogPublicationMode, build_catalog};
use p4kx_extraction::DataForgeEntry;
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::health_origin_assertions as proof;
use crate::sdk_full_proof as full_proof;

#[path = "catalog_mineable_sdk_fixture.rs"]
mod fixture;

const TABLE: &str = "/definitions/mineableEntities/rows";

#[test]
fn mineable_sdk_replays_catalog_rows_and_their_original_contributors() -> TestResult {
    let database = fixture::database();
    for (origin, encrypted) in [(DcbOrigin::Plain, false), (DcbOrigin::Encrypted, true)] {
        let (_folder, archive) = common::open(&database, origin)?;
        let options = CatalogOptions {
            publication_mode: CatalogPublicationMode::DiagnosticPartial,
            ..common::options()
        };
        let catalog: Value = serde_json::from_slice(
            build_catalog(&archive, options.with_full_provenance())?.json(),
        )?;
        assert!(
            catalog.get("mineableEntities").is_none(),
            "mineable entities exist only in the canonical definitions table"
        );
        full_proof::open(&archive, full_proof::NO_LANGUAGES, None)?.with_mineable_entities(
            CatalogRecordSelection::All,
            None,
            |snapshot| -> TestResult {
                assert!(
                    snapshot.coverage().incomplete_fields.contains(&1),
                    "malformed source fields affect the owning physical record"
                );
                assert_eq!(
                    snapshot.requested_languages(),
                    [],
                    "the original selector needs no INI"
                );
                let inventory = json!({"provenance": snapshot.source_inventory()});
                let actual = serde_json::to_value(snapshot.rows())?;
                assert_eq!(
                    proof::with_origins(actual.clone(), &inventory)?,
                    proof::with_origins(proof::at(&catalog, TABLE)?.clone(), &catalog)?,
                    "the two readers expose the same rows and source coordinates"
                );
                assert_eq!(
                    owner_indices(&actual)?,
                    [1, 0, 2],
                    "class and record index order preserve all admitted owners"
                );
                check_mixed_signatures(proof::at(&actual, "/1")?, &inventory, encrypted)?;
                check_states(proof::at(&actual, "/0")?)?;
                Ok(())
            },
        )??;
    }
    Ok(())
}

#[test]
fn mineable_sdk_replays_every_physical_guid_alias_and_excludes_foreign_records() -> TestResult {
    let database = fixture::database();
    let (_folder, archive) = common::open(&database, DcbOrigin::Encrypted)?;
    let entry = DataForgeEntry::read(&archive)?;
    let guid = common::guid(fixture::DUPLICATE).parse()?;
    with_mineable_entities_from(
        &archive,
        &entry,
        CatalogRecordSelection::Guid(guid),
        None,
        |snapshot| check_guid_aliases(&serde_json::to_value(snapshot.rows())?),
    )??;
    let session = CatalogQuerySession::from_entry(
        &archive,
        &entry,
        CatalogLanguageSelection::AllAvailable,
        None,
    )?;
    session.parsed(None)?.with_mineable_entities(
        CatalogRecordSelection::RecordIndex(2),
        None,
        |snapshot| -> TestResult {
            let rows = serde_json::to_value(snapshot.rows())?;
            assert_eq!(
                owner_indices(&rows)?,
                [2],
                "physical selection returns exactly the requested owner"
            );
            assert_eq!(
                snapshot.requested_languages(),
                [],
                "the parsed session needs no language strings for the original selector"
            );
            Ok(())
        },
    )??;
    Ok(())
}

fn check_guid_aliases(rows: &Value) -> TestResult {
    assert_eq!(
        owner_indices(rows)?,
        [0, 2],
        "GUID selection includes both physical owners and excludes the foreign owner"
    );
    let empty = proof::at(rows, "/1")?;
    assert_eq!(
        proof::at(rows, "/0/guid/value")?,
        proof::at(empty, "/guid/value")?,
        "the two physical owners share their original GUID"
    );
    assert_eq!(
        proof::at(empty, "/componentSlots/value")?,
        &json!(0),
        "the physical owner retains its explicit empty slot count"
    );
    let no_components: &[Value] = &[];
    assert_eq!(
        proof::rows(empty, "/radar")?,
        no_components,
        "empty radar projection"
    );
    assert_eq!(
        proof::rows(empty, "/mining")?,
        no_components,
        "empty mining projection"
    );
    Ok(())
}

fn owner_indices(rows: &Value) -> TestResult<Vec<u64>> {
    rows.as_array()
        .ok_or("mineable rows")?
        .iter()
        .map(|row| {
            row.pointer("/recordIndex/value")
                .and_then(Value::as_u64)
                .ok_or_else(|| "physical mineable record index".into())
        })
        .collect()
}

fn check_mixed_signatures(row: &Value, inventory: &Value, encrypted: bool) -> TestResult {
    proof::present(row, "componentSlots", &json!(5), Some(encrypted))?;
    check_mining_components(row)?;
    check_radar_components(row, inventory, encrypted)
}

fn check_mining_components(row: &Value) -> TestResult {
    let mining = proof::rows(row, "/mining")?;
    let positions: Vec<_> = mining
        .iter()
        .map(|slot| proof::at(slot, "/sourcePosition/value"))
        .collect::<TestResult<_>>()?;
    assert_eq!(
        positions,
        [&json!(0), &json!(3), &json!(4)],
        "projection preserves the non-contiguous physical positions of mining components"
    );
    let compositions: Vec<_> = mining
        .iter()
        .map(|slot| proof::at(slot, "/composition/value/targetRecordIndex/value"))
        .collect::<TestResult<_>>()?;
    assert_eq!(
        compositions,
        [&json!(4), &json!(5), &json!(4)],
        "projection preserves repeated links and distinct compositions"
    );
    let second_mining = proof::at(row, "/mining/1")?;
    assert_eq!(
        proof::at(second_mining, "/globalParams/value/targetRecordIndex/value")?,
        &json!(6),
        "the second component retains its link to the original global parameters"
    );
    let calibration = authored_node(
        proof::at(second_mining, "/authoredFields")?,
        "fixtureCalibration",
    )?
    .ok_or("the original calibration field")?;
    assert_eq!(
        proof::at(calibration, "/value/value")?,
        &json!({"type": "int64", "value": "9007199254740993"}),
        "the authored calibration preserves integer precision beyond the JSON safe integer range"
    );
    Ok(())
}

fn check_radar_components(row: &Value, inventory: &Value, encrypted: bool) -> TestResult {
    assert_eq!(
        proof::at(row, "/radar/0/sourcePosition/value")?,
        &json!(2),
        "the radar component preserves its position among mining components"
    );
    let properties = proof::at(row, "/radar/0/properties/value")?;
    let tagged = authored_node(
        proof::at(properties, "/authoredFields")?,
        "taggedEmissionModifierParams",
    )?
    .ok_or("an original empty modifier array")?;
    assert_eq!(
        proof::at(tagged, "/state")?,
        &json!("present"),
        "an explicitly empty modifier array is present"
    );
    assert_eq!(
        proof::at(tagged, "/value")?,
        &json!({"type": "array", "elements": []}),
        "the array preserves its declared type with no elements"
    );
    let base = proof::at(properties, "/baseSignatureParams/value")?;
    let slots = proof::at(base, "/signatures/value")?;
    assert_eq!(
        slots.as_array().ok_or("source signature slots")?.len(),
        4,
        "all source signature positions are preserved"
    );
    assert_eq!(
        proof::at(slots, "/0/state")?,
        &json!("present"),
        "a non-finite number is present as a lossless source value"
    );
    assert_eq!(
        proof::at(slots, "/0/value/rawBits")?,
        &json!(format!("{:08x}", fixture::NAN_BITS)),
        "the NaN payload retains its exact Single bits"
    );
    assert_eq!(
        proof::at(slots, "/0/value/finite")?,
        &Value::Null,
        "the NaN payload has no finite projection"
    );
    assert_eq!(
        proof::at(slots, "/1/value/rawBits")?,
        &json!("80000000"),
        "negative zero retains its sign bit"
    );
    let scan = proof::present(base, "scanSignature", &json!(4270.0), Some(encrypted))?;
    check_scan_uses(inventory, scan, proof::at(slots, "/2")?)
}

fn check_scan_uses(inventory: &Value, scan: &Value, selected: &Value) -> TestResult {
    let uses = proof::uses(inventory, scan)?;
    assert!(
        uses.iter().any(|used| {
            used.get("role").and_then(Value::as_str) == Some("selector")
                && used.pointer("/location/namespace") == Some(&json!("dataforgeValue"))
                && used.pointer("/location/indices") == Some(&json!(["8", "7"]))
                && used.pointer("/location/field") == Some(&json!("$value"))
        }),
        "the third original global signature type's displayName selects the slot"
    );
    let signature_uses = proof::uses(inventory, selected)?;
    assert!(
        signature_uses.iter().any(|used| {
            used.get("role").and_then(Value::as_str) == Some("payload")
                && used.pointer("/location/indices/0") == Some(&json!("0"))
                && uses.contains(used)
        }),
        "the exact original Single also contributes to the derived scan signature"
    );
    Ok(())
}

fn check_states(row: &Value) -> TestResult {
    let radar = proof::rows(row, "/radar")?;
    assert_eq!(
        radar.len(),
        4,
        "all four original radar components remain separate"
    );
    for (component, state) in radar
        .iter()
        .zip(["absent", "present", "invalid", "unreadable"])
    {
        let properties = proof::at(component, "/properties/value")?;
        assert!(
            authored_node(
                proof::at(properties, "/authoredFields")?,
                "taggedEmissionModifierParams"
            )?
            .is_none(),
            "the absent modifier property is not synthesized into the authored fields"
        );
        let base = proof::at(properties, "/baseSignatureParams/value")?;
        let signatures = proof::cell(base, "signatures", state)?;
        if state == "present" {
            assert_eq!(
                proof::at(signatures, "/value")?,
                &json!([]),
                "the available empty signature array is retained as empty"
            );
        }
        proof::cell(base, "scanSignature", "absent")?;
    }
    Ok(())
}

fn authored_node<'a>(arena: &'a Value, name: &str) -> TestResult<Option<&'a Value>> {
    let Some(field) = proof::rows(arena, "/roots")?
        .iter()
        .find(|root| root.pointer("/name/value").and_then(Value::as_str) == Some(name))
    else {
        return Ok(None);
    };
    let index: usize = proof::at(field, "/value/valueIndex")?
        .as_str()
        .ok_or("authored value index")?
        .parse()?;
    let node = proof::rows(arena, "/values")?
        .get(index)
        .ok_or("closed authored value index")?;
    proof::at(node, "/value").map(Some)
}
