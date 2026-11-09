// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The mineable elements and refining processes of the Basetool catalog of a real Archive; runs
//! with `--ignored` when `P4KX_DATA_P4K` names one.

use std::collections::HashSet;
use std::error::Error;
use std::path::Path;
use std::time::SystemTime;

use p4kx_export::{CatalogOptions, Kind, build_catalog};
use p4kx_extraction::{Archive, DataForgeEntry, Guid};
use p4kx_testkit::real_archive_path;
use serde_json::Value;

use crate::catalog_v3_consumer;

type TestResult = Result<(), Box<dyn Error>>;

#[expect(
    clippy::print_stdout,
    reason = "the measurements are the result of the verification"
)]
fn print_line(line: &str) {
    println!("{line}");
}

fn list<'c>(catalog: &'c Value, kind: &str) -> Result<&'c Vec<Value>, String> {
    catalog_v3_consumer::named_rows(catalog, kind).map_err(|error| error.to_string())
}

struct OnDisk {
    names: Vec<String>,
    unresolved: usize,
}

fn on_disk(archive: &Archive, elements: &[Value]) -> Result<OnDisk, Box<dyn Error>> {
    let entry = DataForgeEntry::read(archive)?;
    let forge = entry.open()?;
    let rule = Kind::MineableElements.rule().ok_or("a rule")?;
    let mut names = Vec::new();
    for record in forge.records() {
        if rule.matches_path(record.record_path()?) && record.type_name()? == rule.type_name {
            names.push(record.name()?.to_owned());
        }
    }
    let mut unresolved = 0;
    for element in elements {
        let linked = element["resourceTypeGuid"]
            .as_str()
            .and_then(|text| text.parse::<Guid>().ok())
            .and_then(|guid| forge.record_by_guid(guid))
            .map(|record| record.type_name().is_ok_and(|name| name == "ResourceType"));
        if linked != Some(true) {
            unresolved += 1;
        }
    }
    Ok(OnDisk { names, unresolved })
}

#[expect(
    clippy::too_many_lines,
    reason = "the measurements print before the assertions"
)]
fn verify(path: &Path) -> TestResult {
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::real_archive_key()?))?;
    let built = build_catalog(&archive, &CatalogOptions::new(SystemTime::now()))?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let elements = list(&catalog, "mineableElements")?;
    let processes = list(&catalog, "refiningProcesses")?;
    let disk = on_disk(&archive, elements)?;
    let written: HashSet<&str> = elements
        .iter()
        .filter_map(|element| element["className"].as_str())
        .collect();
    let skipped: Vec<&str> = disk
        .names
        .iter()
        .map(|name| name.split_once('.').map_or(name.as_str(), |(_, rest)| rest))
        .filter(|class| !written.contains(class))
        .collect();
    let resources: HashSet<&str> = elements
        .iter()
        .filter_map(|element| element["resourceTypeGuid"].as_str())
        .collect();
    let mut subtypes = HashSet::new();
    for choice in catalog_v3_consumer::presentations(&catalog)? {
        if let Some(guid) = catalog_v3_consumer::nullable_guid(
            choice
                .get("subtypeGuid")
                .ok_or("Commodity subtype carrier")?,
        )? {
            subtypes.insert(guid);
        }
    }
    let with_commodity = resources
        .iter()
        .filter(|guid| subtypes.contains(**guid))
        .count();
    let instabilities: Vec<f64> = elements
        .iter()
        .filter_map(|element| element["instability"].as_f64())
        .collect();
    let pairs: HashSet<(&str, &str)> = processes
        .iter()
        .filter_map(|process| {
            Some((
                process["refiningSpeed"].as_str()?,
                process["refiningQuality"].as_str()?,
            ))
        })
        .collect();
    let german = processes
        .iter()
        .filter(|process| {
            process
                .pointer("/processName/de")
                .is_some_and(Value::is_string)
        })
        .count();
    let english = processes
        .iter()
        .filter(|process| {
            process
                .pointer("/processName/en")
                .is_some_and(Value::is_string)
        })
        .count();
    let skipped_count = built
        .summary()
        .kind(Kind::MineableElements)
        .map_or(0, |kind| kind.skipped);
    print_line(&format!(
        "{}: {} mineable elements written, {} Records under the folder, {skipped_count} skipped {skipped:?}, {} distinct ResourceTypes ({with_commodity} with a commodity), {} unresolved, {} with an instability; {} refining processes, {} distinct speed/quality pairs, {english} with an English and {german} with a German name",
        path.display(),
        elements.len(),
        disk.names.len(),
        resources.len(),
        disk.unresolved,
        instabilities.len(),
        processes.len(),
        pairs.len(),
    ));
    for element in elements.iter().take(3) {
        print_line(&format!("element sample: {element}"));
    }
    for process in processes.iter().take(3) {
        print_line(&format!("process sample: {process}"));
    }
    assert_eq!(
        elements.len() + skipped_count,
        disk.names.len(),
        "written plus skipped is every Record under the folder"
    );
    assert_eq!(
        skipped.len(),
        skipped_count,
        "the skipped Records are those missing"
    );
    assert!(
        skipped
            .iter()
            .all(|class| class.to_ascii_lowercase().contains("template")),
        "only templates are left out: {skipped:?}"
    );
    assert!(elements.len() >= 20, "some mineable elements");
    assert_eq!(
        disk.unresolved, 0,
        "every element links to a ResourceType Record"
    );
    assert_eq!(instabilities.len(), elements.len(), "every element has one");
    assert!(
        instabilities
            .iter()
            .all(|value| (0.0..=1000.0).contains(value)),
        "instability from 0 to 1000"
    );
    assert_eq!(pairs.len(), processes.len(), "one process per pair");
    assert!(processes.len() >= 9, "the nine refining processes");
    Ok(())
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn the_mineable_elements_and_refining_processes_of_a_real_archive_are_plausible() -> TestResult {
    verify(&real_archive_path()?)
}

fn named<'c>(catalog: &'c Value, kind: &str, class: &str) -> Result<&'c Value, Box<dyn Error>> {
    list(catalog, kind)?
        .iter()
        .find(|entry| entry["className"] == class)
        .ok_or_else(|| format!("{kind} has no {class}").into())
}

fn guids<'c>(catalog: &'c Value, kind: &str) -> Result<HashSet<&'c str>, Box<dyn Error>> {
    Ok(list(catalog, kind)?
        .iter()
        .filter_map(|entry| entry["guid"].as_str())
        .collect())
}

fn at_path<'c>(catalog: &'c Value, kind: &str, name: &str) -> Result<&'c Value, Box<dyn Error>> {
    let suffix = format!("/{name}");
    list(catalog, kind)?
        .iter()
        .find(|entry| {
            entry["recordPath"]
                .as_str()
                .is_some_and(|path| path.to_ascii_lowercase().ends_with(&suffix))
        })
        .ok_or_else(|| format!("{kind} has no {name}").into())
}

fn real_catalog() -> Result<Value, Box<dyn Error>> {
    let archive = Archive::open_with_key(
        real_archive_path()?,
        Some(p4kx_testkit::real_archive_key()?),
    )?;
    let built = build_catalog(&archive, &CatalogOptions::new(SystemTime::now()))?;
    Ok(serde_json::from_slice(built.json())?)
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn the_scan_signature_of_an_asteroid_and_a_surface_rock_of_a_real_archive_is_the_named_slot()
-> TestResult {
    let catalog = real_catalog()?;
    for (name, signature) in [
        ("mineablerock_asteroidlegendary_quantainium", 3170.0),
        ("mineablerock_asteroidcommon_iron", 4270.0),
        ("mineablerock_surfacerare_gold", 3585.0),
    ] {
        let rock = at_path(&catalog, "mineableEntities", name)?;
        print_line(&format!("mineable entity sample: {rock}"));
        assert_eq!(rock["scanSignature"], signature, "{name}");
        assert_eq!(rock["signatures"].as_array().map(Vec::len), Some(8));
        assert_eq!(rock["signatures"][4], signature, "{name}: slot 4 on LIVE");
        assert!(rock["radarContactTypeGuid"].is_string(), "{name}");
    }
    Ok(())
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn the_mineable_entities_of_a_real_archive_are_plausible_and_linked() -> TestResult {
    let catalog = real_catalog()?;
    let compositions = guids(&catalog, "mineableCompositions")?;
    let entities = list(&catalog, "mineableEntities")?;
    let signatures: Vec<f64> = entities
        .iter()
        .filter_map(|entity| entity["scanSignature"].as_f64())
        .collect();
    print_line(&format!(
        "{} mineable entities, {} with a scan signature",
        entities.len(),
        signatures.len()
    ));
    assert!(
        entities.len() >= 200,
        "mineable entities: {}",
        entities.len()
    );
    assert!(
        signatures.len() >= 200,
        "with a signature: {}",
        signatures.len()
    );
    assert!(
        signatures
            .iter()
            .all(|value| (3000.0..=4900.0).contains(value)),
        "rock signatures from 3000 to 4900"
    );
    for entity in entities {
        if let Some(composition) = entity["mineableCompositionGuid"].as_str() {
            assert!(compositions.contains(composition), "{composition}");
        }
    }
    let known = guids(&catalog, "mineableEntities")?;
    let mut linked = 0_usize;
    for harvestable in list(&catalog, "harvestableProviderPresets")?
        .iter()
        .flat_map(|preset| preset["groups"].as_array().into_iter().flatten())
        .flat_map(|group| group["harvestables"].as_array().into_iter().flatten())
    {
        if harvestable["harvestable"]["mineableCompositionGuid"].is_string() {
            let entity = harvestable["harvestable"]["entityGuid"]
                .as_str()
                .ok_or("an entity")?;
            assert!(known.contains(entity), "{entity} is a mineable entity");
            linked += 1;
        }
    }
    assert!(
        linked > 100,
        "harvestables reach mineable entities: {linked}"
    );
    Ok(())
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
#[expect(
    clippy::too_many_lines,
    reason = "the checks of one Archive follow each other"
)]
fn an_asteroid_composition_and_a_location_of_a_real_archive_are_linked() -> TestResult {
    let path = real_archive_path()?;
    let archive = Archive::open_with_key(&path, Some(p4kx_testkit::real_archive_key()?))?;
    let built = build_catalog(&archive, &CatalogOptions::new(SystemTime::now()))?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let elements = guids(&catalog, "mineableElements")?;
    let compositions = guids(&catalog, "mineableCompositions")?;
    let objects = guids(&catalog, "starmapObjects")?;

    let iron = named(&catalog, "mineableCompositions", "Asteroid_CType_Iron")?;
    let parts = iron["parts"].as_array().ok_or("the parts")?;
    print_line(&format!("composition sample: {iron}"));
    assert!(parts.len() >= 3, "an iron asteroid has several parts");
    for part in parts {
        let element = part["mineableElementGuid"].as_str().ok_or("an element")?;
        assert!(
            elements.contains(element),
            "{element} is a mineable element"
        );
        let (min, max) = (
            part["minPercentage"].as_f64().ok_or("a minimum")?,
            part["maxPercentage"].as_f64().ok_or("a maximum")?,
        );
        assert!(min <= max, "the range of a part: {min} to {max}");
    }

    let presets = list(&catalog, "harvestableProviderPresets")?;
    let preset = named(&catalog, "harvestableProviderPresets", "HPP_Lagrange_A")?;
    let preset_guid = preset["guid"].as_str().ok_or("a GUID")?;
    let place = list(&catalog, "harvestableLocations")?
        .iter()
        .find(|location| {
            location["presetGuid"] == preset_guid
                && location["starmapObjectGuid"].as_str().is_some_and(|guid| {
                    list(&catalog, "starmapObjects").is_ok_and(|all| {
                        all.iter().any(|object| {
                            object["guid"] == guid && object["className"] == "Stanton1_L1_01"
                        })
                    })
                })
        })
        .ok_or("HPP_Lagrange_A is located at Stanton1_L1_01")?;
    print_line(&format!("location sample: {place}"));
    let container = place["container"].as_str().ok_or("a container")?;
    assert!(
        container.to_ascii_lowercase().ends_with(".socpak"),
        "{container}"
    );

    let mut linked = 0_usize;
    for harvestable in presets
        .iter()
        .flat_map(|preset| preset["groups"].as_array().into_iter().flatten())
        .flat_map(|group| group["harvestables"].as_array().into_iter().flatten())
    {
        if let Some(composition) = harvestable["harvestable"]["mineableCompositionGuid"].as_str() {
            assert!(
                compositions.contains(composition),
                "{composition} is a kept composition"
            );
            linked += 1;
        }
    }
    assert!(
        linked > 100,
        "mineable harvestables reach compositions: {linked}"
    );
    for location in list(&catalog, "harvestableLocations")? {
        let preset = location["presetGuid"].as_str().ok_or("a preset")?;
        assert!(
            presets.iter().any(|entry| entry["guid"] == preset),
            "{preset} is a kept preset"
        );
        if let Some(object) = location["starmapObjectGuid"].as_str() {
            assert!(objects.contains(object), "{object} is a starmap object");
        }
    }
    Ok(())
}
