// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Helpers of the loadouts tests: build the fixture catalog and read its JSON.

use crate::loadouts_fixture::write_archive;

use std::error::Error;
use std::time::{Duration, UNIX_EPOCH};

use p4kx_export::{BuiltCatalog, CatalogOptions, Filter, build_catalog};
use p4kx_extraction::Archive;
use serde_json::Value;

const EXPORTED_AT: Duration = Duration::from_secs(1_792_000_000);

pub(crate) fn build(filter: Option<&Filter>) -> Result<(BuiltCatalog, Value), Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let archive =
        Archive::open_with_key(write_archive(dir.path())?, Some(p4kx_testkit::test_key()))?;
    let mut options = CatalogOptions::new(UNIX_EPOCH + EXPORTED_AT);
    options.publication_mode = p4kx_export::CatalogPublicationMode::DiagnosticPartial;
    options.filter = filter;
    let built = build_catalog(&archive, &options)?;
    let json = serde_json::from_slice(built.json())?;
    Ok((built, json))
}

pub(crate) fn guid(value: u128) -> String {
    let text = format!("{value:032x}");
    format!(
        "{}-{}-{}-{}-{}",
        &text[0..8],
        &text[8..12],
        &text[12..16],
        &text[16..20],
        &text[20..32]
    )
}

pub(crate) fn loadout(catalog: &Value, entity: u128) -> Result<&Value, Box<dyn Error>> {
    let wanted = guid(entity);
    Ok(catalog["loadouts"]
        .as_array()
        .ok_or("loadouts")?
        .iter()
        .find(|entry| entry["entityGuid"] == wanted.as_str())
        .ok_or("the loadout")?)
}

pub(crate) fn port<'c>(ports: &'c Value, name: &str) -> Result<&'c Value, Box<dyn Error>> {
    Ok(ports
        .as_array()
        .ok_or("ports")?
        .iter()
        .find(|port| port["name"] == name)
        .ok_or("the port")?)
}

pub(crate) fn known(catalog: &Value) -> Vec<&str> {
    ["items", "ships"]
        .into_iter()
        .flat_map(|kind| catalog[kind].as_array().into_iter().flatten())
        .filter_map(|entity| entity["guid"].as_str())
        .collect()
}

pub(crate) fn names(ports: &Value) -> Vec<&str> {
    ports
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|port| port["name"].as_str())
        .collect()
}
