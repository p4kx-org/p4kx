// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Canonical resource packaging preserves source absence, explicit nulls and authored links.

use p4kx_export::CatalogPublicationMode;
use serde_json::{Value, json};

use crate::health_reference_json as health;

use crate::catalog_crafting_support;
use crate::resource_1069_fixture::{self as fixture, Packaging, Validation};
use crate::resource_origin_fixture::TestResult;

#[test]
fn missing_null_and_empty_owned_packaging_remain_distinct() -> TestResult {
    for (shape, state, value) in [
        (Packaging::Absent, "absent", Value::Null),
        (Packaging::Null, "null", Value::Null),
        (Packaging::Empty, "present", json!([])),
    ] {
        let (_, catalog) =
            fixture::build(&fixture::packaging(shape), CatalogPublicationMode::Normal)?;
        let packaging = fixture::row(&catalog)?
            .get("defaultCargoContainers")
            .ok_or("packaging carrier")?;
        assert_eq!(packaging["state"], state, "source presence is explicit");
        assert_eq!(
            packaging["value"], value,
            "empty packaging is a present empty list"
        );
        assert_eq!(packaging["containsEncryptedSource"], false);
        assert_eq!(packaging["sourceSetComplete"], true);
        assert!(
            packaging
                .get("sourceUsesRefs")
                .and_then(Value::as_array)
                .is_some_and(|uses| !uses.is_empty()),
            "presence and absence both retain the inspected source"
        );
        assert_eq!(catalog["exportHealth"]["status"], "complete");
    }
    Ok(())
}

#[test]
fn missing_explicit_null_and_false_validation_do_not_collapse() -> TestResult {
    for (shape, state, value) in [
        (Validation::Absent, "absent", Value::Null),
        (Validation::Null, "null", Value::Null),
        (Validation::False, "present", json!(false)),
    ] {
        let (_, catalog) =
            fixture::build(&fixture::validation(shape), CatalogPublicationMode::Normal)?;
        let validation = fixture::row(&catalog)?
            .get("validateDefaultCargoBox")
            .ok_or("validation carrier")?;
        assert_eq!(validation["state"], state, "null is independent of false");
        assert_eq!(validation["value"], value);
        assert_eq!(validation["containsEncryptedSource"], false);
        assert_eq!(validation["sourceSetComplete"], true);
        assert_eq!(catalog["exportHealth"]["status"], "complete");
    }
    Ok(())
}

#[test]
fn dangling_cargo_slots_keep_authored_order_guids_and_status() -> TestResult {
    let (_, catalog) = fixture::build(
        &fixture::packaging(Packaging::Dangling),
        CatalogPublicationMode::Normal,
    )?;
    let slots = fixture::row(&catalog)?
        .pointer("/defaultCargoContainers/value")
        .and_then(Value::as_array)
        .ok_or("authored cargo slots")?;
    let names: Option<Vec<_>> = slots
        .iter()
        .map(|slot| slot.pointer("/value/slot/value").and_then(Value::as_str))
        .collect();
    assert_eq!(names, Some(vec!["twoSCU", "one_eighthSCU", "oneSCU"]));
    assert_eq!(
        slots.len(),
        3,
        "neither repeated GUIDs nor the null slot are removed"
    );
    for index in [0, 2] {
        assert_dangling_slot(slots.get(index).ok_or("dangling slot")?)?;
    }
    let null_slot = slots
        .get(1)
        .and_then(|slot| slot.pointer("/value/entityClass"))
        .ok_or("explicit null slot")?;
    assert_eq!(null_slot["state"], "null");
    assert_eq!(null_slot["value"], Value::Null);
    assert_unresolved_boxes(&catalog)?;
    assert_eq!(catalog["exportHealth"]["status"], "complete");
    Ok(())
}

fn assert_dangling_slot(slot: &Value) -> TestResult {
    let link = slot.pointer("/value/entityClass").ok_or("box reference")?;
    assert_eq!(
        link["state"], "present",
        "the authored reference is retained"
    );
    assert_eq!(
        link.pointer("/value/status").and_then(Value::as_str),
        Some("dangling"),
        "an unresolved target keeps its explicit dangling status"
    );
    let expected_guid = catalog_crafting_support::text(fixture::MISSING_BOX);
    assert_eq!(
        link.pointer("/value/guid/value").and_then(Value::as_str),
        Some(expected_guid.as_str()),
        "a missing target cannot replace its authored GUID"
    );
    assert_eq!(
        link.pointer("/value/guid/state").and_then(Value::as_str),
        Some("present"),
        "the authored GUID remains present independently of target resolution"
    );
    for field in ["targetRecordIndex", "recordPath", "targetType"] {
        let target = link
            .get("value")
            .and_then(|value| value.get(field))
            .ok_or("nullable target identity carrier")?;
        assert_eq!(target["state"], "absent", "no target identity is invented");
        assert_eq!(
            target.get("value"),
            Some(&Value::Null),
            "the absent target has an explicit null value"
        );
    }
    assert_eq!(
        link["containsEncryptedSource"], false,
        "the authored reference comes from the known plain input"
    );
    assert_eq!(
        link["sourceSetComplete"], true,
        "the retained reference has complete source evidence"
    );
    Ok(())
}

fn assert_unresolved_boxes(catalog: &Value) -> TestResult {
    let unresolved = catalog
        .pointer("/exportHealth/unresolvedReferences")
        .and_then(Value::as_array)
        .ok_or("unresolved box references")?;
    for field in [
        "defaultCargoContainers.twoSCU",
        "defaultCargoContainers.oneSCU",
    ] {
        let reference = health::by_field(catalog, unresolved, field)?;
        assert_eq!(
            reference["status"], "dangling",
            "the exact slot has an unresolved target"
        );
        assert_eq!(
            health::present_value(reference, "recordIndex")?,
            &json!(0),
            "the physical source owner survives"
        );
        assert_eq!(
            health::present_value(reference, "recordGuid")?,
            &json!(catalog_crafting_support::text(fixture::RESOURCE)),
            "the owning resource identity survives"
        );
        assert_eq!(
            health::present_value(reference, "targetGuid")?,
            &json!(catalog_crafting_support::text(fixture::MISSING_BOX)),
            "the unresolved target GUID survives independently"
        );
        assert_eq!(
            health::absent_value(reference, "targetRecordIndex")?,
            &Value::Null,
            "an unresolved target has no invented physical index"
        );
    }
    Ok(())
}
