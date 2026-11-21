// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{BuiltCatalog, ExportHealthStatus, Kind, build_catalog_from};
use p4kx_extraction::{DataType, Filter, SourceUseRole, Value as OriginalValue};
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};
use serde_json::{Value, json};

use crate::health_origin_fixture::{Fixture, Input, TestResult, options};
use crate::loot_v3_graph::{self as graph, Graph};
use crate::loot_v3_origin::{self as origin, ExpectedOrigin};

const SINGLE: &str = "LootV3SecondaryChoicesSingleLayerRecord";
const MULTI: &str = "LootV3SecondaryChoicesMultiLayerRecord";
const THEME: &str = "LootThemeRecord";
const SINGLE_KIND: Kind = Kind::LootSecondaryChoiceSingleLayers;
const MULTI_KIND: Kind = Kind::LootSecondaryChoiceMultiLayers;
const THEME_KIND: Kind = Kind::LootThemes;
const SINGLE_ID: u128 = 0xd847_0000_0000_4000_8000_0000_0000_0001;
const MULTI_ID: u128 = 0xd847_0000_0000_4000_8000_0000_0000_0002;
const THEME_ID: u128 = 0xd847_0000_0000_4000_8000_0000_0000_0003;
const SINGLE_GUID: &str = "d8470000-0000-4000-8000-000000000001";
const MULTI_GUID: &str = "d8470000-0000-4000-8000-000000000002";
const THEME_GUID: &str = "d8470000-0000-4000-8000-000000000003";
const CHOICE: &str = "LootV3SecondaryChoiceEntry";
const THEME_BASE: &str = "LootTheme_NoRef";
const THEME_VARIANT: &str = "LootTheme_TestVariant";
const SINGLE_FILE: &str = "synthetic-other-folder/single";
const NAN_BITS: u32 = 0x7fc0_0047;

#[test]
fn loot_secondary_definitions_keep_exact_distinct_owners_and_encrypted_cells() -> TestResult {
    let input = Input::Encrypted;
    let fixture = Fixture::open(&database(), input, Vec::new())?;
    check_original(&fixture)?;
    let built = build_catalog_from(&fixture.archive, &fixture.entry, &options())?;
    check_health(&built)?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let single = Graph::in_table(&catalog, SINGLE_KIND.name(), SINGLE_GUID)?;
    let choices = graph::array(
        graph::payload(single.root("choices")?, "array")?,
        "elements",
    )?;
    let choice = graph::payload(
        single.follow(choices.first().ok_or("choice slot")?)?,
        "struct",
    )?;
    let weight = single.field(graph::array(choice, "fields")?, "weight")?;
    let payload = graph::payload(weight, "number")?;
    assert_eq!(
        payload.pointer("/value/type"),
        Some(&json!("f32")),
        "the exact original numeric width"
    );
    assert_eq!(
        payload.pointer("/value/rawBits"),
        Some(&json!("7fc00047")),
        "the exact stored nonfinite token has its own emitted receipt"
    );
    origin::verify(
        weight.get("value").ok_or("weight datum")?,
        built.summary().source_inventory(),
        &ExpectedOrigin {
            encrypted: input.marker(),
            complete: true,
            uses: &[origin::value_use(2, "$value", SourceUseRole::Payload)],
        },
    )
    .map_err(|problem| format!("actual encrypted original weight: {problem:?}"))?;

    let theme = Graph::in_table(&catalog, THEME_KIND.name(), THEME_GUID)?;
    let owned = graph::payload(theme.root("theme")?, "struct")?;
    assert_eq!(
        owned.get("sourceType").map(graph::present).transpose()?,
        Some(&json!(THEME_VARIANT)),
        "resolved owned theme type"
    );
    let own_fields = graph::array(owned, "fields")?;
    let extra = graph::payload(theme.field(own_fields, "futureFlag")?, "boolean")?;
    assert_eq!(
        extra.get("value"),
        Some(&json!(false)),
        "forward member retains stored false"
    );
    check_layers(&catalog)
}

#[test]
fn loot_secondary_definitions_keep_reference_slots_when_target_body_is_filtered() -> TestResult {
    let fixture = Fixture::open(&database(), Input::Plain, Vec::new())?;
    let filter = Filter::parse("synthetic-selection/layers")?;
    let mut selected = options();
    selected.filter = Some(&filter);
    let built = build_catalog_from(&fixture.archive, &fixture.entry, &selected)?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    check_layers(&catalog)?;
    for omitted in [SINGLE_KIND, THEME_KIND] {
        let dataset = catalog
            .pointer(&format!("/definitions/{}", omitted.name()))
            .ok_or("declared kind filtered out")?;
        assert_eq!(
            dataset.get("availability"),
            Some(&json!("available")),
            "a filter leaves the declaration available"
        );
        assert_eq!(
            graph::array(dataset, "rows")?,
            &[] as &[Value],
            "only the requested owner has a row"
        );
    }
    let health = built.summary().export_health();
    assert_eq!(
        health.kind(MULTI_KIND).map(|kind| kind.written_rows),
        Some(1),
        "selected multi owner appears once"
    );
    assert_eq!(
        health.kind(SINGLE_KIND).map(|kind| kind.written_rows),
        Some(0),
        "the target's definition is outside the filter"
    );
    Ok(())
}

fn check_health(built: &BuiltCatalog) -> TestResult {
    let health = built.summary().export_health();
    assert_eq!(
        health.status,
        ExportHealthStatus::Complete,
        "readable exact source allows normal publication"
    );
    for kind in [SINGLE_KIND, MULTI_KIND, THEME_KIND] {
        let kind_health = health.kind(kind).ok_or("registered physical Loot kind")?;
        assert_eq!(
            kind_health.source_records.selected, 1,
            "one selected source for {kind}"
        );
        assert_eq!(
            kind_health.source_records.projected, 1,
            "one accounted source for {kind}"
        );
        assert_eq!(kind_health.written_rows, 1, "one physical row for {kind}");
        assert!(
            kind_health.field_inventory_complete,
            "original fields of {kind} fully inspected"
        );
    }
    Ok(())
}

fn check_layers(catalog: &Value) -> TestResult {
    let multi = Graph::in_table(catalog, MULTI_KIND.name(), MULTI_GUID)?;
    let layers = graph::array(graph::payload(multi.root("layers")?, "array")?, "elements")?;
    let [first, null, repeated] = layers else {
        return Err("three original layer slots including one null".into());
    };
    assert_ne!(
        first, repeated,
        "equal references occupy distinct original slots"
    );
    for reference in [first, repeated] {
        let payload = graph::payload(multi.follow(reference)?, "recordReference")?;
        assert_eq!(
            payload.pointer("/value/guid/value"),
            Some(&json!(SINGLE_GUID)),
            "the exact declared target GUID"
        );
        assert_eq!(
            payload.pointer("/value/targetRecordIndex/value"),
            Some(&json!(0)),
            "the actual target position"
        );
        assert_eq!(
            payload.pointer("/value/recordPath/value"),
            Some(&json!(SINGLE_FILE)),
            "the actual target path before filtering"
        );
    }
    assert_eq!(
        multi.follow(null)?.pointer("/value/state"),
        Some(&json!("null")),
        "the original null slot remains null"
    );
    Ok(())
}

fn check_original(fixture: &Fixture) -> TestResult {
    let forge = fixture.entry.open()?;
    assert_eq!(
        forge.record_count(),
        3,
        "all three physical owners precede export"
    );
    for (record, (name, field_name, data_type, array)) in forge.records().zip([
        (SINGLE, "choices", DataType::Class, true),
        (MULTI, "layers", DataType::Reference, true),
        (THEME, "theme", DataType::StrongPointer, false),
    ]) {
        assert_eq!(record.type_name()?, name, "actual original wrapper type");
        let tree = record.tree()?;
        assert!(tree.is_from(&record), "original tree belongs to its Record");
        assert_eq!(tree.error_count(), 0, "all original wrapper fields decode");
        let [field] = tree.fields() else {
            return Err("one original wrapper declaration".into());
        };
        assert_eq!(field.name(), field_name, "physical declaration name");
        assert_eq!(
            field.storage().data_type(),
            data_type,
            "actual declared field type"
        );
        assert_eq!(
            field.storage().is_array(),
            array,
            "actual declared array flag"
        );
        check_original_value(name, field.value())?;
    }
    Ok(())
}

fn check_original_value(name: &str, value: &OriginalValue<'_>) -> TestResult {
    match (name, value) {
        (SINGLE, OriginalValue::Array(values)) => {
            let Some(OriginalValue::Struct(choice)) = values.first() else {
                return Err("original inline choice".into());
            };
            let Some(OriginalValue::Single(weight)) = choice.field("weight") else {
                return Err("original Single weight".into());
            };
            assert_eq!(
                weight.to_bits(),
                NAN_BITS,
                "independent original weight bits"
            );
        }
        (MULTI, OriginalValue::Array(values)) => {
            let [
                OriginalValue::Reference(first),
                OriginalValue::Null,
                OriginalValue::Reference(repeated),
            ] = values.as_ref()
            else {
                return Err("original ordered repeated and null Reference slots".into());
            };
            for reference in [first, repeated] {
                assert_eq!(
                    reference.original_target().record_guid().to_string(),
                    SINGLE_GUID,
                    "independent actual target GUID"
                );
                assert_eq!(
                    reference.original_target().record_index(),
                    0,
                    "actual physical target ordinal"
                );
            }
        }
        (THEME, OriginalValue::Struct(theme)) => {
            assert_eq!(
                theme.type_name(),
                THEME_VARIANT,
                "original nested concrete type"
            );
            assert_eq!(
                theme.field("futureFlag"),
                Some(&OriginalValue::Bool(false)),
                "original forward field"
            );
        }
        _ => return Err("original physical wrapper shape".into()),
    }
    Ok(())
}

fn database() -> Vec<u8> {
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new(CHOICE).field("weight", TestType::Single))
        .structure(TestStruct::new(SINGLE).array("choices", TestType::Class(CHOICE.into())))
        .structure(TestStruct::new(MULTI).array("layers", TestType::ReferenceTo(SINGLE.into())))
        .structure(TestStruct::new(THEME_BASE))
        .structure(
            TestStruct::new(THEME_VARIANT)
                .parent(THEME_BASE)
                .field("futureFlag", TestType::Bool),
        )
        .structure(
            TestStruct::new(THEME).field("theme", TestType::StrongPointer(THEME_BASE.into())),
        )
        .record(
            TestRecord::new(
                SINGLE_ID,
                SINGLE,
                "Synthetic.Single",
                format!("{SINGLE_FILE}.xml"),
            )
            .value(
                "choices",
                TestValue::Array(vec![TestValue::Struct(
                    TestInstance::new(CHOICE)
                        .value("weight", TestValue::Single(f32::from_bits(NAN_BITS))),
                )]),
            ),
        )
        .record(
            TestRecord::new(
                MULTI_ID,
                MULTI,
                "Synthetic.Multi",
                "synthetic-selection/layers.xml",
            )
            .value(
                "layers",
                TestValue::Array(vec![
                    TestValue::Reference(SINGLE_ID),
                    TestValue::Null,
                    TestValue::Reference(SINGLE_ID),
                ]),
            ),
        )
        .record(
            TestRecord::new(
                THEME_ID,
                THEME,
                "Synthetic.Theme",
                "synthetic-other-folder/theme.xml",
            )
            .value(
                "theme",
                TestValue::Struct(
                    TestInstance::new(THEME_VARIANT).value("futureFlag", TestValue::Bool(false)),
                ),
            ),
        )
        .build()
}
