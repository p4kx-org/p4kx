// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The verification of M3 over real Archives: the whole catalog against the checked-in schema,
//! plausible counts, no development leftovers, resolved texts and the upload limit; runs with
//! `--ignored` when `P4KX_DATA_P4K` names one; the test of the second Channel returns early
//! while `P4KX_DATA_P4K_B` is unset.

use std::collections::HashSet;
use std::error::Error;
use std::io::Read;
use std::path::Path;
use std::time::SystemTime;

use p4kx_export::{
    CatalogOptions, CatalogSummary, Compression, DECOMPRESSED_LIMIT, EXCLUDED_TOKENS, Kind,
    UPLOAD_LIMIT, catalog_schema, write_catalog,
};
use p4kx_extraction::Archive;
use p4kx_testkit::{real_archive_b_path, real_archive_path};
use serde_json::Value;

use crate::real_archive_broker_entries::check_broker_links;
use crate::real_archive_contract_templates::check_template_links;
use crate::real_archive_spawns::check_spawn_links;

use crate::real_archive_texts::check_texts;

pub(crate) type TestResult = Result<(), Box<dyn Error>>;

const MINIMUM: [(Kind, usize); 43] = [
    (Kind::Manufacturers, 500),
    (Kind::Items, 10_000),
    (Kind::Ships, 200),
    (Kind::Commodities, 100),
    (Kind::Blueprints, 1_000),
    (Kind::ResourceGroups, 15),
    (Kind::BlueprintCategories, 10),
    (Kind::GameplayProperties, 1),
    (Kind::QualityDistributions, 2),
    (Kind::QualityQuantizations, 60),
    (Kind::BlueprintPools, 100),
    (Kind::DismantleProcesses, 1),
    (Kind::Paints, 900),
    (Kind::Loadouts, 1_000),
    (Kind::MineableElements, 20),
    (Kind::RefiningProcesses, 9),
    (Kind::MineableCompositions, 200),
    (Kind::MiningGlobalParams, 5),
    (Kind::HarvestableProviderPresets, 40),
    (Kind::SubHarvestableMultiConfigs, 100),
    (Kind::HarvestableLocations, 300),
    (Kind::Shops, 1_000),
    (Kind::FactionReputations, 30),
    (Kind::ReputationScopes, 30),
    (Kind::ReputationStandings, 300),
    (Kind::ReputationContexts, 15),
    (Kind::MissionTypes, 30),
    (Kind::MissionLocalities, 15),
    (Kind::ContractScenarios, 2),
    (Kind::CargoManifests, 80),
    (Kind::LootTables, 50),
    (Kind::LootArchetypes, 30),
    (Kind::LootPoolFilters, 10),
    (Kind::LootContainers, 1_000),
    (Kind::MineableEntities, 200),
    (Kind::LegacyLootTables, 150),
    (Kind::LegacyLootArchetypes, 200),
    (Kind::Crafters, 2),
    (Kind::MissionLocationTemplates, 2_000),
    (Kind::ContractShipSpawns, 400),
    (Kind::ContractLocationQueries, 1_500),
    (Kind::ContractTemplates, 400),
    (Kind::MissionBrokerEntries, 2_000),
];

fn built(archive: &Archive) -> Result<(CatalogSummary, Value), Box<dyn Error>> {
    let (summary, frame) = write_catalog(
        archive,
        &CatalogOptions::new(SystemTime::now()),
        Compression::ZSTD,
        || Ok(Vec::new()),
    )?;
    assert_eq!(
        summary.compressed_size(),
        Some(u64::try_from(frame.len())?),
        "the compressed size is the frame size"
    );
    let mut json = Vec::new();
    zstd::Decoder::new(frame.as_slice())?.read_to_end(&mut json)?;
    assert_eq!(
        summary.size(),
        u64::try_from(json.len())?,
        "the decompressed size"
    );
    let catalog = serde_json::from_slice(&json)?;
    Ok((summary, catalog))
}

/// Returns the `guid` of every entry of the kind `kind` of `catalog`.
pub(crate) fn guids<'c>(catalog: &'c Value, kind: &str) -> Result<HashSet<&'c str>, String> {
    Ok(catalog[kind]
        .as_array()
        .ok_or_else(|| format!("{kind} is a list"))?
        .iter()
        .filter_map(|entry| entry["guid"].as_str())
        .collect())
}

pub(crate) fn list(catalog: &Value, kind: Kind) -> Result<&Vec<Value>, String> {
    catalog[kind.name()]
        .as_array()
        .ok_or_else(|| format!("the {kind} array"))
}

/// Returns the leftover word of `text`; `template` is none for a contract template, which live
/// Records such as `handyman_template` carry.
fn has_excluded_token(kind: Kind, text: &str) -> Option<&'static str> {
    text.split(|c: char| !c.is_ascii_alphanumeric())
        .find_map(|word| {
            EXCLUDED_TOKENS.into_iter().find(|token| {
                word.eq_ignore_ascii_case(token)
                    && !(kind == Kind::ContractTemplates && *token == "template")
            })
        })
}

#[expect(
    clippy::too_many_lines,
    reason = "the checks of one Archive follow each other"
)]
fn verify_catalog(path: &Path, baselined: bool) -> Result<Value, Box<dyn Error>> {
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::real_archive_key()?))?;
    let (summary, catalog) = built(&archive)?;
    let summary = &summary;
    let validator = jsonschema::draft202012::new(&catalog_schema())?;
    assert!(
        validator.is_valid(&catalog),
        "valid against the catalog schema"
    );
    report(path, summary);
    assert!(
        crate::real_archive::is_complete_but_known_defects(summary),
        "every selected Record is readable and every warning a known defect: {:?} {:?}",
        summary.failures().first(),
        summary.warnings()
    );

    for (kind, minimum) in MINIMUM {
        let counts = summary.kind(kind).ok_or("the kind")?;
        assert!(
            counts.count >= minimum,
            "plausible {kind} count: {counts:?}"
        );
        assert!(counts.complete, "{kind} complete without a Filter");
        let entities = list(&catalog, kind)?;
        assert_eq!(entities.len(), counts.count, "{kind} written");
        if matches!(kind, Kind::HarvestableLocations | Kind::LootContainers) {
            continue;
        }
        let own = match kind {
            Kind::Paints => "itemGuid",
            Kind::Loadouts => "entityGuid",
            Kind::Shops => "key",
            _ => "guid",
        };
        let guids: Vec<&str> = entities.iter().filter_map(|e| e[own].as_str()).collect();
        assert_eq!(guids.len(), entities.len(), "{kind} all have a {own}");
        assert!(
            guids.is_sorted_by(|a, b| a < b),
            "{kind} strictly ordered by {own}, none twice"
        );
    }
    check_unique_guids(&catalog)?;
    assert_eq!(
        summary.size_warning(),
        None,
        "the file is below 90 % of the upload limit of {UPLOAD_LIMIT} bytes"
    );
    assert_eq!(
        summary.decompressed_warning(),
        None,
        "the catalog is below 90 % of the decompressed limit of {DECOMPRESSED_LIMIT} bytes"
    );
    assert!(
        summary.upload_size() <= UPLOAD_LIMIT,
        "within the upload limit"
    );
    assert!(
        summary.size() <= DECOMPRESSED_LIMIT,
        "within the decompressed limit"
    );
    check_leftovers(&catalog)?;
    check_texts(&archive, &catalog, baselined)?;
    Ok(catalog)
}

/// Checks that no GUID occurs twice; a paint carries no GUID of its own, only the `itemGuid` of
/// its item, and a shop is identified by its `key` (its `entityGuid` is shared by the placements
/// of one shop module). A contract and its sub contracts share the `guid`, so the identity of a contract is
/// the pair of `guid` and `subContractGuid`, which is unique and strictly ordered within the kind.
fn check_unique_guids(catalog: &Value) -> TestResult {
    let mut seen = HashSet::new();
    let identities: Vec<(&str, &str)> = list(catalog, Kind::Contracts)?
        .iter()
        .map(|entry| {
            (
                entry["guid"].as_str().unwrap_or_default(),
                entry["subContractGuid"].as_str().unwrap_or_default(),
            )
        })
        .collect();
    assert!(
        identities.is_sorted_by(|a, b| a < b),
        "contracts strictly ordered by guid and subContractGuid, none twice"
    );
    let kinds = Kind::ALL
        .into_iter()
        .filter(|kind| !kind.is_derived() && !matches!(kind, Kind::Contracts | Kind::Crafters));
    for kind in kinds {
        for entity in list(catalog, kind)? {
            let guid = entity["guid"].as_str().ok_or("a GUID")?;
            assert!(seen.insert(guid), "{guid} occurs twice, also in {kind}");
        }
    }
    Ok(())
}

fn check_leftovers(catalog: &Value) -> TestResult {
    for kind in Kind::ALL {
        for entity in list(catalog, kind)? {
            for field in ["recordPath", "className"] {
                let text = entity[field].as_str().unwrap_or_default();
                assert_eq!(
                    has_excluded_token(kind, text),
                    None,
                    "development leftover in {kind}: {text}"
                );
            }
        }
    }
    Ok(())
}

fn check_links(catalog: &Value) -> TestResult {
    check_spawn_links(catalog)?;
    check_template_links(catalog)?;
    check_broker_links(catalog)?;
    let guids = |kind: Kind| -> Result<HashSet<&str>, String> {
        Ok(list(catalog, kind)?
            .iter()
            .filter_map(|e| e["guid"].as_str())
            .collect())
    };
    let manufacturers = guids(Kind::Manufacturers)?;
    let producible: HashSet<&str> = guids(Kind::Items)?
        .union(&guids(Kind::Ships)?)
        .copied()
        .collect();
    let items = guids(Kind::Items)?;
    let groups = guids(Kind::ResourceGroups)?;
    let categories = guids(Kind::BlueprintCategories)?;
    let resources: HashSet<&str> = list(catalog, Kind::Commodities)?
        .iter()
        .filter_map(|e| e["subtypeGuid"].as_str())
        .collect();
    for kind in [Kind::Items, Kind::Ships] {
        for entity in list(catalog, kind)? {
            if let Some(guid) = entity["manufacturerGuid"].as_str() {
                assert!(manufacturers.contains(guid), "{kind} manufacturer {guid}");
            }
        }
    }
    for commodity in list(catalog, Kind::Commodities)? {
        if let Some(guid) = commodity["typeGuid"].as_str() {
            assert!(groups.contains(guid), "commodity type {guid}");
        }
    }
    let mut nested = 0_usize;
    for group in list(catalog, Kind::ResourceGroups)? {
        if let Some(guid) = group["parentGuid"].as_str() {
            assert!(groups.contains(guid), "parent group {guid}");
            nested += 1;
        }
    }
    assert!(nested > 0, "at least one nested resource group");
    for blueprint in list(catalog, Kind::Blueprints)? {
        if let Some(guid) = blueprint["categoryGuid"].as_str() {
            assert!(categories.contains(guid), "blueprint category {guid}");
        }
        if let Some(guid) = blueprint["producedItemGuid"].as_str() {
            assert!(producible.contains(guid), "produced entity {guid}");
        }
        for ingredient in blueprint["ingredients"].as_array().into_iter().flatten() {
            if let Some(guid) = ingredient["resourceGuid"].as_str() {
                assert!(resources.contains(guid), "resource {guid}");
            }
            if let Some(guid) = ingredient["itemGuid"].as_str() {
                assert!(items.contains(guid), "ingredient item {guid}");
            }
        }
    }
    Ok(())
}

#[expect(
    clippy::print_stdout,
    reason = "the measurements are the result of the verification"
)]
pub(crate) fn print_line(line: &str) {
    println!("{line}");
}

fn report(path: &Path, summary: &CatalogSummary) {
    print_line(&format!(
        "{}: catalog {} bytes decompressed (limit {DECOMPRESSED_LIMIT}), {} bytes compressed (upload limit {UPLOAD_LIMIT})",
        path.display(),
        summary.size(),
        summary.upload_size()
    ));
    for kind in summary.kinds() {
        print_line(&format!("{kind:?}"));
    }
    print_line(&format!("{:?}", summary.item_classes()));
    for text in summary.texts() {
        print_line(&format!("{text:?}"));
    }
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn the_catalog_of_a_real_archive_is_complete_plausible_and_within_the_limit() -> TestResult {
    let path = real_archive_path()?;
    check_links(&verify_catalog(&path, true)?)
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn the_catalog_of_a_second_real_archive_is_complete_plausible_and_within_the_limit() -> TestResult {
    let Some(path) = real_archive_b_path() else {
        return Ok(());
    };
    check_links(&verify_catalog(&path, false)?)
}
