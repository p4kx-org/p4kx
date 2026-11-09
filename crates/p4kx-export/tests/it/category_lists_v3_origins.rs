// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use p4kx_export::CatalogPublicationMode;
use serde_json::{Value, json};

use crate::category_lists_v3_fixture as fixture;
use crate::category_lists_v3_source as source;
use crate::commodity_v3_fixture::{DcbOrigin, TestResult};
use crate::commodity_v3_source_inventory::{body, header, reference_closure, uses};

#[test]
fn category_lists_v3_exposes_exact_independent_plain_encrypted_unknown_origins() -> TestResult {
    let database = fixture::clean();
    source::ordered(&database, false)?;
    for (origin, marker) in [
        (DcbOrigin::Plain, json!(false)),
        (DcbOrigin::Encrypted, json!(true)),
        (DcbOrigin::Unknown, Value::Null),
    ] {
        let (_, catalog) = fixture::build(
            &database,
            origin,
            CatalogPublicationMode::DiagnosticPartial,
            None,
        )?;
        let row = fixture::row(&catalog, 0)?;
        assert_markers(row, &marker)?;
        assert_headers(&catalog, row)?;
        assert_members(&catalog, row)?;
        reference_closure(&catalog)?;
    }
    Ok(())
}

fn assert_markers(value: &Value, marker: &Value) -> TestResult {
    if value.get("state").is_some() && value.get("sourceUsesRefs").is_some() {
        let keys = value
            .as_object()
            .ok_or("source carrier")?
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        assert_eq!(
            keys,
            BTreeSet::from([
                "state",
                "value",
                "sourceUsesRefs",
                "sourceSetComplete",
                "containsEncryptedSource"
            ]),
            "every header/container/slot/reference datum has all required source keys"
        );
        assert_eq!(
            fixture::at(value, "/containsEncryptedSource")?,
            marker,
            "the immediate nullable encryption marker is explicit per datum"
        );
        assert_eq!(
            fixture::at(value, "/sourceSetComplete")?,
            &json!(true),
            "complete contributors are separate from unknown encryption"
        );
    }
    match value {
        Value::Array(values) => {
            for child in values {
                assert_markers(child, marker)?;
            }
        }
        Value::Object(values) => {
            for child in values.values() {
                assert_markers(child, marker)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn assert_headers(catalog: &Value, row: &Value) -> TestResult {
    for (field, input, role) in [
        ("guid", "guid", "payload"),
        ("recordName", "recordName", "payload"),
        ("recordIndex", "recordIndex", "selector"),
        ("sourceType", "sourceType", "selector"),
        ("recordPath", "fileName", "conversion"),
        ("fileName", "fileName", "payload"),
        ("metadataTag", "metadataTag", "payload"),
    ] {
        assert_eq!(
            uses(catalog, fixture::at(row, &format!("/{field}"))?)?,
            BTreeSet::from([(0, header(input), role.to_owned())]),
            "header fields retain only the actual original header contribution"
        );
    }
    assert_eq!(
        uses(catalog, fixture::at(row, "/className")?)?,
        BTreeSet::from([
            (0, header("recordName"), "conversion".to_owned()),
            (0, header("sourceType"), "selector".to_owned())
        ]),
        "the class-name prefix conversion keeps both actual header inputs"
    );
    Ok(())
}

fn assert_members(catalog: &Value, row: &Value) -> TestResult {
    assert_eq!(
        uses(catalog, fixture::at(row, "/categories")?)?,
        BTreeSet::from([(0, body("categories"), "payload".to_owned())]),
        "the authored container is not tainted by every target header"
    );
    for ordinal in [0, 1, 2, 3] {
        let path = format!("categories[{ordinal}]");
        let slot = fixture::at(row, &format!("/categories/value/{ordinal}"))?;
        let expected = BTreeSet::from([(0, body(&path), "payload".to_owned())]);
        assert_eq!(
            uses(catalog, slot)?,
            expected,
            "null/repeated slots retain their own actual original ordinal"
        );
        if ordinal != 1 {
            assert_eq!(
                uses(catalog, fixture::at(slot, "/value/guid")?)?,
                expected,
                "the GUID comes from the authored slot, not the resolved target header"
            );
            assert_target(catalog, slot, if ordinal == 2 { 1 } else { 3 })?;
        }
    }
    Ok(())
}

fn assert_target(catalog: &Value, slot: &Value, index: u64) -> TestResult {
    for (field, input, role) in [
        ("targetRecordIndex", "recordIndex", "selector"),
        ("targetType", "sourceType", "selector"),
        ("recordPath", "fileName", "conversion"),
    ] {
        assert_eq!(
            uses(catalog, fixture::at(slot, &format!("/value/{field}"))?)?,
            BTreeSet::from([(index, header(input), role.to_owned())]),
            "resolved target metadata uses its actual separate physical header without target-body or locale input"
        );
    }
    Ok(())
}
