// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::time::UNIX_EPOCH;

use p4kx_export::{CatalogOptions, CatalogPublicationMode, Filter, build_catalog_from};
use p4kx_extraction::{Archive, ArchiveWatch, DataForgeEntry, RecordTree, TreeBuilder, Value};
use p4kx_testkit::{real_archive_key, real_archive_pair};
use serde_json::Value as Json;

use crate::real_archive_auto_spawns::original::Locations;
use crate::real_archive_auto_spawns::provenance::Origins;
use crate::real_archive_shop_inventory_v3::verify_build;
use crate::real_archive_verification::print_line;

const SOURCE_TYPE: &str = "ProceduralPlanetAudioData";
const CONTROLS: [&str; 8] = [
    "breathabilityRtpc",
    "disturbanceVelocityRtpc",
    "firstRoomIsPlanetRoomRtpc",
    "firstRoomIsPlanetRoomNotIncludingShipsRtpc",
    "firstRoomIsPlanetRoomOrVehicleRoomRtpc",
    "humidityRtpc",
    "pressureRtpc",
    "temperatureRtpc",
];

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn emitted_planet_audio_rtpcs_match_one_original_record_per_build() -> TestResult {
    let Some((live, ptu)) = real_archive_pair()? else {
        return Ok(());
    };
    let key = real_archive_key()?;
    for (path, new_controls) in [(live, false), (ptu, true)] {
        let archive = Archive::open_with_key(path, Some(key.clone()))?;
        let watch = ArchiveWatch::new(&archive);
        let entry = DataForgeEntry::read(&archive)?;
        let forge = entry.open()?;
        let record = forge
            .records()
            .find(|row| row.type_name().ok() == Some(SOURCE_TYPE))
            .ok_or("the configured archive contains procedural planet audio")?;
        let tree = TreeBuilder::new().build(&record)?;
        assert_eq!(
            tree.root().field(CONTROLS[3]).is_some(),
            new_controls,
            "the selected pair represents both audited declarations"
        );
        watch.check()?;
        verify_record(&archive, &entry, &tree)?;
        watch.check()?;
        print_line("one authored planet audio record and eight controls checked");
    }
    Ok(())
}

fn verify_record(archive: &Archive, entry: &DataForgeEntry, tree: &RecordTree<'_>) -> TestResult {
    let filter = Filter::new(tree.record_path());
    let options = CatalogOptions {
        filter: Some(&filter),
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..CatalogOptions::new(UNIX_EPOCH)
    };
    let built = build_catalog_from(archive, entry, &options)
        .map_err(|_| "build selected planet audio catalog")?;
    let catalog: Json = serde_json::from_slice(built.json())?;
    verify_build(archive, &catalog)?;
    let index = tree.record_index();
    let row = catalog
        .pointer("/definitions/proceduralPlanetAudio/rows")
        .and_then(Json::as_array)
        .and_then(|rows| {
            rows.iter().find(|row| {
                row.pointer("/recordIndex/value").and_then(Json::as_u64) == Some(u64::from(index))
            })
        })
        .ok_or("the physical planet audio record is exported")?;
    let origins = Origins::read(&catalog, entry.origin())?;
    let locations = Locations::read(tree.root());
    for field in CONTROLS {
        let exported = row.get(field).ok_or("separate RTPC carrier")?;
        let Some(source) = tree.root().field(field) else {
            assert_eq!(
                exported["state"], "absent",
                "missing original control {field} stays absent"
            );
            continue;
        };
        let Value::Struct(control) = source else {
            return Err("the selected authored control has an inline AudioRtpc".into());
        };
        let string = control.field("rtpc").ok_or("authored RTPC String")?;
        let Value::String(expected) = string else {
            return Err("the selected authored RTPC is a readable String".into());
        };
        let leaf = exported.pointer("/value/rtpc").ok_or("typed RTPC leaf")?;
        assert_eq!(
            leaf["state"], "present",
            "the original String in {field} was readable"
        );
        assert!(
            leaf["value"].as_str() == Some(expected),
            "the exact original RTPC String is exported"
        );
        origins.occurrence(leaf, index, locations.ordinal(string)?, "$value")?;
    }
    verify_collections(row, tree, &locations, &origins)
}

fn verify_collections(
    row: &Json,
    tree: &RecordTree<'_>,
    locations: &Locations<'_>,
    origins: &Origins<'_>,
) -> TestResult {
    let index = tree.record_index();
    for name in ["entries", "disturbanceLists", "effectTagMovementRtpcs"] {
        let source = tree
            .root()
            .field(name)
            .ok_or("authored planet audio collection")?;
        let exported = row.get(name).ok_or("exported planet audio collection")?;
        assert_eq!(
            exported["state"], "present",
            "authored collection {name} is available"
        );
        origins.occurrence(exported, index, locations.ordinal(source)?, "$value")?;
    }
    Ok(())
}
