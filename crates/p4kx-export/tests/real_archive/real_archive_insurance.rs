// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The ship insurance of the Basetool catalog of a real Archive; runs
//! with `--ignored` when `P4KX_DATA_P4K` names one.
//!
//! Run with `--nocapture` to see the counts. The anchors are the exact values of LIVE and follow
//! each patch; a failure after a patch means the value moved, to be checked against the Archive
//! and updated here and in `docs/basetool-catalog.md`.

use std::error::Error;
use std::sync::OnceLock;
use std::time::SystemTime;

use p4kx_export::{CatalogOptions, build_catalog};
use p4kx_extraction::Archive;
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde_json::Value;

type TestResult = Result<(), Box<dyn Error>>;

#[expect(clippy::print_stdout, reason = "the numbers are the result of the run")]
fn print_line(line: &str) {
    println!("{line}");
}

fn catalog() -> Result<&'static Value, Box<dyn Error>> {
    static CATALOG: OnceLock<Result<Value, String>> = OnceLock::new();
    CATALOG
        .get_or_init(|| {
            let read = || -> Result<Value, Box<dyn Error>> {
                let archive =
                    Archive::open_with_key(real_archive_path()?, real_archive_key().ok())?;
                let built = build_catalog(&archive, &CatalogOptions::new(SystemTime::now()))?;
                Ok(serde_json::from_slice(built.json())?)
            };
            read().map_err(|error| error.to_string())
        })
        .as_ref()
        .map_err(|reason| reason.clone().into())
}

fn ship<'c>(catalog: &'c Value, class_name: &str) -> Result<&'c Value, Box<dyn Error>> {
    catalog["ships"]
        .as_array()
        .and_then(|ships| ships.iter().find(|ship| ship["className"] == class_name))
        .ok_or_else(|| format!("the catalog has no ship {class_name}").into())
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn every_ship_of_a_real_archive_carries_its_insurance() -> TestResult {
    let catalog = catalog()?;
    let ships = catalog["ships"]
        .as_array()
        .ok_or("the catalog has no ships")?;
    let insured = ships
        .iter()
        .filter(|ship| !ship["insurance"].is_null())
        .count();
    let with_fee = ships
        .iter()
        .filter(|ship| ship["insurance"]["expediteFee"].is_u64())
        .count();
    print_line(&format!(
        "{} ships, {insured} with insurance, {with_fee} with an expedite fee",
        ships.len()
    ));
    assert!(insured > 0, "no ship carries insurance");
    assert_eq!(with_fee, insured, "every insurance states its fee");
    Ok(())
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn the_gladius_and_the_idris_carry_the_values_of_their_records() -> TestResult {
    let catalog = catalog()?;
    let gladius = ship(catalog, "AEGS_Gladius")?;
    print_line(&format!("AEGS_Gladius: {}", gladius["insurance"]));
    let insurance = &gladius["insurance"];
    assert_eq!(insurance["restoreOnClaim"], true, "restoreOnClaim");
    assert_eq!(insurance["shipEntityClassName"], "AEGS_Gladius", "class");
    assert_eq!(
        insurance["baseWaitTimeMinutes"].as_f64(),
        Some(5.83),
        "claim time"
    );
    assert_eq!(
        insurance["mandatoryWaitTimeMinutes"].as_f64(),
        Some(1.94),
        "mandatory wait"
    );
    assert_eq!(insurance["expediteFee"], 2335, "fee");
    assert_eq!(
        gladius["recordPath"], "libs/foundry/records/entities/spaceships/aegs_gladius",
        "the source path"
    );
    let idris = ship(catalog, "AEGS_Idris_M")?;
    print_line(&format!("AEGS_Idris_M: {}", idris["insurance"]));
    assert_eq!(idris["insurance"]["expediteFee"], 150_990, "Idris-M fee");
    Ok(())
}
