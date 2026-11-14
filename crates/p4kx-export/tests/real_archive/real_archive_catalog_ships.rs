// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The ship base data in the `ships` kind of the P4KX catalog of a real Archive; runs with
//! `--ignored` when `P4KX_DATA_P4K` names one.
//!
//! Run with `--nocapture` to see the counts, the field survey and the build time.
//!
//! The anchors and counts are the exact values of LIVE and follow each patch; a failure after a
//! patch means the value moved, to be checked against the Archive and updated here and in
//! `docs/catalog.md`. The survey test asserts the documented findings on the field
//! names of the ship Records.

use std::collections::BTreeMap;
use std::error::Error;
use std::sync::OnceLock;
use std::time::{Instant, SystemTime};

use p4kx_export::{CatalogOptions, ShipBase, ShipBaseOptions, build_catalog, build_ship_base};
use p4kx_extraction::{Archive, DataForgeEntry, Field, TreeBuilder, Value};
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde_json::Value as Json;

type TestResult = Result<(), Box<dyn Error>>;

struct Anchor {
    class_name: &'static str,
    mass: f64,
    hull: f64,
    cargo: f64,
}

const ANCHORS: [Anchor; 4] = [
    Anchor {
        class_name: "AEGS_Vanguard_Harbinger",
        mass: 230_404.0,
        hull: 48_490.0,
        cargo: 0.0,
    },
    Anchor {
        class_name: "MISC_Freelancer_MAX",
        mass: 341_408.0,
        hull: 35_650.0,
        cargo: 120.0,
    },
    Anchor {
        class_name: "ANVL_Carrack",
        mass: 3_275_858.0,
        hull: 88_000.0,
        cargo: 456.0,
    },
    Anchor {
        class_name: "RSI_Constellation_Andromeda",
        mass: 386_857.0,
        hull: 180_844.0,
        cargo: 96.0,
    },
];

const UNKNOWN_CARGO: [&str; 2] = ["AEGS_Javelin", "ANVL_Hornet_F7C"];

const SURVEY_WORDS: [&str; 9] = [
    "crew",
    "dimension",
    "bound",
    "extent",
    "length",
    "width",
    "height",
    "minimum",
    "seat",
];

const INVENTORY_BOUNDS: [&str; 5] = [
    "/Components/AttachDef/inventoryOccupancyDimensions",
    "/Components/AttachDef/inventoryOccupancyLocalBoundsMin",
    "/Components/AttachDef/inventoryOccupancyLocalBoundsMax",
    "/Components/maxBoundingBoxSize",
    "/Components/hasLocalBounds",
];

#[expect(clippy::print_stdout, reason = "the numbers are the result of the run")]
fn print_line(line: &str) {
    println!("{line}");
}

fn catalog() -> Result<&'static Json, Box<dyn Error>> {
    static CATALOG: OnceLock<Result<Json, String>> = OnceLock::new();
    CATALOG
        .get_or_init(|| {
            let read = || -> Result<Json, Box<dyn Error>> {
                let archive =
                    Archive::open_with_key(real_archive_path()?, real_archive_key().ok())?;
                let started = Instant::now();
                let built = build_catalog(&archive, &CatalogOptions::new(SystemTime::now()))?;
                print_line(&format!(
                    "catalog build {:.2} s, {} bytes",
                    started.elapsed().as_secs_f64(),
                    built.json().len()
                ));
                Ok(serde_json::from_slice(built.json())?)
            };
            read().map_err(|error| error.to_string())
        })
        .as_ref()
        .map_err(|reason| reason.clone().into())
}

fn ship<'c>(catalog: &'c Json, class_name: &str) -> Result<&'c Json, Box<dyn Error>> {
    catalog["ships"]
        .as_array()
        .and_then(|ships| ships.iter().find(|ship| ship["className"] == class_name))
        .ok_or_else(|| format!("the catalog has no ship {class_name}").into())
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn the_known_ships_carry_their_mass_hull_and_cargo() -> TestResult {
    let catalog = catalog()?;
    for anchor in &ANCHORS {
        let ship = ship(catalog, anchor.class_name)?;
        print_line(&format!(
            "{}: mass {} hull {} cargo {} (grids with issue {}) kind {} size {}",
            anchor.class_name,
            ship["mass"],
            ship["hullHealth"],
            ship["cargoScu"],
            ship["cargoGridsWithIssue"],
            ship["vehicleKind"],
            ship["size"]
        ));
        assert_eq!(
            ship["mass"].as_f64(),
            Some(anchor.mass),
            "{}",
            anchor.class_name
        );
        assert_eq!(
            ship["hullHealth"].as_f64(),
            Some(anchor.hull),
            "{}",
            anchor.class_name
        );
        assert_eq!(
            ship["cargoScu"].as_f64(),
            Some(anchor.cargo),
            "{}",
            anchor.class_name
        );
        assert_eq!(ship["vehicleKind"], "spaceship", "{}", anchor.class_name);
    }
    Ok(())
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn dangling_container_references_leave_the_cargo_unknown() -> TestResult {
    let catalog = catalog()?;
    for class_name in UNKNOWN_CARGO {
        let ship = ship(catalog, class_name)?;
        assert!(
            ship["cargoScu"].is_null(),
            "{class_name}: {}",
            ship["cargoScu"]
        );
        assert!(
            ship["cargoGridsWithIssue"].as_u64().is_some_and(|n| n > 0),
            "{class_name}"
        );
    }
    Ok(())
}

fn assert_entry_equals_base(entry: &Json, base: &ShipBase) -> TestResult {
    let name = base.class_name.as_str();
    assert_eq!(
        entry["mass"].as_f64(),
        base.mass.as_ref().map(|mass| mass.kg),
        "{name}"
    );
    assert_eq!(
        entry["hullHealth"].as_f64(),
        base.hull.as_ref().map(|hull| hull.health),
        "{name}"
    );
    assert_eq!(entry["cargoScu"].as_f64(), base.cargo.scu, "{name}");
    assert_eq!(
        entry["cargoGridsWithIssue"].as_u64(),
        u64::try_from(base.cargo.grids_with_issue).ok(),
        "{name}"
    );
    assert_eq!(
        entry["size"].as_i64(),
        base.size.as_ref().map(|size| size.value),
        "{name}"
    );
    let mass_basis = match &base.mass {
        Some(mass) => serde_json::to_value(mass.basis)?,
        None => Json::Null,
    };
    assert_eq!(entry["massBasis"], mass_basis, "{name}");
    assert_eq!(
        entry["vehicleKind"].as_str(),
        base.vehicle_kind.map(p4kx_export::VehicleKind::code),
        "{name}"
    );
    Ok(())
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn the_catalog_ships_equal_the_ship_base_export_and_the_kind_is_settled() -> TestResult {
    let catalog = catalog()?;
    let archive = Archive::open_with_key(real_archive_path()?, real_archive_key().ok())?;
    let started = Instant::now();
    let bases = build_ship_base(&archive, &ShipBaseOptions::new(SystemTime::now()))?;
    print_line(&format!(
        "ship base build {:.2} s",
        started.elapsed().as_secs_f64()
    ));
    let ships = catalog["ships"].as_array().ok_or("ships")?;
    assert_eq!(ships.len(), bases.ships().len());
    let mut kinds: BTreeMap<String, usize> = BTreeMap::new();
    for ship in ships {
        *kinds.entry(ship["vehicleKind"].to_string()).or_default() += 1;
    }
    print_line(&format!("vehicle kinds {kinds:?}"));
    for base in bases.ships() {
        let entry = ships
            .iter()
            .find(|ship| ship["guid"] == base.guid.to_string().as_str())
            .ok_or_else(|| format!("{} is missing", base.class_name))?;
        assert_entry_equals_base(entry, base)?;
    }
    let ground = ships
        .iter()
        .filter(|ship| {
            ship["recordPath"]
                .as_str()
                .is_some_and(|path| path.contains("/entities/groundvehicles/"))
        })
        .all(|ship| ship["vehicleKind"] == "groundVehicle");
    assert!(
        ground,
        "every ship under groundvehicles is a ground vehicle"
    );
    assert!(
        ships.iter().all(|ship| !ship["vehicleKind"].is_null()),
        "no ship has an undecided kind: no Record under spaceships has a wheel or tread"
    );
    Ok(())
}

fn survey(prefix: &str, fields: &[Field<'_>], found: &mut BTreeMap<String, usize>) {
    for field in fields {
        let path = format!("{prefix}/{}", field.name());
        let lower = field.name().to_ascii_lowercase();
        if SURVEY_WORDS.iter().any(|word| lower.contains(word)) {
            *found.entry(path.clone()).or_default() += 1;
        }
        match field.value() {
            Value::Struct(inner) => survey(&path, inner.fields(), found),
            Value::Array(items) => {
                for item in items {
                    if let Value::Struct(inner) = item {
                        survey(&path, inner.fields(), found);
                    }
                }
            }
            _ => {}
        }
    }
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn survey_the_ship_records_for_minimum_crew_and_outer_dimensions() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, real_archive_key().ok())?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let mut trees = TreeBuilder::new();
    let mut found = BTreeMap::new();
    let mut ships = 0_usize;
    for record in forge.records() {
        let Ok(path) = record.record_path() else {
            continue;
        };
        if !path.contains("/entities/spaceships/") && !path.contains("/entities/groundvehicles/") {
            continue;
        }
        let Ok(tree) = trees.build(&record) else {
            continue;
        };
        ships += 1;
        survey("", tree.fields(), &mut found);
    }
    print_line(&format!("surveyed {ships} ship Records"));
    for (path, count) in &found {
        print_line(&format!("{count:5} {path}"));
    }
    assert!(ships > 0);
    assert_documented_findings(&found, ships);
    Ok(())
}

fn assert_documented_findings(found: &BTreeMap<String, usize>, ships: usize) {
    let last = |path: &str| {
        path.rsplit('/')
            .next()
            .unwrap_or_default()
            .to_ascii_lowercase()
    };
    let crew: Vec<&str> = found
        .keys()
        .map(String::as_str)
        .filter(|path| last(path).contains("crew"))
        .collect();
    assert_eq!(
        crew,
        [
            "/Components/crewManifest",
            "/Components/crewSize",
            "/StaticEntityClassData/displayParams/crewSize"
        ],
        "crewSize and crewManifest are the only crew fields; the game states no minimum crew"
    );
    assert!(
        found.keys().all(|path| {
            let name = path.rsplit('/').next().unwrap_or_default();
            !name.starts_with("seat") && !name.contains("Seat")
        }),
        "no field names a seat (useAttachedLookAimPose only contains the letters)"
    );
    for outer in ["length", "width", "height", "dimensions"] {
        assert!(
            found.keys().all(|path| last(path) != outer),
            "no field states the outer size of a ship: {outer}"
        );
    }
    for inventory in INVENTORY_BOUNDS {
        assert_eq!(
            found.get(inventory),
            Some(&ships),
            "{inventory} is on every ship Record and describes inventory space, not the hull"
        );
    }
}
