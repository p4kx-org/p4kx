// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::CatalogRecordSelection;
use p4kx_export::catalog_queries::loot::{LootDefinitionKind, with_loot_definitions_from};
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};
use serde_json::{Value, json};

use crate::health_origin_assertions as proof;
use crate::health_origin_fixture::{Fixture, Input, TestResult};
use crate::loot_fixture::{self, TABLE};
use crate::starmap_fixture;

use super::{Graph, entries, entry_fields, graph, is_value_use};

const OPTIONAL: &str = "FixtureOptional";
const INTEGER: &str = "FixtureLimitInteger";
const NULL: &str = "FixtureLimitNull";
const BROKEN: &str = "FixtureLimitBroken";

#[test]
fn weighted_limits_distinguish_unselected_source_states_and_keep_original_positions() -> TestResult
{
    let fixture = loot_fixture::source_fixture(&database().build(), Input::Encrypted)?;
    let (_, catalog) = fixture.diagnostic()?;
    let table = Graph::in_table(&catalog, "lootTables", &starmap_fixture::guid_text(TABLE))?;
    let owners = entries(&table)?;
    let [absent, explicit_null, invalid, unreadable, recovered] =
        proof::rows(&catalog, "/lootTables/0/entries")?
    else {
        return Err("five projected source conditions".into());
    };
    assert_states(absent, explicit_null, invalid, unreadable)?;
    assert_original_cases(&catalog, &table, owners, explicit_null, invalid, unreadable)?;
    assert_recovered_origin(&catalog, &table, owners, recovered)?;
    assert_sdk_coverage(&fixture)
}

fn assert_sdk_coverage(fixture: &Fixture) -> TestResult {
    with_loot_definitions_from(
        &fixture.archive,
        &fixture.entry,
        LootDefinitionKind::Table,
        CatalogRecordSelection::All,
        None,
        |snapshot| {
            let [definition] = snapshot.rows.as_slice() else {
                return Err("expected one physical Loot record".into());
            };
            let physical = definition
                .source()
                .record_index()
                .value()
                .ok_or("index missing")?;
            assert_eq!(
                snapshot.coverage.incomplete_fields.as_slice(),
                [*physical],
                "recoverable authored field failures are reported once for the physical owner"
            );
            assert!(
                snapshot.coverage.unreadable_bodies.is_empty(),
                "available fields and errors are retained in a readable record"
            );
            Ok(())
        },
    )?
}

fn assert_states(
    absent: &Value,
    explicit_null: &Value,
    invalid: &Value,
    unreadable: &Value,
) -> TestResult {
    for (row, state, complete) in [
        (absent, "absent", true),
        (explicit_null, "null", true),
        (invalid, "invalid", true),
        (unreadable, "unreadable", false),
    ] {
        let source = proof::cell(row, "dupeLimitSource", state)?;
        assert_eq!(
            row.get("dupeLimit"),
            Some(&Value::Null),
            "the numeric getter retains an explicit null when no limit is selected"
        );
        assert_eq!(
            proof::at(source, "/sourceSetComplete")?,
            &json!(complete),
            "only a read failure limits coverage"
        );
        assert_eq!(
            proof::at(source, "/containsEncryptedSource")?,
            &json!(true),
            "the retained source was encrypted"
        );
    }
    assert_eq!(
        invalid.get("weight"),
        Some(&Value::Null),
        "a nonfinite weight stays out of the numeric view"
    );
    proof::cell(invalid, "weightSource", "invalid")?;
    Ok(())
}

fn assert_original_cases(
    catalog: &Value,
    table: &Graph<'_>,
    owners: &[Value],
    explicit_null: &Value,
    invalid: &Value,
    unreadable: &Value,
) -> TestResult {
    let [_, null_entry, invalid_entry, broken_entry, _] = owners else {
        return Err("five original Loot entries".into());
    };
    for (entry, view) in [(null_entry, explicit_null), (invalid_entry, invalid)] {
        let raw = limit_link(table, entry, 0)?;
        assert!(
            proof::uses(catalog, proof::at(view, "/dupeLimitSource")?)?
                .iter()
                .any(|used| is_value_use(used, raw, "selector")),
            "the retained known candidate cites the exact original ordinal"
        );
    }
    let broken = limit_link(table, broken_entry, 0)?;
    let node = table.follow(broken)?;
    assert_eq!(
        proof::at(node, "/value/state")?,
        &json!("unreadable"),
        "the original slot is kept"
    );
    assert!(
        !proof::at(node, "/originalError")?.is_null(),
        "the original failure remains navigable"
    );
    assert!(
        proof::rows(catalog, "/provenance/uses")?
            .iter()
            .any(|used| is_value_use(used, broken, "attempted")),
        "the unsuccessful read is recorded at the actual original ordinal"
    );
    assert_eq!(
        proof::at(unreadable, "/dupeLimitSource/sourceSetComplete")?,
        &json!(false),
        "registering an attempt never certifies the missing value"
    );
    Ok(())
}

fn assert_recovered_origin(
    catalog: &Value,
    table: &Graph<'_>,
    owners: &[Value],
    recovered: &Value,
) -> TestResult {
    let owner = owners.last().ok_or("the original recovered entry")?;
    let expected = limit_link(table, owner, 1)?;
    let source = proof::at(recovered, "/dupeLimitSource")?;
    assert_eq!(
        proof::at(recovered, "/dupeLimit")?,
        &json!(4),
        "the later readable value still wins"
    );
    assert_eq!(
        graph::present(source)?,
        expected,
        "the winner uses its actual local ordinal"
    );
    assert_eq!(
        proof::at(source, "/sourceSetComplete")?,
        &json!(false),
        "the earlier error is still material"
    );
    assert!(
        proof::uses(catalog, source)?
            .iter()
            .any(|used| is_value_use(used, expected, "conversion")),
        "the selected numeric value retains its exact original source"
    );
    Ok(())
}

fn limit_link<'catalog>(
    table: &Graph<'catalog>,
    entry: &Value,
    slot: usize,
) -> TestResult<&'catalog Value> {
    let fields = entry_fields(table, entry)?;
    let array = graph::payload(table.field(fields, "optionalData")?, "array")?;
    let raw = table.follow(
        graph::array(array, "elements")?
            .get(slot)
            .ok_or("optional slot")?,
    )?;
    let fields = graph::array(graph::payload(raw, "struct")?, "fields")?;
    fields
        .iter()
        .find(|field| field.pointer("/name/value").and_then(Value::as_str) == Some("dupeLimit"))
        .and_then(|field| field.get("value"))
        .ok_or_else(|| "physical duplicate-limit field link".into())
}

fn database() -> DataForgeBuilder {
    let broken = || optional(BROKEN, TestValue::Raw(u32::MAX.to_le_bytes().to_vec()));
    let entries = vec![
        entry(1.0, Vec::new()),
        entry(1.0, vec![optional(NULL, TestValue::Null)]),
        entry(
            f32::NAN,
            vec![optional(INTEGER, TestValue::UInt64(u64::MAX))],
        ),
        entry(1.0, vec![broken()]),
        entry(1.0, vec![broken(), optional(INTEGER, TestValue::UInt64(4))]),
    ];
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("LootTableV3Record").field("lootTable", class("LootTableV3")))
        .structure(
            TestStruct::new("LootTableV3").array("lootArchetypes", class("LootTableV3Entry")),
        )
        .structure(
            TestStruct::new("LootTableV3Entry")
                .field("weight", TestType::Single)
                .array("optionalData", pointer(OPTIONAL)),
        )
        .structure(TestStruct::new(OPTIONAL))
        .structure(
            TestStruct::new(INTEGER)
                .parent(OPTIONAL)
                .field("dupeLimit", TestType::UInt64),
        )
        .structure(
            TestStruct::new(NULL)
                .parent(OPTIONAL)
                .field("dupeLimit", pointer(OPTIONAL)),
        )
        .structure(
            TestStruct::new(BROKEN)
                .parent(OPTIONAL)
                .field("dupeLimit", TestType::String),
        )
        .record(
            TestRecord::new(
                TABLE,
                "LootTableV3Record",
                "LootTableV3Record.FixtureStates",
                "libs/foundry/records/lootgeneration/loottables/fixture_states.xml",
            )
            .value(
                "lootTable",
                TestValue::Struct(
                    TestInstance::new("LootTableV3")
                        .value("lootArchetypes", TestValue::Array(entries)),
                ),
            ),
        )
}

fn entry(weight: f32, slots: Vec<TestValue>) -> TestValue {
    TestValue::Struct(
        TestInstance::new("LootTableV3Entry")
            .value("weight", TestValue::Single(weight))
            .value("optionalData", TestValue::Array(slots)),
    )
}

fn optional(source_type: &str, value: TestValue) -> TestValue {
    TestValue::Struct(TestInstance::new(source_type).value("dupeLimit", value))
}

fn class(name: &str) -> TestType {
    TestType::Class(name.to_owned())
}

fn pointer(name: &str) -> TestType {
    TestType::StrongPointer(name.to_owned())
}
