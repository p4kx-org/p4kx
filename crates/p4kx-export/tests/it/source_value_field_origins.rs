// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::UNIX_EPOCH;

use p4kx_export::{CatalogOptions, CatalogPublicationMode};
use p4kx_extraction::{DataType, SourceUseRole, Value as SourceValue};
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};
use serde_json::{Value, json};

use crate::health_origin_fixture::{Fixture, Input, TestResult};
use crate::loot_v3_graph as raw;
use crate::loot_v3_origin::{self as origin, ExpectedOrigin};

const GUID: u128 = 0xe990_0000_0000_4000_8000_0000_0000_0001;

#[test]
fn raw_field_names_are_direct_payloads_of_their_actual_physical_declaration() -> TestResult {
    for input in [Input::Plain, Input::Encrypted, Input::Standalone] {
        let fixture = Fixture::open(&database(), input, Vec::new())?;
        original(&fixture)?;
        let options = CatalogOptions {
            publication_mode: CatalogPublicationMode::DiagnosticPartial,
            ..CatalogOptions::new(UNIX_EPOCH)
        };
        let built = p4kx_export::build_catalog_from(&fixture.archive, &fixture.entry, &options)?;
        let catalog: Value = serde_json::from_slice(built.json())?;
        let row = catalog
            .pointer("/definitions/lootPoolFilters/rows/0")
            .ok_or("actual source row")?;
        let field = raw::array(row, "fields")?
            .first()
            .ok_or("actual original declaration")?;
        let name = field.get("name").ok_or("direct original name cell")?;
        assert_eq!(
            raw::present(name)?,
            &json!("rootFlag"),
            "exact actual field name"
        );
        let uses = [origin::value_use(0, "$fieldName", SourceUseRole::Payload)];
        origin::verify(
            name,
            built.summary().source_inventory(),
            &ExpectedOrigin {
                encrypted: input.marker(),
                complete: true,
                uses: &uses,
            },
        )
        .map_err(|problem| format!("direct actual field name payload: {problem:?}"))?;
        let value = raw::array(row, "values")?
            .first()
            .ok_or("actual original scalar occurrence")?;
        assert_eq!(
            raw::payload(value, "boolean")?.get("value"),
            Some(&json!(false)),
            "the distinct original scalar remains false"
        );
    }
    Ok(())
}

fn original(fixture: &Fixture) -> TestResult {
    let forge = fixture.entry.open()?;
    assert_eq!(forge.record_count(), 1, "one genuine source record");
    let record = forge.record(0).ok_or("actual original owner")?;
    let tree = record.tree()?;
    assert!(
        tree.is_from(&record),
        "actual source ownership before producer output"
    );
    assert_eq!(
        tree.error_count(),
        0,
        "the actual declaration/value are readable"
    );
    assert_eq!(tree.fields().len(), 1, "one genuine original declaration");
    let field = tree.fields().first().ok_or("actual original declaration")?;
    assert_eq!(field.name(), "rootFlag", "actual original field bytes");
    assert_eq!(
        field.storage().data_type(),
        DataType::Bool,
        "actual declared storage"
    );
    assert!(!field.storage().is_array(), "actual scalar declaration");
    assert!(
        matches!(field.value(), SourceValue::Bool(false)),
        "actual stored false"
    );
    Ok(())
}

fn database() -> Vec<u8> {
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("PoolFilterRecord").field("rootFlag", TestType::Bool))
        .record(
            TestRecord::new(
                GUID,
                "PoolFilterRecord",
                "PoolFilterRecord.Origin",
                "libs/foundry/records/lootgeneration/filters/origin.xml",
            )
            .value("rootFlag", TestValue::Bool(false)),
        )
        .build()
}
