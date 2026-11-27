// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use p4kx_extraction::{DataForge, DataForgeEntry, RecordTree, StructValue, Value as Raw};
use p4kx_testkit::{TestInstance, TestRecord, TestValue};
use serde_json::{Value, json};

use crate::catalog_crafting_support::blueprint;
use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::crafting_detail;
use crate::crafting_detail_fixture as fixture;

const ABSENT: u128 = 0xa600_0000_0000_4000_8000_0000_0006;

#[test]
fn dismantle_return_uses_its_actual_three_source_gates() -> TestResult {
    let (catalog, sources) = observed(fixture::TWO_TIERS, Some(fixture::PRODUCT_A))?;
    let row = blueprint::by_guid(&catalog, fixture::TWO_TIERS)?;
    let cell = &row.presentation["dismantle"];
    assert_eq!(cell["state"], "present");
    assert_eq!(
        cell["value"]["processGuid"],
        common::guid(fixture::DISMANTLE)
    );
    assert_eq!(cell["value"]["tier"], row.presentation["firstTier"]);
    assert_eq!(
        owners(&catalog, cell)?,
        BTreeSet::from([sources.blueprint, sources.global, sources.process])
    );
    Ok(())
}

#[test]
fn blacklisted_item_stops_dismantle_evidence_before_the_process() -> TestResult {
    let (catalog, sources) = observed(fixture::BARE, Some(fixture::BLACKLISTED))?;
    let row = blueprint::by_guid(&catalog, fixture::BARE)?;
    let cell = &row.presentation["dismantle"];
    assert_eq!(cell["state"], "absent");
    assert_eq!(
        owners(&catalog, cell)?,
        BTreeSet::from([sources.blueprint, sources.global]),
        "the blacklist is decisive before consulting any process"
    );
    Ok(())
}

#[test]
fn absent_product_stops_dismantle_evidence_before_the_blacklist() -> TestResult {
    let (catalog, sources) = observed(ABSENT, None)?;
    let row = blueprint::by_guid(&catalog, ABSENT)?;
    let cell = &row.presentation["dismantle"];
    assert_eq!(cell["state"], "absent");
    assert_eq!(
        owners(&catalog, cell)?,
        BTreeSet::from([sources.blueprint]),
        "the actual absent product is decisive before any other Record"
    );
    Ok(())
}

#[test]
fn dismantle_process_keeps_exact_duration_inputs_and_their_conversion() -> TestResult {
    use crate::health_origin_assertions as proof;

    let (catalog, sources) = observed(fixture::TWO_TIERS, Some(fixture::PRODUCT_A))?;
    let [row] = proof::rows(&catalog, "/definitions/dismantleProcesses/rows")? else {
        return Err("one physical dismantle definition".into());
    };
    let header = proof::present(
        row,
        "guid",
        &json!(common::guid(fixture::DISMANTLE)),
        Some(true),
    )?;
    proof::header(
        &catalog,
        header,
        sources.process,
        "guid",
        "payload",
        "archiveEntry",
    )?;
    let process = proof::at(row, "/blueprint/value/processSpecificData/value")?;
    proof::present(
        process,
        "efficiency",
        &json!({"type": "f32", "rawBits": "3f000000", "finite": 0.5}),
        Some(true),
    )?;
    verify_dismantle_duration(&catalog, process, sources.process)?;
    proof::closure(&catalog)
}

fn verify_dismantle_duration(catalog: &Value, process: &Value, record_index: u32) -> TestResult {
    use crate::health_origin_assertions as proof;

    let elapsed = proof::present(process, "timeSeconds", &json!("15"), Some(true))?;
    let conversion_uses = proof::uses(catalog, elapsed)?;
    let duration = proof::at(process, "/dismantleTime/value")?;
    let zero = json!({"type": "int32", "value": 0});
    let stored_seconds = json!({"type": "f32", "rawBits": "416c0000", "finite": 14.75});
    for (field, expected) in [
        ("days", &zero),
        ("hours", &zero),
        ("minutes", &zero),
        ("seconds", &stored_seconds),
    ] {
        let cell = proof::present(duration, field, expected, Some(true))?;
        let inputs = proof::uses(catalog, cell)?;
        let payload = inputs
            .iter()
            .find(|used| {
                used["role"] == "payload"
                    && used.pointer("/location/field").and_then(Value::as_str) == Some("$value")
            })
            .ok_or("original duration component payload")?;
        assert_eq!(
            payload
                .pointer("/location/indices/0")
                .and_then(Value::as_str),
            Some(record_index.to_string().as_str()),
            "the original duration belongs to the selected physical process"
        );
        assert!(
            conversion_uses.iter().any(|used| {
                used["role"] == "conversion" && used["location"] == payload["location"]
            }),
            "the converted seconds cites the exact authored component {field}"
        );
    }
    Ok(())
}

struct Physical {
    blueprint: u32,
    global: u32,
    process: u32,
}

fn observed(guid: u128, product: Option<u128>) -> TestResult<(Value, Physical)> {
    let database = fixture::database().record(
        TestRecord::new(
            ABSENT,
            "CraftingBlueprintRecord",
            "CraftingBlueprintRecord.BP_CRAFT_no_product",
            "libs/foundry/records/crafting/blueprints/crafting/bp_craft_no_product.xml",
        )
        .value(
            "blueprint",
            TestValue::Struct(TestInstance::new("CraftingBlueprintBase")),
        ),
    );
    let (_directory, archive) = common::open(&database, DcbOrigin::Encrypted)?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let sources = original(&forge, guid, product)?;
    let built = p4kx_export::build_catalog_from(
        &archive,
        &entry,
        common::options().with_full_provenance(),
    )?;
    assert!(
        built.summary().is_complete(),
        "every original source is healthy"
    );
    let json = serde_json::from_slice(built.json())?;
    Ok((json, sources))
}

fn original(forge: &DataForge<'_>, guid: u128, product: Option<u128>) -> TestResult<Physical> {
    let blueprint = tree(forge, guid)?;
    let source = structure(blueprint.root(), "blueprint")?;
    if let Some(expected) = product {
        let process = structure(source, "processSpecificData")?;
        let Some(Raw::Reference(link)) = process.field("entityClass") else {
            return Err("actual authored entityClass Reference".into());
        };
        assert_eq!(
            link.guid(),
            crafting_detail::guid(expected),
            "the original produced item has the expected link"
        );
    } else {
        assert_eq!(
            source.field("processSpecificData"),
            None,
            "the actual Blueprint does not declare a product"
        );
    }
    let global = tree(forge, fixture::GLOBAL)?;
    let Some(Raw::Array(blacklist)) = global.root().field("dismantleBlacklistEntityClasses") else {
        return Err("actual blacklist array".into());
    };
    let [Raw::Reference(excluded)] = blacklist.as_ref() else {
        return Err("one actual exclusion".into());
    };
    assert_eq!(
        excluded.guid(),
        crafting_detail::guid(fixture::BLACKLISTED),
        "the original blacklist names the excluded item"
    );
    let process = tree(forge, fixture::DISMANTLE)?;
    let process_body = structure(
        structure(process.root(), "blueprint")?,
        "processSpecificData",
    )?;
    assert_eq!(
        process_body.type_name(),
        "GenericCraftingProcess_Dismantle",
        "the participating process is the original dismantle record"
    );
    Ok(Physical {
        blueprint: blueprint.record_index(),
        global: global.record_index(),
        process: process.record_index(),
    })
}

fn tree<'source>(
    forge: &'source DataForge<'source>,
    guid: u128,
) -> TestResult<RecordTree<'source>> {
    let record = forge
        .record_by_guid(crafting_detail::guid(guid))
        .ok_or("actual physical Record")?;
    let tree = record.tree()?;
    assert!(
        tree.is_from(&record),
        "the decoded tree belongs to the exact original Record"
    );
    Ok(tree)
}

fn structure<'read, 'source>(
    parent: &'read StructValue<'source>,
    name: &str,
) -> TestResult<&'read StructValue<'source>> {
    match parent.field(name) {
        Some(Raw::Struct(value)) => Ok(value),
        _ => Err(format!("actual original {name} Struct").into()),
    }
}

fn owners(catalog: &Value, cell: &Value) -> TestResult<BTreeSet<u32>> {
    assert_eq!(
        cell["containsEncryptedSource"], true,
        "the dismantle value retains its encrypted origin"
    );
    assert_eq!(
        cell["sourceSetComplete"], true,
        "the consulted source set is complete"
    );
    let inventory = catalog
        .pointer("/provenance/uses")
        .and_then(Value::as_array)
        .ok_or("actual use inventory")?;
    let mut found = BTreeSet::new();
    for reference in cell["sourceUsesRefs"].as_array().ok_or("immediate uses")? {
        let used = inventory
            .iter()
            .find(|used| &used["id"] == reference)
            .ok_or("closed immediate use")?;
        assert_eq!(
            used["scope"], "payload",
            "the dismantle origin is an authored value input"
        );
        let location = &used["location"];
        let owner = location
            .pointer("/indices/0")
            .and_then(Value::as_str)
            .ok_or("physical subresource Record index")?
            .parse()?;
        found.insert(owner);
    }
    Ok(found)
}
