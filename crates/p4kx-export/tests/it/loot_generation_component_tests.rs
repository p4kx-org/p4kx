// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{ExportHealthStatus, build_catalog_from};
use p4kx_extraction::{Filter, SourceUseRole, Value as Original};
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};
use serde_json::{Value, json};

use crate::health_origin_fixture::{Fixture, Input, TestResult, options};
use crate::loot_v3_graph::{self as graph, Graph};
use crate::loot_v3_origin::{self as origin, ExpectedOrigin};

const ENTITY: &str = "EntityClassDefinition";
const COMPONENTS: &str = "Components";
const COMPONENT: &str = "LootGenerationComponentParams";
const THEME: &str = "LootThemeRecord";
const SECONDARY_BASE: &str = "LootV3SecondaryChoicesRecordRef_Base";
const SECONDARY_WRAPPER: &str = "LootV3SecondaryChoicesRecordRef_SingleLayer";
const SECONDARY_RECORD: &str = "LootV3SecondaryChoicesSingleLayerRecord";
const POOL: &str = "FixtureComponent";
const UNRELATED: &str = "FixtureOtherComponent";
const OWNER: &str = "synthetic-outside-catalog/owner.xml";
const OTHER_OWNER: &str = "synthetic-outside-catalog/other.xml";
const THEME_ID: u128 = 0xd847_0000_0000_4000_8000_0000_0000_0021;
const THEME_GUID: &str = "d8470000-0000-4000-8000-000000000021";
const SECONDARY_ID: u128 = 0xd847_0000_0000_4000_8000_0000_0000_0022;
const SECONDARY_GUID: &str = "d8470000-0000-4000-8000-000000000022";
const OWNER_ID: u128 = 0xd847_0000_0000_4000_8000_0000_0000_0023;
const NAN_BITS: u32 = 0x7fc0_0847;
const TABLE: &str = "/configurations/lootGeneration";

#[test]
fn loot_generation_reconciles_independent_owners_positions_and_existing_targets() -> TestResult {
    let input = Input::Encrypted;
    let fixture = Fixture::open(&database(), input, Vec::new())?;
    check_original_slots(&fixture)?;
    let built = build_catalog_from(&fixture.archive, &fixture.entry, &options())?;
    assert_eq!(
        built.summary().export_health().status,
        ExportHealthStatus::Complete
    );
    let catalog: Value = serde_json::from_slice(built.json())?;
    let chunks = rows(&catalog)?;
    assert_eq!(chunks.len(), 3, "one row per physical component");
    for (row, (record, ordinal)) in chunks.iter().zip([(0, 1), (0, 3), (3, 1)]) {
        assert_eq!(row.pointer("/ownerRecordIndex/value"), Some(&json!(record)));
        assert_eq!(
            row.pointer("/componentOrdinal/value"),
            Some(&json!(ordinal))
        );
        assert_eq!(row.pointer("/sourceType/value"), Some(&json!(COMPONENT)));
    }
    let first = chunks.first().ok_or("first original loot component")?;
    let graph = graph_for(first)?;
    let constraints = constraints(&graph)?;
    let themes = graph::elements(graph.field(constraints, "themes")?)?;
    let [first_theme, null, repeated] = themes else {
        return Err("three original theme slots".into());
    };
    for link in [first_theme, repeated] {
        assert_eq!(graph::reference_guid(graph.follow(link)?)?, THEME_GUID);
    }
    graph::source_null(graph.follow(null)?);
    let secondary = graph::structure(
        graph.field(constraints, "secondaryChoices")?,
        SECONDARY_WRAPPER,
    )?;
    assert_eq!(
        graph::reference_guid(graph.field(secondary, "singleLayerRecord")?)?,
        SECONDARY_GUID
    );
    let raw = graph.field(constraints, "futureScore")?;
    assert_eq!(
        graph::payload(raw, "number")?.pointer("/value/rawBits"),
        Some(&json!("7fc00847"))
    );
    origin::verify(
        raw.get("value").ok_or("immediate raw score")?,
        built.summary().source_inventory(),
        &ExpectedOrigin {
            encrypted: input.marker(),
            complete: true,
            uses: &[origin::value_use(11, "$value", SourceUseRole::Payload)],
        },
    )
    .map_err(|error| format!("original score evidence: {error:?}"))?;
    Ok(())
}

fn check_original_slots(fixture: &Fixture) -> TestResult {
    let forge = fixture.entry.open()?;
    let owner = forge.record(0).ok_or("original owner")?;
    let original = owner.tree()?;
    let Some(Original::Array(components)) = original.field(COMPONENTS) else {
        return Err("original component array".into());
    };
    assert_eq!(components.len(), 4, "all original component slots");
    let locations = components
        .iter()
        .enumerate()
        .filter_map(|(index, value)| match value {
            Original::Struct(component) if component.type_name() == COMPONENT => Some(index),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        locations,
        [1, 3],
        "the exact independent component positions"
    );
    Ok(())
}

#[test]
fn loot_generation_filter_excludes_other_holders_but_keeps_authored_references() -> TestResult {
    let fixture = Fixture::open(&database(), Input::Plain, Vec::new())?;
    let filter = Filter::parse("synthetic-outside-catalog/owner")?;
    let mut selected = options();
    selected.filter = Some(&filter);
    let built = build_catalog_from(&fixture.archive, &fixture.entry, &selected)?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let chunks = rows(&catalog)?;
    assert_eq!(
        chunks.len(),
        2,
        "only one physical owner matches the Record filter"
    );
    assert_eq!(
        catalog.pointer("/definitions/lootThemes/rows"),
        Some(&json!([]))
    );
    let first = chunks.first().ok_or("filtered owner component")?;
    let graph = graph_for(first)?;
    let fields = constraints(&graph)?;
    let themes = graph::elements(graph.field(fields, "themes")?)?;
    let target = graph.follow(themes.first().ok_or("first source theme")?)?;
    assert_eq!(graph::reference_guid(target)?, THEME_GUID);
    assert_eq!(
        target.pointer("/value/value/value/targetRecordIndex/value"),
        Some(&json!(1))
    );
    let second = chunks.get(1).ok_or("second owner component")?;
    let graph = graph_for(second)?;
    let fields = constraints(&graph)?;
    assert_eq!(
        graph::elements(graph.field(fields, "themes")?)?,
        &[] as &[Value],
        "authored empty themes remain explicit"
    );
    graph::source_null(graph.field(fields, "secondaryChoices")?);
    Ok(())
}

fn rows(catalog: &Value) -> TestResult<&[Value]> {
    let table = catalog
        .pointer(TABLE)
        .ok_or("physical Loot configuration table")?;
    assert_eq!(
        table.get("availability"),
        Some(&json!("available")),
        "declared original component family"
    );
    graph::array(table, "rows")
}

fn graph_for(row: &Value) -> TestResult<Graph<'_>> {
    Graph::in_arena(row.get("authoredFields").ok_or("authored loot arena")?)
}

fn constraints<'catalog>(graph: &Graph<'catalog>) -> TestResult<&'catalog [Value]> {
    let config = graph::structure(graph.root("lootConfig")?, "LootConfig")?;
    graph::structure(graph.field(config, "lootConstraints")?, "LootConstraints")
}

fn database() -> Vec<u8> {
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new(ENTITY).array(COMPONENTS, TestType::StrongPointer(POOL.into())))
        .structure(TestStruct::new(POOL))
        .structure(TestStruct::new(UNRELATED).parent(POOL))
        .structure(
            TestStruct::new(COMPONENT)
                .parent(POOL)
                .field("lootConfig", TestType::Class("LootConfig".into())),
        )
        .structure(
            TestStruct::new("LootConfig")
                .field("lootConstraints", TestType::Class("LootConstraints".into())),
        )
        .structure(
            TestStruct::new("LootConstraints")
                .array("themes", TestType::ReferenceTo(THEME.into()))
                .field(
                    "secondaryChoices",
                    TestType::StrongPointer(SECONDARY_BASE.into()),
                )
                .field("futureScore", TestType::Single),
        )
        .structure(TestStruct::new(SECONDARY_BASE))
        .structure(
            TestStruct::new(SECONDARY_WRAPPER)
                .parent(SECONDARY_BASE)
                .field(
                    "singleLayerRecord",
                    TestType::ReferenceTo(SECONDARY_RECORD.into()),
                ),
        )
        .structure(TestStruct::new(SECONDARY_RECORD))
        .structure(TestStruct::new(THEME))
        .record(owner(
            OWNER_ID,
            OWNER,
            vec![component(true), TestValue::Null, component(false)],
        ))
        .record(TestRecord::new(
            THEME_ID,
            THEME,
            "Fixture.Theme",
            "synthetic-targets/theme.xml",
        ))
        .record(TestRecord::new(
            SECONDARY_ID,
            SECONDARY_RECORD,
            "Fixture.Layer",
            "synthetic-targets/layer.xml",
        ))
        .record(owner(OWNER_ID + 1, OTHER_OWNER, vec![component(false)]))
        .build()
}

fn owner(id: u128, path: &str, components: Vec<TestValue>) -> TestRecord {
    let mut rows = vec![TestValue::Struct(TestInstance::new(UNRELATED))];
    rows.extend(components);
    TestRecord::new(id, ENTITY, "EntityClassDefinition.FixtureLootOwner", path)
        .value(COMPONENTS, TestValue::Array(rows))
}

fn component(populated: bool) -> TestValue {
    let themes = if populated {
        vec![
            TestValue::Reference(THEME_ID),
            TestValue::Null,
            TestValue::Reference(THEME_ID),
        ]
    } else {
        Vec::new()
    };
    let secondary = if populated {
        TestValue::Struct(
            TestInstance::new(SECONDARY_WRAPPER)
                .value("singleLayerRecord", TestValue::Reference(SECONDARY_ID)),
        )
    } else {
        TestValue::Null
    };
    let constraints = TestInstance::new("LootConstraints")
        .value("themes", TestValue::Array(themes))
        .value("secondaryChoices", secondary)
        .value(
            "futureScore",
            TestValue::Single(if populated {
                f32::from_bits(NAN_BITS)
            } else {
                0.0
            }),
        );
    let config =
        TestInstance::new("LootConfig").value("lootConstraints", TestValue::Struct(constraints));
    TestValue::Struct(TestInstance::new(COMPONENT).value("lootConfig", TestValue::Struct(config)))
}
