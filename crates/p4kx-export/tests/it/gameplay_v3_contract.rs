// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::gameplay_v3_schema as schema;
use crate::gameplay_v3_source as source;
use crate::gameplay_v3_support::{self as support, TestResult};
use crate::gameplay_v3_values as values;
use p4kx_export::Kind;
use p4kx_testkit::TestValue;
use serde_json::{Value, json};

#[test]
fn gameplay_v3_has_one_physical_definition_table_with_eight_headers_and_four_original_members()
-> TestResult {
    let database = values::simple().record(values::record(
        values::FIRST,
        "Second",
        values::scale(TestValue::Single(2.5)),
    ));
    source::declarations(&database)?;
    source::basic(&database)?;
    source::with_tree(&database, 1, |tree| {
        assert_eq!(
            tree.guid().to_string(),
            crate::commodity_v3_fixture::guid(values::FIRST),
            "equal nominal GUIDs are distinct physical source records"
        );
        Ok(())
    })?;
    let (built, catalog) = support::diagnostic(&database)?;
    assert_definition_table(&built, &catalog)?;
    assert_availability()
}

fn assert_headers(row: &Value) -> TestResult {
    for (field, expected) in [
        (
            "guid",
            json!(crate::commodity_v3_fixture::guid(values::FIRST)),
        ),
        ("recordName", json!("CraftingGameplayPropertyDef.Primary")),
        ("className", json!("Primary")),
        (
            "recordPath",
            json!("libs/foundry/records/crafting/craftedproperties/Primary"),
        ),
        ("recordIndex", json!(0)),
        ("sourceType", json!(schema::PROPERTY)),
        ("fileName", json!(values::FIRST_FILE)),
        ("metadataTag", json!("SyntheticCrafting")),
    ] {
        let cell = support::at(row, &format!("/{field}"))?;
        support::state(cell, "present")?;
        assert_eq!(
            support::at(cell, "/value")?,
            &expected,
            "original sourced {field}"
        );
        support::marker(cell, Some(false))?;
    }
    assert!(
        row.get("name").is_none(),
        "the reduced legacy name DTO is retired from this wire"
    );
    Ok(())
}

fn assert_availability() -> TestResult {
    for (database, expected) in [
        (schema::schema(), "available"),
        (p4kx_testkit::DataForgeBuilder::new().version(8), "absent"),
    ] {
        let (_, catalog) = support::normal(&database)?;
        assert!(
            support::rows(&catalog)?.is_empty(),
            "declaration availability is independent of row count"
        );
        assert_eq!(
            support::at(&catalog, "/definitions/gameplayProperties/availability")?,
            &json!(expected),
            "the actual declared family controls table availability"
        );
    }
    Ok(())
}

fn assert_definition_table(built: &p4kx_export::BuiltCatalog, catalog: &Value) -> TestResult {
    let rows = support::rows(catalog)?;
    assert_eq!(rows.len(), 2, "both original physical definitions survive");
    assert!(
        catalog.get("gameplayProperties").is_none(),
        "no flat legacy wire mirror"
    );
    assert_eq!(
        support::at(catalog, "/schemaVersion")?,
        &json!(3),
        "the writer emits v3 only"
    );
    assert_eq!(
        built
            .summary()
            .kind(Kind::GameplayProperties)
            .map(|kind| (kind.count, kind.skipped)),
        Some((2, 0)),
        "the existing Kind counts physical definitions"
    );
    assert_eq!(
        Kind::ALL.get(7),
        Some(&Kind::GameplayProperties),
        "the existing Kind ordinal is preserved"
    );
    assert_headers(support::row(catalog, 0)?)?;
    assert_eq!(
        support::at(support::row(catalog, 1)?, "/recordIndex/value")?,
        &json!(1),
        "equal GUID ties retain original physical order"
    );
    for field in [
        "propertyName",
        "displayTransformation",
        "unitFormat",
        "nameOverrides",
    ] {
        crate::text_forms_assertions::direct(support::at(
            support::row(catalog, 0)?,
            &format!("/{field}"),
        )?)?;
    }
    Ok(())
}
