// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The contract facts in the Basetool catalog of a real Archive; runs with `--ignored` when
//! `P4KX_DATA_P4K` names one.

use std::error::Error;
use std::time::SystemTime;

use p4kx_export::{CatalogOptions, build_catalog};
use p4kx_extraction::Archive;
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde_json::Value;

use crate::real_archive_contracts::v3;

type TestResult = Result<(), Box<dyn Error>>;

const LAYERS: [&str; 4] = ["subContract", "contract", "handler", "template"];

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn contracts_of_a_real_archive_state_their_facts_and_the_level_of_each() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, Some(real_archive_key()?))?;
    let built = build_catalog(&archive, &CatalogOptions::new(SystemTime::now()))?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let contracts = v3::rows(&catalog)?;
    assert!(!contracts.is_empty(), "contracts");
    let mut resolved = false;
    for entry in contracts {
        for field in ["timeToComplete", "buyInAmount"] {
            assert!(
                v3::cell(entry, field)?.is_some_and(Value::is_number),
                "{field}"
            );
        }
        for field in [
            "onceOnly",
            "maxPlayersPerInstance",
            "availableInPrison",
            "canReacceptAfterAbandoning",
            "canReacceptAfterFailing",
            "hideInMobiGlas",
        ] {
            let fact = entry.get(field).ok_or(field)?;
            let layer = v3::cell(fact, "layer")?
                .and_then(Value::as_str)
                .ok_or(field)?;
            let value = v3::cell(fact, "value")?.ok_or(field)?;
            assert!(LAYERS.contains(&layer), "{field}: {layer}");
            assert!(
                if field == "maxPlayersPerInstance" {
                    value.as_i64().is_some()
                } else {
                    value.is_boolean()
                },
                "{field} retains its typed value"
            );
        }
        let requirements = v3::cell(entry, "requiredIntros")?
            .and_then(Value::as_array)
            .ok_or("present intro list")?;
        for required in requirements {
            let grants = v3::cell(required, "introContracts")?
                .and_then(Value::as_array)
                .ok_or("present intro grants")?;
            resolved |= !grants.is_empty();
        }
    }
    assert!(resolved, "an intro contract grants a required tag");
    Ok(())
}
