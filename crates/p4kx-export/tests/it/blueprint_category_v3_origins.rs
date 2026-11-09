// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use p4kx_export::CatalogPublicationMode;
use serde_json::{Value, json};

use crate::blueprint_category_v3_fixture as fixture;
use crate::canonical_header_fixture;
use crate::commodity_v3_fixture::{DcbOrigin, TestResult};
use crate::commodity_v3_source_inventory;

#[test]
fn category_v3_exposes_each_header_origin_and_explicit_plain_encrypted_unknown_marker() -> TestResult
{
    let source = fixture::single();
    fixture::original(
        &source,
        0,
        fixture::CATEGORY,
        fixture::RAW_NAME,
        fixture::FILE_NAME,
        fixture::TAG,
    )?;
    fixture::fieldless(&source, 0)?;
    for (origin, marker) in [
        (DcbOrigin::Plain, json!(false)),
        (DcbOrigin::Encrypted, json!(true)),
        (DcbOrigin::Unknown, Value::Null),
    ] {
        let (_, catalog) = fixture::build(
            &source,
            origin,
            CatalogPublicationMode::DiagnosticPartial,
            None,
        )?;
        let row = fixture::row(&catalog, 0)?;
        for field in fixture::FIELDS {
            let cell = fixture::at(row, &format!("/{field}"))?;
            assert_eq!(
                cell.as_object()
                    .ok_or("sourced header cell")?
                    .keys()
                    .map(String::as_str)
                    .collect::<BTreeSet<_>>(),
                BTreeSet::from([
                    "state",
                    "value",
                    "sourceUsesRefs",
                    "sourceSetComplete",
                    "containsEncryptedSource"
                ]),
                "the marker and all immediate source-state keys are required, even for unknown encryption"
            );
            assert_eq!(
                fixture::at(cell, "/containsEncryptedSource")?,
                &marker,
                "the marker is an explicit exact nullable boolean"
            );
            assert_eq!(
                fixture::at(cell, "/sourceSetComplete")?,
                &json!(true),
                "known contributors are separate from unknown encryption"
            );
        }
        assert_header_roles(&catalog, row)?;
        commodity_v3_source_inventory::reference_closure(&catalog)?;
    }
    Ok(())
}

fn assert_header_roles(catalog: &Value, row: &Value) -> TestResult {
    for (cell, field, role) in [
        ("guid", "guid", "payload"),
        ("recordName", "recordName", "payload"),
        ("recordIndex", "recordIndex", "selector"),
        ("sourceType", "sourceType", "selector"),
        ("fileName", "fileName", "payload"),
        ("metadataTag", "metadataTag", "payload"),
        ("recordPath", "fileName", "conversion"),
    ] {
        assert_eq!(
            canonical_header_fixture::header_uses(catalog, fixture::at(row, &format!("/{cell}"))?)?,
            BTreeSet::from([(0, field.to_owned(), role.to_owned())]),
            "the precise original header site supplies this cell without locale/body taint"
        );
    }
    assert_eq!(
        canonical_header_fixture::header_uses(catalog, fixture::at(row, "/className")?)?,
        BTreeSet::from([
            (0, "recordName".to_owned(), "conversion".to_owned()),
            (0, "sourceType".to_owned(), "selector".to_owned()),
        ]),
        "className uses the actual prefix conversion, not a fabricated stored header field"
    );
    Ok(())
}
