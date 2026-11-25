// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The verification of M3 over real Archives: the whole catalog against the checked-in schema,
//! plausible counts, no development leftovers, resolved texts and transport measurements; runs with
//! `--ignored` when `P4KX_DATA_P4K` names one; the test of the second Channel returns early
//! while `P4KX_DATA_P4K_B` is unset.

#[path = "real_archive_definition_identity.rs"]
mod definition_identity;

#[path = "real_archive_ammo_projection.rs"]
mod ammo_projection;

#[path = "real_archive_identity.rs"]
mod identity;
pub(crate) use identity::check_unique_guids;

use std::collections::HashSet;
use std::error::Error;
use std::io::Read;
use std::path::Path;
use std::time::SystemTime;

use p4kx_export::{
    CatalogOptions, CatalogSummary, Compression, EXCLUDED_TOKENS, Kind, catalog_schema,
    write_catalog,
};
use p4kx_extraction::{Archive, DataForgeEntry};
use p4kx_testkit::{real_archive_b_path, real_archive_path};
use serde_json::Value;

use crate::real_archive_catalog_links;

use crate::catalog_v3_consumer;
use crate::real_archive_texts::check_texts;

pub(crate) type TestResult = Result<(), Box<dyn Error>>;

const MINIMUM: [(Kind, usize); 45] = [
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
    (Kind::ContractNpcSpawns, 300),
    (Kind::ContractEntitySpawns, 30),
];

fn built(archive: &Archive) -> Result<(CatalogSummary, Value), Box<dyn Error>> {
    let (summary, frame) = write_catalog(
        archive,
        CatalogOptions::new(SystemTime::now()).with_full_provenance(),
        Compression::ZSTD,
        || Ok(Vec::new()),
    )?;

    let mut json = Vec::new();
    zstd::Decoder::new(frame.as_slice())?.read_to_end(&mut json)?;
    crate::real_archive::check_transport(
        &summary,
        u64::try_from(json.len())?,
        Some(u64::try_from(frame.len())?),
    );
    let catalog = serde_json::from_slice(&json)?;
    Ok((summary, catalog))
}

/// Returns the `guid` of every entry of the kind `kind` of `catalog`.
pub(crate) fn guids<'c>(catalog: &'c Value, kind: &str) -> Result<HashSet<&'c str>, String> {
    let kind = Kind::ALL
        .into_iter()
        .find(|candidate| candidate.name() == kind)
        .ok_or("known Kind")?;
    catalog_v3_consumer::rows(catalog, kind)
        .map_err(|error| error.to_string())?
        .iter()
        .map(|row| catalog_v3_consumer::identity(row, kind).map_err(|error| error.to_string()))
        .collect()
}

pub(crate) fn list(catalog: &Value, kind: Kind) -> Result<&Vec<Value>, String> {
    catalog_v3_consumer::rows(catalog, kind).map_err(|error| error.to_string())
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
        let guids: Vec<&str> = entities
            .iter()
            .map(|entry| catalog_v3_consumer::identity(entry, kind))
            .collect::<Result<_, _>>()?;
        assert_eq!(guids.len(), entities.len(), "{kind} all have a {own}");
        if kind == Kind::Loadouts {
            check_loadout_order(entities, &guids)?;
            continue;
        }
        if matches!(
            kind,
            Kind::Items
                | Kind::BlueprintCategories
                | Kind::BlueprintCategoryLists
                | Kind::GameplayProperties
                | Kind::CargoManifests
                | Kind::MissionTypes
                | Kind::MissionLocalities
                | Kind::DismantleProcesses
                | Kind::ReputationContexts
                | Kind::FactionReputations
                | Kind::ReputationScopes
                | Kind::ReputationStandings
                | Kind::QualityDistributions
                | Kind::QualityQuantizations
        ) {
            assert!(
                guids.is_sorted(),
                "{kind} orders nominal GUIDs while retaining physical alternatives"
            );
            definition_identity::check(&catalog, kind)?;
        } else {
            assert!(
                guids.is_sorted_by(|a, b| a < b),
                "{kind} strictly ordered by {own}, none twice"
            );
        }
    }
    check_unique_guids(&catalog)?;
    if !list(&catalog, Kind::Items)?.is_empty()
        || !list(&catalog, Kind::BlueprintCategoryLists)?.is_empty()
        || !list(&catalog, Kind::GameplayProperties)?.is_empty()
        || !list(&catalog, Kind::ExplosionDefinitions)?.is_empty()
        || !list(&catalog, Kind::AmmoDefinitions)?.is_empty()
        || !list(&catalog, Kind::MissionLocalities)?.is_empty()
    {
        let entry = DataForgeEntry::read(&archive)?;
        let forge = entry.open()?;
        definition_identity::check_original_items(&catalog, &forge)?;
        definition_identity::check_original_lists(&catalog, &forge)?;
        definition_identity::check_original_properties(&catalog, &forge)?;
        definition_identity::check_original_explosions(&catalog, &forge)?;
        definition_identity::check_original_ammo(&catalog, &forge)?;
        definition_identity::check_original_localities(&catalog, &forge)?;
    }
    check_leftovers(&catalog)?;
    check_texts(&archive, &catalog, baselined)?;
    Ok(catalog)
}

fn check_loadout_order(rows: &[Value], guids: &[&str]) -> TestResult {
    assert!(guids.is_sorted(), "Loadouts are ordered by nominal GUID");
    let mut indices = HashSet::with_capacity(rows.len());
    for row in rows {
        let index = catalog_v3_consumer::field(row, Kind::Loadouts, "sourceRecordIndex")?
            .as_u64()
            .ok_or("original physical Loadout Record index")?;
        assert!(indices.insert(index), "a physical Loadout occurs only once");
    }
    Ok(())
}

fn check_leftovers(catalog: &Value) -> TestResult {
    for kind in Kind::ALL {
        for entity in list(catalog, kind)? {
            for field in ["recordPath", "className"] {
                if matches!(
                    kind,
                    Kind::BlueprintCategories | Kind::BlueprintCategoryLists
                ) && field == "className"
                {
                    let carrier = entity.get(field).ok_or("required category class carrier")?;
                    if matches!(
                        carrier.get("state").and_then(Value::as_str),
                        Some("absent" | "null")
                    ) {
                        catalog_v3_consumer::source_value(carrier)?;
                        continue;
                    }
                }
                let text = if matches!(
                    kind,
                    Kind::BlueprintCategories
                        | Kind::BlueprintCategoryLists
                        | Kind::Manufacturers
                        | Kind::Commodities
                        | Kind::ResourceTypes
                        | Kind::QualityDistributions
                        | Kind::QualityQuantizations
                        | Kind::MissionLocalities
                ) {
                    catalog_v3_consumer::field(entity, kind, field)?
                        .as_str()
                        .ok_or("canonical identity field")?
                } else {
                    entity
                        .get(field)
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                };
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

#[expect(
    clippy::print_stdout,
    reason = "the measurements are the result of the verification"
)]
pub(crate) fn print_line(line: &str) {
    println!("{line}");
}

fn report(path: &Path, summary: &CatalogSummary) {
    print_line(&format!("catalog input: {}", path.display()));
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
fn the_catalog_of_a_real_archive_is_complete_and_plausible() -> TestResult {
    let path = real_archive_path()?;
    real_archive_catalog_links::check(&verify_catalog(&path, true)?)
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn the_catalog_of_a_second_real_archive_is_complete_and_plausible() -> TestResult {
    let Some(path) = real_archive_b_path() else {
        return Ok(());
    };
    real_archive_catalog_links::check(&verify_catalog(&path, false)?)
}
