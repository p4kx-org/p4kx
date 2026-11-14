// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{Kind, build_catalog_from};
use p4kx_extraction::{DataType, Filter};
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};
use serde_json::{Value, json};

use crate::health_origin_fixture::{Fixture, Input, TestResult, options};
use crate::loot_v3_graph::{self as graph, Graph};

const SINGLE: &str = "SubHarvestableConfigRecord";
const MULTI: &str = "SubHarvestableMultiConfigRecord";
const CONTEXT: &str = "HierarchicalContextualDataPresetRecord";
const GLOBAL: &str = "LootGenerationGlobalParams";
const SINGLE_ID: u128 = 0xd847_0000_0000_4000_8000_0000_0000_0011;
const MULTI_ID: u128 = 0xd847_0000_0000_4000_8000_0000_0000_0012;
const EXCLUDED_ID: u128 = 0xd847_0000_0000_4000_8000_0000_0000_0013;
const CONTEXT_ID: u128 = 0xd847_0000_0000_4000_8000_0000_0000_0014;
const GLOBAL_ID: u128 = 0xd847_0000_0000_4000_8000_0000_0000_0015;
const THEME_ID: u128 = 0xd847_0000_0000_4000_8000_0000_0000_0016;
const SINGLE_GUID: &str = "d8470000-0000-4000-8000-000000000011";
const MULTI_GUID: &str = "d8470000-0000-4000-8000-000000000012";
const EXCLUDED_GUID: &str = "d8470000-0000-4000-8000-000000000013";
const CONTEXT_GUID: &str = "d8470000-0000-4000-8000-000000000014";
const GLOBAL_GUID: &str = "d8470000-0000-4000-8000-000000000015";
const THEME_GUID: &str = "d8470000-0000-4000-8000-000000000016";
const SINGLE_KIND: Kind = Kind::SubHarvestableConfigs;
const MULTI_KIND: Kind = Kind::SubHarvestableMultiConfigs;
const CONTEXT_KIND: Kind = Kind::HierarchicalContextualDataPresets;
const GLOBAL_KIND: Kind = Kind::LootGenerationGlobalParams;
const SLOT: &str = "libs/foundry/records/harvestable/slotpresets/catalogued";
const OTHER: &str = "libs/foundry/records/harvestable/slotpresets/dev/hidden-projection";

#[test]
fn loot_context_definitions_share_one_record_with_the_legacy_multi_projection() -> TestResult {
    let bytes = database();
    let fixture = Fixture::open(&bytes, Input::Encrypted, Vec::new())?;
    source_declarations(&fixture)?;
    let built = build_catalog_from(&fixture.archive, &fixture.entry, &options())?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let multi = catalog
        .get(MULTI_KIND.name())
        .ok_or("legacy multi dataset")?;
    let [projection] = multi
        .as_array()
        .ok_or("legacy multi projections")?
        .as_slice()
    else {
        return Err("only the original slotpreset qualifies for the legacy projection".into());
    };
    assert_eq!(
        projection.get("guid"),
        Some(&json!(MULTI_GUID)),
        "same physical owner"
    );
    assert_eq!(
        projection.get("ignoreAttachableTags"),
        Some(&json!(true)),
        "legacy projection value"
    );
    let health = built
        .summary()
        .export_health()
        .kind(MULTI_KIND)
        .ok_or("multi kind health")?;
    assert_eq!(
        health.source_records.selected, 2,
        "only two physical owner Records are selected"
    );
    assert_eq!(
        health.written_rows, 2,
        "one counted raw row per physical owner"
    );
    assert_eq!(
        health.source_records.projected, 2,
        "extra rendering does not duplicate disposition"
    );
    for guid in [MULTI_GUID, EXCLUDED_GUID] {
        let source = Graph::in_table(&catalog, MULTI_KIND.name(), guid)?;
        check_theme_reference(&source, "multiConfig")?;
    }
    let single = Graph::in_table(&catalog, SINGLE_KIND.name(), SINGLE_GUID)?;
    check_theme_reference(&single, "subConfig")?;
    assert_context_and_global(&catalog)
}

#[test]
fn loot_context_definitions_retain_exact_theme_targets_under_a_context_filter() -> TestResult {
    let fixture = Fixture::open(&database(), Input::Plain, Vec::new())?;
    let filter = Filter::parse("synthetic/dev/context")?;
    let mut selected = options();
    selected.filter = Some(&filter);
    let built = build_catalog_from(&fixture.archive, &fixture.entry, &selected)?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let excluded = catalog
        .pointer("/definitions/lootThemes")
        .ok_or("declared theme dataset")?;
    assert_eq!(
        excluded.get("availability"),
        Some(&json!("available")),
        "declared target family"
    );
    assert_eq!(
        graph::array(excluded, "rows")?,
        &[] as &[Value],
        "filter omits target body"
    );
    let context = context(&catalog)?;
    check_context(&context)
}

fn assert_context_and_global(catalog: &Value) -> TestResult {
    check_context(&context(catalog)?)?;
    let global = Graph::in_table(catalog, GLOBAL_KIND.name(), GLOBAL_GUID)?;
    let order = graph::payload(global.root("themeCategoryOrdering")?, "struct")?;
    let tags = global.field(graph::array(order, "fields")?, "orderedCategories")?;
    let members = graph::array(graph::payload(tags, "array")?, "elements")?;
    for (member, expected) in members.iter().zip([4, 1, 4]) {
        let numeric = graph::payload(global.follow(member)?, "number")?;
        assert_eq!(
            numeric.pointer("/value/value"),
            Some(&json!(expected)),
            "global ordered tag slot"
        );
    }
    assert_eq!(members.len(), 3, "global array preserves duplicates");
    Ok(())
}

fn context(catalog: &Value) -> TestResult<Graph<'_>> {
    Graph::in_table(catalog, CONTEXT_KIND.name(), CONTEXT_GUID)
}

fn check_context(context: &Graph<'_>) -> TestResult {
    let hcds = graph::array(graph::payload(context.root("HCDs")?, "array")?, "elements")?;
    assert_eq!(hcds.len(), 1, "one physical contextual element");
    let element = graph::payload(
        context.follow(hcds.first().ok_or("owned contextual element")?)?,
        "struct",
    )?;
    let fields = graph::array(element, "fields")?;
    let ignore = context.field(fields, "parentThemesToIgnore")?;
    assert_eq!(
        ignore.pointer("/value/state"),
        Some(&json!("null")),
        "unset parent selector stays null"
    );
    check_reference_array(context, context.field(fields, "themes")?)
}

fn check_theme_reference(graph: &Graph<'_>, field: &str) -> TestResult {
    let config = graph::payload(graph.root(field)?, "struct")?;
    check_reference_array(
        graph,
        graph.field(graph::array(config, "fields")?, "lootThemes")?,
    )
}

fn check_reference_array(graph: &Graph<'_>, value: &Value) -> TestResult {
    let elements = graph::array(graph::payload(value, "array")?, "elements")?;
    let Some(source) = elements.first() else {
        return Err("actual stored theme slot".into());
    };
    let stored = graph::payload(graph.follow(source)?, "recordReference")?;
    assert_eq!(
        stored.pointer("/value/guid/value"),
        Some(&json!(THEME_GUID)),
        "authored theme GUID"
    );
    assert_eq!(
        stored.pointer("/value/targetRecordIndex/value"),
        Some(&json!(5)),
        "original target header index"
    );
    Ok(())
}

fn source_declarations(fixture: &Fixture) -> TestResult {
    let forge = fixture.entry.open()?;
    assert_eq!(
        forge.records().len(),
        6,
        "independent original Record population"
    );
    for (index, name, field, data_type, array) in [
        (0, SINGLE, "subConfig", DataType::Class, false),
        (1, MULTI, "multiConfig", DataType::Class, false),
        (2, MULTI, "multiConfig", DataType::Class, false),
        (3, CONTEXT, "HCDs", DataType::StrongPointer, true),
        (4, GLOBAL, "themeCategoryOrdering", DataType::Class, false),
    ] {
        let record = forge.record(index).ok_or("genuine physical Record")?;
        assert_eq!(record.type_name()?, name, "original owner type");
        let tree = record.tree()?;
        let Some(field) = tree.fields().iter().find(|item| item.name() == field) else {
            return Err("genuine original field".into());
        };
        assert_eq!(
            field.storage().data_type(),
            data_type,
            "original element storage"
        );
        assert_eq!(
            field.storage().is_array(),
            array,
            "original cardinality declaration"
        );
        assert_eq!(
            tree.error_count(),
            0,
            "fixture spellings decode independently"
        );
    }
    Ok(())
}

fn database() -> Vec<u8> {
    schema()
        .record(
            TestRecord::new(SINGLE_ID, SINGLE, "Fixture.Single", "synthetic/single.xml")
                .value("subConfig", config("SubHarvestableConfig")),
        )
        .record(
            TestRecord::new(MULTI_ID, MULTI, "Fixture.Multi", format!("{SLOT}.xml"))
                .value("multiConfig", config("SubHarvestableMultiConfig")),
        )
        .record(
            TestRecord::new(EXCLUDED_ID, MULTI, "Fixture.Dev", format!("{OTHER}.xml"))
                .value("multiConfig", config("SubHarvestableMultiConfig")),
        )
        .record(
            TestRecord::new(
                CONTEXT_ID,
                CONTEXT,
                "Fixture.Context",
                "synthetic/dev/context.xml",
            )
            .value(
                "HCDs",
                TestValue::Array(vec![TestValue::Struct(
                    TestInstance::new("LocationLootGenerationContextData")
                        .value("themes", themes()),
                )]),
            ),
        )
        .record(
            TestRecord::new(GLOBAL_ID, GLOBAL, "Fixture.Global", "synthetic/global.xml").value(
                "themeCategoryOrdering",
                TestValue::Struct(TestInstance::new("TagList").value(
                    "orderedCategories",
                    TestValue::Array([4, 1, 4].into_iter().map(TestValue::Int32).collect()),
                )),
            ),
        )
        .record(TestRecord::new(
            THEME_ID,
            "LootThemeRecord",
            "Fixture.Theme",
            "synthetic/theme.xml",
        ))
        .build()
}

fn config(name: &str) -> TestValue {
    let original = TestInstance::new(name).value("lootThemes", themes());
    TestValue::Struct(if name == "SubHarvestableMultiConfig" {
        original.value(
            "ignoreAttachableTagsForTaggedConfigs",
            TestValue::Bool(true),
        )
    } else {
        original
    })
}

fn themes() -> TestValue {
    TestValue::Array(vec![TestValue::Reference(THEME_ID)])
}

fn schema() -> DataForgeBuilder {
    let theme = TestType::ReferenceTo("LootThemeRecord".into());
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("LootThemeRecord"))
        .structure(TestStruct::new("SubHarvestableConfig").array("lootThemes", theme.clone()))
        .structure(
            TestStruct::new(SINGLE)
                .field("subConfig", TestType::Class("SubHarvestableConfig".into())),
        )
        .structure(
            TestStruct::new("SubHarvestableMultiConfig")
                .array("lootThemes", theme.clone())
                .field("ignoreAttachableTagsForTaggedConfigs", TestType::Bool),
        )
        .structure(TestStruct::new(MULTI).field(
            "multiConfig",
            TestType::Class("SubHarvestableMultiConfig".into()),
        ))
        .structure(TestStruct::new("LootThemeSelector_Base"))
        .structure(TestStruct::new("HierarchicalContextualData"))
        .structure(
            TestStruct::new("LocationLootGenerationContextData")
                .parent("HierarchicalContextualData")
                .array("themes", theme)
                .field(
                    "parentThemesToIgnore",
                    TestType::StrongPointer("LootThemeSelector_Base".into()),
                ),
        )
        .structure(TestStruct::new(CONTEXT).array(
            "HCDs",
            TestType::StrongPointer("HierarchicalContextualData".into()),
        ))
        .structure(TestStruct::new("TagList").array("orderedCategories", TestType::Int32))
        .structure(
            TestStruct::new(GLOBAL)
                .field("themeCategoryOrdering", TestType::Class("TagList".into())),
        )
}
