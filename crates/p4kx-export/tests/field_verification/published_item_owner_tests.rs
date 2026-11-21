// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value, json};

use super::{fixture, reference, verify};

#[test]
fn canonical_item_headers_require_immediate_provenance_before_guid_join() -> super::super::Result<()>
{
    let fixture = reference()?;
    let mut catalog = fixture::catalog()?;
    let guid = catalog
        .pointer_mut("/items/0/guid")
        .ok_or("actual Item GUID cell")?;
    let value = guid.get("value").ok_or("original GUID value")?.clone();
    *guid = value;
    let error = verify(&fixture, &catalog)
        .err()
        .ok_or("raw GUID must not pass sourced Item validation")?;
    assert!(
        error.to_string().contains("source-set completeness"),
        "the immediate-origin contract stays enforced: {error}"
    );
    Ok(())
}

#[test]
fn component_owner_record_and_source_receipt_must_match_the_actual_item_header()
-> super::super::Result<()> {
    let fixture = reference()?;
    for field in ["ownerRecordIndex", "ownerGuid"] {
        let mut catalog = fixture::catalog()?;
        let cell = catalog
            .pointer_mut(&format!("/configurations/shieldGenerators/rows/0/{field}"))
            .ok_or("actual physical component owner cell")?;
        if field == "ownerRecordIndex" {
            *cell.get_mut("value").ok_or("record index payload")? = json!(2);
        } else {
            *cell
                .get_mut("sourceUsesRefs")
                .ok_or("original GUID receipt")? = json!([]);
        }
        let error = verify(&fixture, &catalog)
            .err()
            .ok_or("foreign physical owner or header receipt must fail")?;
        assert!(
            error.to_string().contains("original Item header"),
            "matching a nominal GUID cannot replace its source owner: {error}"
        );
    }
    Ok(())
}

#[test]
fn duplicate_physical_items_and_nominal_alias_comparisons_remain_explicitly_rejected()
-> super::super::Result<()> {
    let fixture = reference()?;
    let mut catalog = fixture::catalog()?;
    let rows = catalog
        .get_mut("items")
        .and_then(Value::as_array_mut)
        .ok_or("canonical Item rows")?;
    rows.push(rows.first().ok_or("first physical owner")?.clone());
    let error = verify(&fixture, &catalog)
        .err()
        .ok_or("duplicate physical owner must fail")?;
    assert!(
        error.to_string().contains("duplicate physical Item owner"),
        "physical rows cannot silently collapse: {error}"
    );
    let catalog = fixture::catalog_with_alias()?;
    let rows = catalog
        .get("items")
        .and_then(Value::as_array)
        .ok_or("actual alias rows")?;
    let first = rows.first().ok_or("original owner")?;
    let aliases: Vec<_> = rows
        .iter()
        .filter(|row| row.pointer("/guid/value") == first.pointer("/guid/value"))
        .collect();
    assert_eq!(
        aliases.len(),
        2,
        "two genuinely physical owners share a nominal GUID"
    );
    let [first, second] = aliases.as_slice() else {
        return Err("two physical alias owners".into());
    };
    assert_ne!(
        first.pointer("/recordIndex/value"),
        second.pointer("/recordIndex/value"),
        "aliases come from distinct original Records"
    );
    let error = verify(&fixture, &catalog)
        .err()
        .ok_or("GUID-only reference cannot resolve physical alternatives")?;
    assert!(
        error
            .to_string()
            .contains("duplicate unique GUID join identity"),
        "ambiguous comparison never counts as agreement: {error}"
    );
    Ok(())
}
