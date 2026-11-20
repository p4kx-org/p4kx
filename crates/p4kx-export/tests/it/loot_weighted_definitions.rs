// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{BuiltCatalog, Kind, build_catalog_from};
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};
use serde_json::{Value, json};

use crate::health_origin_assertions as proof;
use crate::health_origin_fixture::{Input, TestResult, options};
use crate::loot_fixture;
use crate::loot_v3_graph::{self as graph, Graph};
use crate::starmap_fixture;

#[path = "loot_weighted_states.rs"]
mod states;

#[test]
fn weighted_loot_keeps_each_optional_slot_and_its_immediate_origin() -> TestResult {
    let bytes = loot_fixture::database().build();
    for input in [Input::Plain, Input::Encrypted, Input::Standalone] {
        let (_, catalog) = catalog(&bytes, input)?;
        assert_eq!(
            proof::at(&catalog, "/schemaVersion")?,
            &json!(3),
            "the catalog exports schema v3"
        );
        let guid = starmap_fixture::guid_text(loot_fixture::TABLE);
        let original = Graph::in_table(&catalog, "lootTables", &guid)?;
        let [inline, linked] = entries(&original)? else {
            return Err("the fixture authors one inline and one linked entry".into());
        };
        let inline = entry_fields(&original, inline)?;
        let linked = entry_fields(&original, linked)?;
        assert_optional_slots(&original, inline, input)?;
        assert_weight_links(&catalog, &original, inline, input)?;
        assert_linked_archetype(&catalog, &original, linked)?;
    }
    Ok(())
}

fn catalog(bytes: &[u8], input: Input) -> TestResult<(BuiltCatalog, Value)> {
    let fixture = loot_fixture::source_fixture(bytes, input)?;
    let built = build_catalog_from(&fixture.archive, &fixture.entry, &options())?;
    let catalog = serde_json::from_slice(built.json())?;
    Ok((built, catalog))
}

fn entries<'catalog>(loot: &Graph<'catalog>) -> TestResult<&'catalog [Value]> {
    let table = graph::structure(loot.root("lootTable")?, "LootTableV3")?;
    graph::array(
        graph::payload(loot.field(table, "lootArchetypes")?, "array")?,
        "elements",
    )
}

fn entry_fields<'catalog>(loot: &Graph<'catalog>, entry: &Value) -> TestResult<&'catalog [Value]> {
    graph::structure(loot.follow(entry)?, "LootTableV3Entry")
}

fn assert_optional_slots(loot: &Graph<'_>, entry: &[Value], input: Input) -> TestResult {
    let value = graph::payload(loot.field(entry, "optionalData")?, "array")?;
    let [_, _, first, null, unknown, repeated, _] = graph::array(value, "elements")? else {
        return Err("the authored optional sequence is complete".into());
    };
    let first = dupe(loot, first)?;
    let repeated = dupe(loot, repeated)?;
    assert_number(first, "int32", 3, input)?;
    assert_number(repeated, "int32", 7, input)?;
    assert_ne!(
        proof::at(first, "/value/sourceUsesRefs")?,
        proof::at(repeated, "/value/sourceUsesRefs")?,
        "both duplicate-limit occurrences keep distinct physical evidence"
    );
    graph::source_null(loot.follow(null)?);
    let unknown = graph::structure(
        loot.follow(unknown)?,
        "LootTableOptionalDataV3_FixtureExtension",
    )?;
    assert_number(loot.field(unknown, "unknownField")?, "uint16", 12, input)
}

fn dupe<'catalog>(loot: &Graph<'catalog>, element: &Value) -> TestResult<&'catalog Value> {
    let fields = graph::structure(loot.follow(element)?, "LootTableOptionalDataV3_DupeLimit")?;
    loot.field(fields, "dupeLimit")
}

fn assert_number(node: &Value, kind: &str, expected: i32, input: Input) -> TestResult {
    let number = graph::payload(node, "number")?;
    let datum = proof::at(node, "/value")?;
    assert_eq!(
        proof::at(number, "/value/type")?,
        &json!(kind),
        "the stored integer width is preserved"
    );
    assert_eq!(
        proof::at(number, "/value/value")?,
        &json!(expected),
        "the exact stored integer is retained"
    );
    assert_origin(datum, input)
}

fn assert_weight_links(
    catalog: &Value,
    loot: &Graph<'_>,
    original: &[Value],
    input: Input,
) -> TestResult {
    let view = proof::at(catalog, "/lootTables/0")?;
    let definition = proof::at(catalog, "/definitions/lootTables/rows/0")?;
    assert_eq!(
        proof::at(view, "/recordIndex")?,
        proof::at(definition, "/recordIndex/value")?,
        "the view links its exact physical definition"
    );
    let entry = proof::at(view, "/entries/0")?;
    let datum = proof::at(entry, "/weightSource")?;
    let source = loot.follow(graph::present(datum)?)?;
    assert!(
        std::ptr::eq(source, loot.field(original, "weight")?),
        "the finite weight points to the same occurrence kept by the definition"
    );
    assert_eq!(
        proof::at(graph::payload(source, "number")?, "/value/rawBits")?,
        &json!("41200000"),
        "the original weight keeps its IEEE bits"
    );
    assert_origin(datum, input)?;
    assert!(
        proof::uses(catalog, datum)?
            .iter()
            .any(|used| used.get("role").and_then(Value::as_str) == Some("payload")),
        "the convenience weight cites its original scalar"
    );
    let inline = proof::at(entry, "/archetype/0/weightSource")?;
    let inline_weight = loot.follow(graph::present(inline)?)?;
    assert_eq!(
        proof::at(graph::payload(inline_weight, "number")?, "/value/type")?,
        &json!("f32"),
        "the inline archetype uses its table's original arena"
    );
    assert_origin(inline, input)?;
    assert_dupe_prefix_origin(catalog, entry, loot, original, input)
}

fn assert_dupe_prefix_origin(
    catalog: &Value,
    entry: &Value,
    loot: &Graph<'_>,
    original: &[Value],
    input: Input,
) -> TestResult {
    let datum = proof::at(entry, "/dupeLimitSource")?;
    let original_optional = graph::payload(loot.field(original, "optionalData")?, "array")?;
    let [priority, choice, first, later @ ..] = graph::array(original_optional, "elements")? else {
        return Err("first duplicate limit and its selection prefix".into());
    };
    let source = loot.follow(graph::present(datum)?)?;
    assert!(
        std::ptr::eq(source, dupe(loot, first)?),
        "the first readable integer supplies the convenience duplicate limit"
    );
    assert_eq!(
        proof::at(entry, "/dupeLimit")?,
        &json!(3),
        "the legacy scalar remains unchanged"
    );
    assert_origin(datum, input)?;
    let uses = proof::uses(catalog, datum)?;
    for checked in [priority, choice, first] {
        assert!(
            has_value_use(&uses, checked, "selector"),
            "each inspected slot belongs to the first-match prefix"
        );
    }
    for skipped in later {
        assert!(
            !has_value_use(&uses, skipped, "selector"),
            "slots after the selected limit do not contribute"
        );
    }
    assert!(
        has_value_use(&uses, graph::present(datum)?, "conversion"),
        "the selected original Int32 converts to the convenience integer"
    );
    Ok(())
}

fn has_value_use(uses: &[&Value], reference: &Value, role: &str) -> bool {
    uses.iter().any(|used| is_value_use(used, reference, role))
}

fn is_value_use(used: &Value, reference: &Value, role: &str) -> bool {
    let Some(index) = reference.get("valueIndex").and_then(Value::as_str) else {
        return false;
    };
    used.get("role").and_then(Value::as_str) == Some(role)
        && used.pointer("/location/namespace").and_then(Value::as_str) == Some("dataforgeValue")
        && used.pointer("/location/field").and_then(Value::as_str) == Some("$value")
        && used.pointer("/location/indices/1").and_then(Value::as_str) == Some(index)
}

fn assert_origin(cell: &Value, input: Input) -> TestResult {
    assert_eq!(
        proof::at(cell, "/containsEncryptedSource")?,
        &json!(input.marker()),
        "the datum keeps the original encryption marker"
    );
    assert_eq!(
        proof::at(cell, "/sourceSetComplete")?,
        &json!(true),
        "the complete input was read"
    );
    assert!(
        !graph::array(cell, "sourceUsesRefs")?.is_empty(),
        "the datum lists its retained source uses"
    );
    Ok(())
}

fn assert_linked_archetype(catalog: &Value, loot: &Graph<'_>, linked: &[Value]) -> TestResult {
    let optional = graph::payload(loot.field(linked, "optionalData")?, "array")?;
    assert!(
        graph::array(optional, "elements")?.is_empty(),
        "the linked entry stores an empty array"
    );
    let source = graph::structure(
        loot.field(linked, "archetype")?,
        "LootArchetypeV3_RecordRef",
    )?;
    let reference = loot.field(source, "lootArchetypeRecord")?;
    let guid = starmap_fixture::guid_text(loot_fixture::ARCHETYPE);
    assert_eq!(
        graph::reference_guid(reference)?,
        guid,
        "the authored target GUID survives"
    );
    let definition = proof::at(catalog, "/definitions/lootArchetypes/rows/0")?;
    let target = graph::payload(reference, "recordReference")?;
    assert_eq!(
        proof::at(target, "/value/targetRecordIndex/value")?,
        proof::at(definition, "/recordIndex/value")?,
        "the canonical link identifies the exact physical target"
    );
    let view = proof::at(catalog, "/lootArchetypes/0")?;
    assert_eq!(
        proof::at(view, "/recordIndex")?,
        proof::at(definition, "/recordIndex/value")?,
        "the root archetype view joins its canonical row"
    );
    let arena = Graph::in_table(catalog, "lootArchetypes", &guid)?;
    let weight = arena.follow(graph::present(proof::at(view, "/entries/0/weightSource")?)?)?;
    assert_eq!(
        proof::at(graph::payload(weight, "number")?, "/value/rawBits")?,
        &json!("3f800000"),
        "the linked archetype keeps the exact stored weight"
    );
    Ok(())
}

#[test]
fn canonical_loot_counts_aliases_once_and_keeps_the_existing_path_policy() -> TestResult {
    let bytes = loot_fixture::database_with_loot_aliases().build();
    let (built, catalog) = catalog(&bytes, Input::Plain)?;
    let rows = proof::rows(&catalog, "/definitions/lootTables/rows")?;
    let view = graph::array(&catalog, "lootTables")?;
    let [first, second] = rows else {
        return Err("two admitted physical owners despite three matching GUIDs".into());
    };
    assert_eq!(
        proof::at(first, "/guid/value")?,
        proof::at(second, "/guid/value")?,
        "both admissible owners retain their shared GUID"
    );
    assert_ne!(
        proof::at(first, "/recordPath/value")?,
        proof::at(second, "/recordPath/value")?,
        "same-GUID owners retain distinct paths"
    );
    let positions = record_indices(rows, "/recordIndex/value")?;
    assert_eq!(
        positions,
        record_indices(view, "/recordIndex")?,
        "both outputs join the same physical owners"
    );
    assert!(
        positions
            .iter()
            .zip(positions.iter().skip(1))
            .all(|(before, after)| before < after),
        "same-GUID rows retain source order"
    );
    let health = built
        .summary()
        .export_health()
        .kind(Kind::LootTables)
        .ok_or("loot-table health")?;
    assert_eq!(
        health.written_rows, 2,
        "the canonical rows are counted once"
    );
    assert_eq!(
        health.source_records.projected, 2,
        "the two physical owners are projected"
    );
    Ok(())
}

fn record_indices(rows: &[Value], path: &str) -> TestResult<Vec<u64>> {
    rows.iter()
        .map(|row| {
            proof::at(row, path)?
                .as_u64()
                .ok_or_else(|| "physical Loot record index must be an unsigned integer".into())
        })
        .collect()
}

#[test]
fn weighted_loot_distinguishes_an_absent_optional_declaration() -> TestResult {
    let bytes = DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new("LootTableV3Record")
                .field("lootTable", TestType::Class("LootTableV3".into())),
        )
        .structure(
            TestStruct::new("LootTableV3")
                .array("lootArchetypes", TestType::Class("LootTableV3Entry".into())),
        )
        .structure(TestStruct::new("LootTableV3Entry").field("weight", TestType::Single))
        .record(
            TestRecord::new(
                loot_fixture::TABLE,
                "LootTableV3Record",
                "LootTableV3Record.WithoutOptional",
                "libs/foundry/records/lootgeneration/loottables/without_optional.xml",
            )
            .value(
                "lootTable",
                TestValue::Struct(TestInstance::new("LootTableV3").value(
                    "lootArchetypes",
                    TestValue::Array(vec![TestValue::Struct(
                            TestInstance::new("LootTableV3Entry")
                                .value("weight", TestValue::Single(-0.0)),
                        )]),
                )),
            ),
        )
        .build();
    let (_, catalog) = catalog(&bytes, Input::Plain)?;
    let guid = starmap_fixture::guid_text(loot_fixture::TABLE);
    let original = Graph::in_table(&catalog, "lootTables", &guid)?;
    let [entry] = entries(&original)? else {
        return Err("one physical entry".into());
    };
    let fields = entry_fields(&original, entry)?;
    assert_eq!(fields.len(), 1, "the authored entry only declares weight");
    assert!(
        original.field(fields, "optionalData").is_err(),
        "the canonical row cannot invent a declaration"
    );
    assert_eq!(
        proof::at(
            graph::payload(original.field(fields, "weight")?, "number")?,
            "/value/rawBits"
        )?,
        &json!("80000000"),
        "the source weight retains signed zero"
    );
    let entry = proof::at(&catalog, "/lootTables/0/entries/0")?;
    let source = proof::cell(entry, "dupeLimitSource", "absent")?;
    assert_origin(source, Input::Plain)?;
    Ok(())
}
