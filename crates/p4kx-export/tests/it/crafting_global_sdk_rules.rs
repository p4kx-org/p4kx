// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::crafting;
use p4kx_export::catalog_queries::{self, CatalogRecordSelection};
use p4kx_export::{CatalogOptions, CatalogPublicationMode, Crafting, build_catalog, read_crafting};
use p4kx_extraction::{Archive, DataForgeEntry, Filter};
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};
use serde_json::Value;

use super::{fixture, walk};
use crate::catalog_crafting_support::blueprint;
use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::crafting_detail_fixture as detail;
use crate::sdk_full_proof as full_proof;

#[test]
fn multiple_original_global_owners_preserve_blueprint_defaults_and_dismantle_rules() -> TestResult {
    let database = detail::database()
        .record(global("Master", detail::TWO_TIERS, Vec::new()))
        .record(global(
            "Repair",
            detail::BARE,
            vec![TestValue::Reference(detail::PRODUCT_A)],
        ));
    let (_folder, archive) = common::open(&database, DcbOrigin::Plain)?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let interpreted = read_crafting(&forge, None, None)?;
    let mut options = CatalogOptions::new(std::time::UNIX_EPOCH);
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    let catalog: Value =
        serde_json::from_slice(build_catalog(&archive, options.with_full_provenance())?.json())?;
    assert!(
        catalog.get("craftingGlobal").is_none(),
        "no merged compatibility root"
    );
    assert_blueprint_rules(&catalog, &interpreted)?;
    assert_filtered_rules(&archive, &entry)?;
    full_proof::from_entry(&archive, &entry, full_proof::NO_LANGUAGES, None)?
        .with_crafting_global_params(CatalogRecordSelection::All, None, |snapshot| {
            assert_eq!(
                snapshot.rows().len(),
                3,
                "master, repair and baseline owners remain physical rows"
            );
            walk::snapshot(&snapshot, &catalog)
        })??;
    Ok(())
}

fn assert_filtered_rules(archive: &Archive, entry: &DataForgeEntry) -> TestResult {
    let filter = Filter::new("*/bp_craft_two_tiers");
    let options = CatalogOptions {
        filter: Some(&filter),
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..CatalogOptions::new(std::time::UNIX_EPOCH)
    };
    let catalog: Value =
        serde_json::from_slice(build_catalog(archive, options.with_full_provenance())?.json())?;
    assert_eq!(
        blueprint::definitions(&catalog)?.len(),
        1,
        "The output filter selects one Blueprint and excludes the global owner paths"
    );
    let selected = blueprint::by_guid(&catalog, detail::TWO_TIERS)?;
    let record_index = selected
        .definition
        .pointer("/recordIndex/value")
        .and_then(Value::as_u64)
        .ok_or("original physical Blueprint index")?;
    let presentation = selected.presentation;
    assert_eq!(
        presentation
            .pointer("/defaultAvailable/value")
            .and_then(Value::as_bool),
        Some(true),
        "The first additional global provides default availability outside the filter"
    );
    assert_eq!(
        presentation
            .pointer("/dismantle/state")
            .and_then(Value::as_str),
        Some("absent"),
        "The second additional global excludes the produced item outside the filter"
    );
    assert_selected_sdk_rules(archive, entry, u32::try_from(record_index)?)
}

fn assert_selected_sdk_rules(
    archive: &Archive,
    entry: &DataForgeEntry,
    record_index: u32,
) -> TestResult {
    full_proof::from_entry(archive, entry, full_proof::NO_LANGUAGES, None)?.with_blueprints(
        CatalogRecordSelection::RecordIndex(record_index),
        None,
        |snapshot| {
            let [row] = snapshot.presentations() else {
                return Err("one selected physical Blueprint presentation".into());
            };
            assert_eq!(
                row.default_available().value(),
                Some(&true),
                "SDK default rule"
            );
            assert_eq!(
                row.dismantle().state(),
                p4kx_extraction::SourceValueState::Absent,
                "SDK item exclusion rule"
            );
            assert!(
                row.default_available().origin().source_set_complete()
                    && row.dismantle().origin().source_set_complete(),
                "Both outside-filter source sets close independently"
            );
            Ok::<_, Box<dyn std::error::Error>>(())
        },
    )??;
    Ok(())
}

fn assert_blueprint_rules(catalog: &Value, interpreted: &Crafting<'_>) -> TestResult {
    let definitions = blueprint::definitions(catalog)?;
    for index in 0..definitions.len() {
        let row = blueprint::by_index(catalog, index)?;
        let guid = row
            .definition
            .pointer("/guid/value")
            .and_then(Value::as_str)
            .ok_or("original blueprint GUID")?
            .parse()?;
        assert_eq!(
            row.presentation
                .pointer("/defaultAvailable/value")
                .and_then(Value::as_bool),
            Some(interpreted.global.default_blueprints.contains(&guid)),
            "same Build and all original owners determine availability"
        );
        let original = interpreted
            .blueprints
            .iter()
            .find(|source| source.guid == guid)
            .ok_or("interpreted original Blueprint")?;
        assert_eq!(
            row.presentation
                .pointer("/dismantle/value")
                .is_some_and(|value| !value.is_null()),
            interpreted.dismantle_return(original).is_some(),
            "unmodified original item exclusions determine dismantle"
        );
    }
    Ok(())
}

#[test]
fn additional_nonfinite_original_fields_remain_publishable_without_a_catalog_kind() -> TestResult {
    let database =
        fixture::declarations(false).record(fixture::owner("FiniteCoverage", Vec::new()));
    let (_folder, archive) = common::open(&database, DcbOrigin::Plain)?;
    let catalog = build_catalog(
        &archive,
        CatalogOptions::new(std::time::UNIX_EPOCH).with_full_provenance(),
    )?;
    let catalog: Value = serde_json::from_slice(catalog.json())?;
    let raw = common::at(
        &catalog,
        "/configurations/craftingGlobal/rows/0/additionalFields",
    )?;
    assert!(
        raw.to_string().contains("7ff80000000000a1"),
        "exact original NaN payload survives"
    );
    Ok(())
}

#[test]
fn absent_global_family_is_distinct_from_a_declared_empty_configuration() -> TestResult {
    for (database, available) in [
        (
            DataForgeBuilder::new().structure(TestStruct::new("Other")),
            false,
        ),
        (fixture::declarations(false), true),
    ] {
        let (_folder, archive) = common::open(&database, DcbOrigin::Plain)?;
        crafting::with_global_params(&archive, CatalogRecordSelection::All, None, |snapshot| {
            assert!(
                snapshot.rows().is_empty(),
                "no invented configuration owner"
            );
            assert_eq!(
                matches!(
                    snapshot.availability(),
                    catalog_queries::DatasetAvailability::Available
                ),
                available,
                "source declaration distinguishes absent from declared empty"
            );
            assert!(
                snapshot.coverage().unreadable_headers.is_empty(),
                "metadata census is readable"
            );
        })?;
    }
    Ok(())
}

#[test]
fn unsupported_array_storage_and_unreadable_additional_values_keep_coverage_and_original_data()
-> TestResult {
    let database = DataForgeBuilder::new()
        .structure(
            TestStruct::new("CraftingGlobalParams")
                .array("dismantleBlacklistResources", TestType::Bool)
                .field("futureFlag", TestType::Bool),
        )
        .record(
            TestRecord::new(
                fixture::OWNER,
                "CraftingGlobalParams",
                "CraftingGlobalParams.Unknown",
                "libs/foundry/records/crafting/globalparams/unknown.xml",
            )
            .value("futureFlag", TestValue::Raw(vec![2])),
        );
    let (_folder, archive) = common::open(&database, DcbOrigin::Plain)?;
    crafting::with_global_params(&archive, CatalogRecordSelection::All, None, |snapshot| {
        assert_eq!(
            snapshot.coverage().incomplete_fields,
            [0],
            "original field uncertainty survives"
        );
        let [row] = snapshot.rows() else {
            return Err("one original owner required".into());
        };
        assert_eq!(
            row.dismantle_blacklist_resources().state(),
            p4kx_extraction::SourceValueState::Unsupported,
            "empty wrong storage is not a Reference array"
        );
        let names: Vec<_> = row
            .additional_fields()
            .roots()
            .iter()
            .filter_map(|field| field.name().value().copied())
            .collect();
        assert_eq!(
            names,
            ["dismantleBlacklistResources", "futureFlag"],
            "rejected and unknown original fields remain in the arena"
        );
        Ok::<_, Box<dyn std::error::Error>>(())
    })??;
    Ok(())
}

fn global(name: &str, blueprint: u128, excluded: Vec<TestValue>) -> TestRecord {
    TestRecord::new(
        fixture::OWNER,
        "CraftingGlobalParams",
        format!("CraftingGlobalParams.{name}"),
        format!(
            "libs/foundry/records/crafting/globalparams/{}.xml",
            name.to_ascii_lowercase()
        ),
    )
    .value(
        "defaultBlueprintSelection",
        TestValue::Struct(
            TestInstance::new("DefaultBlueprintSelection_Whitelist").value(
                "blueprintRecords",
                TestValue::Array(vec![TestValue::Reference(blueprint)]),
            ),
        ),
    )
    .value(
        "dismantleBlacklistEntityClasses",
        TestValue::Array(excluded),
    )
}
