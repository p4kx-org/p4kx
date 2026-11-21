// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Catalog checks for physical mining configurations and variant identity.

#[path = "catalog_mining_configurations.rs"]
mod configurations;

use p4kx_export::build_catalog;
use p4kx_extraction::Archive;
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as common, TestResult};
use crate::gear_fixture::{self, LASER, LASER_RANGE, LASER_TWIN, MODULE, MODULE_INSTABILITY};
use crate::health_origin_assertions as proof;

fn catalog() -> TestResult<Value> {
    let dir = tempfile::tempdir()?;
    let archive = Archive::open_with_key(
        gear_fixture::write_archive(dir.path())?,
        Some(p4kx_testkit::test_key()),
    )?;
    let built = build_catalog(&archive, &common::options())?;
    Ok(serde_json::from_slice(built.json())?)
}

fn variant(catalog: &Value, guid: u128) -> TestResult<&Value> {
    let guid = common::guid(guid);
    let item = proof::rows(catalog, "/items")?
        .iter()
        .find(|item| item.pointer("/guid/value").and_then(Value::as_str) == Some(guid.as_str()))
        .ok_or_else(|| format!("item {guid}"))?;
    proof::at(item, "/variantOf")
}

#[test]
fn lasers_equal_in_every_compared_field_are_variants() -> TestResult {
    let catalog = catalog()?;
    assert!(
        variant(&catalog, LASER)? == &json!(common::guid(LASER_TWIN))
            || variant(&catalog, LASER_TWIN)? == &json!(common::guid(LASER)),
        "one of the twins is the base of the other",
    );
    Ok(())
}

#[test]
fn a_beam_range_or_a_module_modifier_starts_a_variant_group_of_its_own() -> TestResult {
    let catalog = catalog()?;
    assert!(
        variant(&catalog, LASER_RANGE)?.is_null(),
        "a different range is no variant"
    );
    for guid in [LASER, LASER_TWIN] {
        assert_ne!(variant(&catalog, guid)?, &json!(common::guid(LASER_RANGE)));
    }
    assert!(variant(&catalog, MODULE)?.is_null());
    assert!(variant(&catalog, MODULE_INSTABILITY)?.is_null());
    Ok(())
}
