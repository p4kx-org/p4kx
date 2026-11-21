// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::Kind;
use p4kx_extraction::DataForgeEntry;
use serde_json::{Value, json};

use super::{TestResult, check, definition_identity};
use crate::catalog_v3_consumer as reader;
use crate::commodity_v3_fixture::DcbOrigin;
use crate::real_archive_equipment_catalog::{owner_bindings, owner_rows};
use crate::real_archive_verification::check_unique_guids;

#[path = "real_archive_item_identity_fixture.rs"]
mod fixture;

const TABLE: &str = "/configurations/itemAttachmentConstraints/rows";
const DEFINITIONS: &str = "/definitions/items/rows";

#[test]
fn distinct_same_guid_items_keep_actual_record_headers_and_component_slots() -> TestResult {
    for (origin, marker) in [
        (DcbOrigin::Plain, json!(false)),
        (DcbOrigin::Encrypted, json!(true)),
        (DcbOrigin::Unknown, Value::Null),
    ] {
        let fixture = fixture::read(origin, false)?;
        check_unique_guids(&fixture.catalog)?;
        let entry = DataForgeEntry::read(&fixture.archive)?;
        let forge = entry.open()?;
        definition_identity::check_original_items(&fixture.catalog, &forge)?;
        let [first, second] = two(&fixture.catalog)?;
        assert_eq!(
            first.pointer("/guid/value"),
            second.pointer("/guid/value"),
            "actual original aliases have the same nominal identity"
        );
        for (item, expected) in [(first, 0), (second, 1)] {
            assert_eq!(
                item.pointer("/recordIndex/value"),
                Some(&json!(expected)),
                "each row keeps its actual physical Record"
            );
            assert_eq!(
                item.pointer("/recordIndex/containsEncryptedSource"),
                Some(&marker),
                "plain, encrypted and unknown source states remain independent"
            );
            owner_bindings::check(&fixture.catalog, TABLE, item)?;
            let positions = owner_rows(&fixture.catalog, TABLE, item)
                .map(|row| {
                    row.pointer("/componentOrdinal/value")
                        .and_then(Value::as_u64)
                        .ok_or("original component ordinal")
                })
                .collect::<Result<Vec<_>, _>>()?;
            assert_eq!(
                positions,
                [0, 1],
                "only this actual owner's repeated components"
            );
        }
    }
    Ok(())
}

#[test]
fn physical_item_identity_rejects_repeated_owners_in_either_wire_family() -> TestResult {
    let fixture = fixture::read(DcbOrigin::Plain, false)?;
    for path in ["/items", DEFINITIONS] {
        let mut repeated = fixture.catalog.clone();
        let rows = repeated
            .pointer_mut(path)
            .and_then(Value::as_array_mut)
            .ok_or("actual original Item family")?;
        let first = rows.first().ok_or("first original owner")?.clone();
        rows.push(first);
        assert!(
            check_unique_guids(&repeated).is_err(),
            "one physical owner cannot be repeated in {path}"
        );
    }
    Ok(())
}

#[test]
fn matching_wire_rows_cannot_invent_an_original_physical_index() -> TestResult {
    let fixture = fixture::read(DcbOrigin::Plain, false)?;
    let mut forged = fixture.catalog;
    change(&mut forged, "/items/1/recordIndex/value", json!(999))?;
    change(
        &mut forged,
        "/definitions/items/rows/1/recordIndex/value",
        json!(999),
    )?;
    definition_identity::check(&forged, Kind::Items)?;
    definition_identity::check_rows(reader::table(&forged, "/definitions/items")?, Kind::Items)?;
    let error = check(&forged)
        .err()
        .ok_or("a false physical original must fail")?;
    assert!(
        error.to_string().contains("original Record header"),
        "matching payload numbers do not replace the original Header use: {error}"
    );
    Ok(())
}

#[test]
fn item_aliases_preserve_source_family_and_actual_original_metadata_checks() -> TestResult {
    let fixture = fixture::read(DcbOrigin::Plain, false)?;
    let entry = DataForgeEntry::read(&fixture.archive)?;
    let forge = entry.open()?;
    let mut wrong = fixture.catalog.clone();
    for path in [
        "/items/1/sourceType/value",
        "/definitions/items/rows/1/sourceType/value",
    ] {
        change(&mut wrong, path, json!("BlueprintCategoryRecord"))?;
    }
    assert!(
        check_unique_guids(&wrong).is_err(),
        "only the actual Entity source family admits physical Item aliases"
    );
    let mut metadata = fixture.catalog;
    for path in [
        "/items/1/className/value",
        "/definitions/items/rows/1/className/value",
    ] {
        change(&mut metadata, path, json!("InventedClass"))?;
    }
    check_unique_guids(&metadata)?;
    assert!(
        definition_identity::check_original_items(&metadata, &forge).is_err(),
        "the actual original Record still controls derived header meaning"
    );
    let collision = fixture::read(DcbOrigin::Plain, true)?;
    let error = check_unique_guids(&collision.catalog)
        .err()
        .ok_or("cross-family collision remains forbidden")?;
    assert!(
        error.to_string().contains("occurs twice"),
        "physical aliases do not waive the existing cross-family rule: {error}"
    );
    Ok(())
}

#[test]
fn equipment_joins_reject_foreign_header_proof_and_repeated_component_identity() -> TestResult {
    let fixture = fixture::read(DcbOrigin::Plain, false)?;
    let [_, second] = two(&fixture.catalog)?;
    let second_guid = second.get("guid").ok_or("second original Header")?.clone();
    let mut wrong = fixture.catalog.clone();
    change(&mut wrong, &format!("{TABLE}/0/ownerGuid"), second_guid)?;
    change(
        &mut wrong,
        &format!("{TABLE}/0/ownerRecordIndex/value"),
        json!(1),
    )?;
    let [_, second] = two(&wrong)?;
    assert!(
        owner_bindings::check(&wrong, TABLE, second).is_err(),
        "same numeric Record and GUID require the same actual source cells"
    );
    let mut repeated = fixture.catalog;
    change(
        &mut repeated,
        &format!("{TABLE}/1/componentOrdinal/value"),
        json!(0),
    )?;
    let [first, _] = two(&repeated)?;
    assert!(
        owner_bindings::check(&repeated, TABLE, first).is_err(),
        "a second row cannot reuse the same original component position"
    );
    Ok(())
}

#[test]
fn unknown_encryption_does_not_waive_incomplete_physical_index_evidence() -> TestResult {
    let fixture = fixture::read(DcbOrigin::Plain, false)?;
    let mut incomplete = fixture.catalog;
    for prefix in [
        "/items/1/recordIndex",
        "/definitions/items/rows/1/recordIndex",
    ] {
        change(
            &mut incomplete,
            &format!("{prefix}/sourceSetComplete"),
            json!(false),
        )?;
        change(
            &mut incomplete,
            &format!("{prefix}/containsEncryptedSource"),
            Value::Null,
        )?;
    }
    let error = check(&incomplete)
        .err()
        .ok_or("incomplete physical index must fail")?;
    assert!(
        error.to_string().contains("must be complete"),
        "unknown encryption remains distinct from missing physical identity evidence: {error}"
    );
    Ok(())
}

fn two(catalog: &Value) -> TestResult<[&Value; 2]> {
    let [first, second] = reader::rows(catalog, Kind::Items)?.as_slice() else {
        return Err("two independently produced physical Item aliases".into());
    };
    Ok([first, second])
}

fn change(document: &mut Value, path: &str, value: Value) -> TestResult {
    *document
        .pointer_mut(path)
        .ok_or("actual original consumer field")? = value;
    Ok(())
}
