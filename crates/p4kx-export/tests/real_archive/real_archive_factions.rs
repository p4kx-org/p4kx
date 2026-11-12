// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;
use std::time::SystemTime;

use p4kx_export::{CatalogOptions, CatalogPublicationMode, Kind, build_catalog_from};
use p4kx_extraction::{
    Archive, ArchiveWatch, DataForge, DataForgeEntry, Guid, TreeBuilder, Value as Original,
    ValueError,
};
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde_json::Value;

use crate::catalog_v3_consumer as consumer;
use crate::real_archive_verification::print_line;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

const FLAG_FIELDS: [&str; 4] = [
    "ableToArrest",
    "policesLawfulTrespass",
    "policesCriminality",
    "noLegalRights",
];

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn faction_definitions_and_incoming_scenario_links_match_the_original_records() -> TestResult {
    let path = real_archive_path().map_err(|_| "configured Archive path")?;
    let key = real_archive_key().map_err(|_| "configured Archive key")?;
    let archive = Archive::open_with_key(path, Some(key)).map_err(|_| "open configured Archive")?;
    let watch = ArchiveWatch::new(&archive);
    let entry = DataForgeEntry::read(&archive).map_err(|_| "read original DataForge")?;
    let options = CatalogOptions {
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..CatalogOptions::new(SystemTime::now())
    };
    let built =
        build_catalog_from(&archive, &entry, &options).map_err(|_| "build Faction catalog")?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    crate::real_archive_shop_inventory_v3::verify_build(&archive, &catalog)?;
    let forge = entry.open().map_err(|_| "open original DataForge")?;
    let factions = verify_factions(&forge, &catalog)?;
    let incoming = verify_scenarios(&forge, &catalog, &factions)?;
    watch
        .check()
        .map_err(|_| "Archive changed during Faction verification")?;
    print_line(&format!(
        "factions: {} physical headers and {incoming} incoming ScenarioProgress occurrences checked",
        factions.len()
    ));
    Ok(())
}

fn verify_factions(forge: &DataForge<'_>, catalog: &Value) -> TestResult<BTreeSet<Guid>> {
    let rows = consumer::rows(catalog, Kind::Factions)?;
    let expected: BTreeSet<_> = forge
        .records()
        .filter(|record| matches!(record.type_name(), Ok("Faction")))
        .map(|record| record.index())
        .collect();
    assert!(!expected.is_empty(), "configured build has Faction sources");
    let mut recorded = BTreeSet::new();
    let mut owners = BTreeSet::new();
    let mut order = Vec::new();
    let mut trees = TreeBuilder::new();
    for row in rows {
        let index = u32::try_from(
            consumer::field(row, Kind::Factions, "recordIndex")?
                .as_u64()
                .ok_or("Faction original Record index")?,
        )?;
        let record = forge.record(index).ok_or("original Faction Record")?;
        let guid = consumer::identity(row, Kind::Factions)?;
        assert!(recorded.insert(index), "one row per physical Faction");
        assert!(
            matches!(record.type_name(), Ok("Faction")),
            "Faction source type"
        );
        assert!(guid == record.guid().to_string(), "Faction original GUID");
        assert!(
            consumer::field(row, Kind::Factions, "recordPath")?.as_str()
                == Some(record.record_path()?),
            "Faction original path"
        );
        owners.insert(record.guid());
        order.push((guid, index));
        let tree = trees
            .build(&record)
            .map_err(|_| "build original Faction body")?;
        for field in FLAG_FIELDS
            .into_iter()
            .chain(["defaultReaction", "factionType"])
        {
            check_scalar(row, field, tree.field(field))?;
        }
    }
    assert_eq!(
        recorded, expected,
        "the unfiltered table accounts for the exact physical family"
    );
    assert!(
        order.is_sorted(),
        "GUID and physical ordinal define Faction order"
    );
    Ok(owners)
}

fn check_scalar(row: &Value, field: &str, actual: Option<&Original<'_>>) -> TestResult {
    let carrier = row.get(field).ok_or("required Faction carrier")?;
    let payload = consumer::source_value(carrier)?;
    let expected = match actual {
        None => "absent",
        Some(Original::Null) => "null",
        Some(Original::Bool(value)) => {
            assert!(
                payload.and_then(Value::as_bool) == Some(*value),
                "original Faction flag"
            );
            "present"
        }
        Some(Original::Enum(value)) => {
            assert!(
                payload.and_then(Value::as_str) == Some(*value),
                "original Faction enum label"
            );
            "present"
        }
        Some(Original::Error(_)) => "unreadable",
        Some(_) => "invalid",
    };
    assert_eq!(
        carrier.get("state").and_then(Value::as_str),
        Some(expected),
        "{field} state"
    );
    Ok(())
}

fn verify_scenarios(
    forge: &DataForge<'_>,
    catalog: &Value,
    factions: &BTreeSet<Guid>,
) -> TestResult<usize> {
    let mut count = 0;
    let mut trees = TreeBuilder::new();
    for row in consumer::rows(catalog, Kind::ContractScenarios)? {
        let guid: Guid = consumer::identity(row, Kind::ContractScenarios)?.parse()?;
        let record = forge
            .record_by_guid(guid)
            .ok_or("original ScenarioProgress Record")?;
        let tree = trees
            .build(&record)
            .map_err(|_| "build original ScenarioProgress body")?;
        let originals = match tree.field("factionRewardTiers") {
            Some(Original::Array(values)) => values.as_ref(),
            _ => &[],
        };
        let tiers = row
            .get("factionRewardTiers")
            .and_then(Value::as_array)
            .ok_or("projected faction reward tiers")?;
        let original_tiers = originals.iter().filter_map(|item| match item {
            Original::Struct(body) => Some(body),
            _ => None,
        });
        assert_eq!(
            tiers.len(),
            original_tiers.clone().count(),
            "scenario tier order and cardinality"
        );
        for (tier, original) in tiers.iter().zip(original_tiers) {
            let expected = match original.field("faction") {
                Some(Original::Reference(target)) if target.location().is_none() => {
                    assert!(
                        factions.contains(&target.guid()),
                        "readable incoming Faction resolves"
                    );
                    Some(target.guid())
                }
                Some(Original::Error(ValueError::DanglingReference { guid })) => Some(*guid),
                _ => None,
            };
            let exported = tier
                .get("factionGuid")
                .ok_or("required incoming Faction GUID")?
                .as_str()
                .map(str::parse::<Guid>)
                .transpose()?;
            assert!(exported == expected, "original incoming scenario link");
            count += 1;
        }
    }
    Ok(count)
}
