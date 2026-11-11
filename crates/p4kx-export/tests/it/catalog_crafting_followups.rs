// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The reward-source scan, its warnings and progress, and the filtered links of the crafting
//! catalog, built from the synthetic crafting DataForge.

use crate::catalog_crafting_support::{blueprint, build_from, build_with, text};
use crate::crafting_detail_fixture as fixture;

use std::error::Error;
use std::sync::Mutex;

use p4kx_export::{CatalogPhase, CatalogProgress, CraftingWarning, Filter, read_crafting};
use p4kx_extraction::{DataForge, ReferenceIndex};
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};
use serde_json::json;

type TestResult = Result<(), Box<dyn Error>>;

const BROKEN: u128 = 0x9100_0000_0000_4000_8000_0000_0000_0001;
const POOL_RECORD: u128 = 0x9300_0000_0000_4000_8000_0000_0000_0003;
const PLACEHOLDER_PROPERTY: u128 = 0x9200_0000_0000_4000_8000_0000_0000_0002;

fn broken_struct(name: &str) -> TestStruct {
    TestStruct::new(name)
        .field("a", TestType::Int8)
        .declared_size(3)
}

#[test]
fn the_crafting_step_reports_progress_before_the_record_scan() -> TestResult {
    let seen = Mutex::new(Vec::new());
    let report = |progress: CatalogProgress| {
        if let Ok(mut seen) = seen.lock() {
            seen.push(progress);
        }
    };
    build_with(&fixture::database(), None, Some(&report))?;
    let seen = seen.into_inner().map_err(|_| "poisoned")?;
    let last_crafting = seen
        .iter()
        .rposition(|progress| progress.phase == CatalogPhase::Crafting)
        .ok_or("a crafting report")?;
    let first_records = seen
        .iter()
        .position(|progress| progress.phase == CatalogPhase::Records)
        .ok_or("a record report")?;
    assert!(
        last_crafting < first_records,
        "the steps run one after the other"
    );
    let finished = seen
        .iter()
        .filter(|progress| progress.phase == CatalogPhase::Crafting)
        .max()
        .ok_or("a crafting report")?;
    assert_eq!(
        finished.records_done, finished.records_total,
        "the crafting step reaches every Record"
    );
    assert!(finished.records_total > 0);
    Ok(())
}

#[test]
fn an_unreadable_record_of_an_unrelated_type_is_no_warning() -> TestResult {
    let forge = fixture::database()
        .structure(broken_struct("Broken"))
        .record(TestRecord::new(
            BROKEN,
            "Broken",
            "Broken.one",
            "libs/foundry/records/broken/one.xml",
        ));
    let bytes = forge.build();
    let forge = DataForge::open(&bytes)?;
    assert_eq!(
        ReferenceIndex::build(&forge).unreadable_records(),
        1,
        "the Record cannot be read"
    );
    let crafting = read_crafting(&forge, None, None)?;
    assert!(crafting.warnings.is_empty(), "{:?}", crafting.warnings);
    assert!(crafting.failures.is_empty(), "{:?}", crafting.failures);
    assert!(!crafting.reward_sources.is_empty(), "the sources are found");
    Ok(())
}

fn lone_pool() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("BlueprintPoolRecord"))
        .record(TestRecord::new(
            POOL_RECORD,
            "BlueprintPoolRecord",
            "BlueprintPoolRecord.Lone",
            "libs/foundry/records/crafting/blueprintrewards/lone.xml",
        ))
        .structure(broken_struct("Broken"))
        .record(TestRecord::new(
            BROKEN,
            "Broken",
            "Broken.one",
            "libs/foundry/records/broken/one.xml",
        ))
}

#[test]
fn when_no_source_is_found_every_unreadable_record_is_a_warning() -> TestResult {
    let bytes = lone_pool().build();
    let forge = DataForge::open(&bytes)?;
    let crafting = read_crafting(&forge, None, None)?;
    assert_eq!(crafting.reward_sources.len(), 0);
    assert_eq!(
        crafting.warnings,
        [CraftingWarning::UnreadableRewardRecords(1)]
    );
    Ok(())
}

#[test]
fn a_pool_that_refers_to_itself_is_no_reward_source() -> TestResult {
    let forge = fixture::database().record(
        TestRecord::new(
            POOL_RECORD,
            "BlueprintPoolRecord",
            "BlueprintPoolRecord.Selfish",
            "libs/foundry/records/crafting/blueprintrewards/selfish.xml",
        )
        .value("selfLink", TestValue::Reference(POOL_RECORD)),
    );
    let bytes = forge.build();
    let forge = DataForge::open(&bytes)?;
    let crafting = read_crafting(&forge, None, None)?;
    assert!(
        crafting
            .reward_sources
            .iter()
            .all(|source| source.pool != fixture_guid(POOL_RECORD)),
        "{:?}",
        crafting.reward_sources
    );
    assert_eq!(crafting.reward_sources.len(), 5, "the contracts' own");
    Ok(())
}

fn fixture_guid(value: u128) -> p4kx_extraction::Guid {
    text(value).parse().unwrap_or_default()
}

#[test]
fn the_reward_sources_are_complete_and_ordered_by_record() -> TestResult {
    let bytes = fixture::database().build();
    let forge = DataForge::open(&bytes)?;
    let crafting = read_crafting(&forge, None, None)?;
    let pools: Vec<_> = crafting
        .reward_sources
        .iter()
        .map(|source| source.pool)
        .collect();
    assert_eq!(pools.len(), 5, "every reward of the contracts: {pools:?}");
    assert!(
        crafting
            .reward_sources
            .iter()
            .all(|source| source.record_path.ends_with("haul")),
        "all in the one contract Record"
    );
    Ok(())
}

#[test]
fn a_blueprint_links_no_dismantle_process_a_filter_left_out() -> TestResult {
    let (_, whole) = build_from(&fixture::database(), None)?;
    let linked =
        &blueprint::by_guid(&whole, fixture::TWO_TIERS)?.presentation["dismantle"]["value"];
    assert_eq!(linked["processGuid"], json!(text(fixture::DISMANTLE)));
    let filter = Filter::new("*/bp_craft_two_tiers");
    let (_, filtered) = build_from(&fixture::database(), Some(&filter))?;
    assert_eq!(
        filtered["dismantleProcesses"],
        json!([]),
        "the process is out"
    );
    assert_eq!(
        blueprint::by_guid(&filtered, fixture::TWO_TIERS)?.presentation["dismantle"]["state"],
        "absent",
        "so no blueprint points at it"
    );
    Ok(())
}

#[test]
fn a_placeholder_property_name_keeps_its_key_like_every_other_kind() -> TestResult {
    let forge = fixture::database().record(
        TestRecord::new(
            PLACEHOLDER_PROPERTY,
            "CraftingGameplayPropertyDef",
            "CraftingGameplayPropertyDef.GPP_Placeholder",
            "libs/foundry/records/crafting/craftedproperties/gpp_placeholder.xml",
        )
        .value("propertyName", TestValue::text("@LOC_PLACEHOLDER")),
    );
    let property_source = crate::catalog_crafting_property::original(
        &forge,
        PLACEHOLDER_PROPERTY,
        "@LOC_PLACEHOLDER",
    )?;
    let (built, _) = build_from(&forge, None)?;
    crate::catalog_crafting_property::assert_placeholder(&built, &property_source)
}

fn unreadable_property() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .structure(broken_struct("CraftingGameplayPropertyDef"))
        .record(TestRecord::new(
            BROKEN,
            "CraftingGameplayPropertyDef",
            "CraftingGameplayPropertyDef.Broken",
            "libs/foundry/records/crafting/craftedproperties/broken.xml",
        ))
}

#[test]
fn a_record_outside_the_filter_is_no_failure_of_the_selection() -> TestResult {
    let bytes = unreadable_property().build();
    let forge = DataForge::open(&bytes)?;
    let whole = read_crafting(&forge, None, None)?;
    assert_eq!(whole.failures.len(), 1, "unreadable without a Filter");
    let filter = Filter::new("*/bp_craft_pick");
    let selected = read_crafting(&forge, Some(&filter), None)?;
    assert!(selected.failures.is_empty(), "{:?}", selected.failures);
    let matching = Filter::new("*/broken");
    let kept = read_crafting(&forge, Some(&matching), None)?;
    assert_eq!(kept.failures.len(), 1, "inside the Filter it still fails");
    Ok(())
}
