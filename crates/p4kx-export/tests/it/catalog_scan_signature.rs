// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The scan signature of the mineable entities when the radar parameters do not name its slot.

use crate::harvest_fixture::{ENTITY, Radar, write_archive_with};

use std::error::Error;
use std::time::{Duration, UNIX_EPOCH};

use p4kx_export::{BuiltCatalog, CatalogOptions, CraftingWarning, Filter, build_catalog};
use p4kx_extraction::Archive;
use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn Error>>;

fn build(radar: Radar, filter: Option<&Filter>) -> Result<(BuiltCatalog, Value), Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let archive = Archive::open_with_key(
        write_archive_with(dir.path(), radar)?,
        Some(p4kx_testkit::test_key()),
    )?;
    let mut options = CatalogOptions::new(UNIX_EPOCH + Duration::from_secs(1_792_000_000));
    options.publication_mode = p4kx_export::CatalogPublicationMode::DiagnosticPartial;
    options.filter = filter;
    let built = build_catalog(&archive, &options)?;
    let json = serde_json::from_slice(built.json())?;
    Ok((built, json))
}

fn slot_warnings(built: &BuiltCatalog) -> usize {
    built
        .summary()
        .warnings()
        .iter()
        .filter(|warning| matches!(warning, CraftingWarning::UnknownScanSignatureSlot(_)))
        .count()
}

fn iron(catalog: &Value) -> Result<&Value, Box<dyn Error>> {
    catalog["mineableEntities"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|entity| entity["className"] == "MineableRock_Iron")
        .ok_or_else(|| format!("no iron in {ENTITY}").into())
}

fn assert_unknown(radar: Radar) -> TestResult {
    let (built, catalog) = build(radar, None)?;
    let rock = iron(&catalog)?;
    assert_eq!(rock["scanSignature"], Value::Null, "{radar:?}");
    assert_eq!(
        rock["signatures"],
        json!([0.0, 0.0, 4270.0, 0.0]),
        "{radar:?}"
    );
    assert_eq!(slot_warnings(&built), 1, "{radar:?}");
    Ok(())
}

#[test]
fn a_named_slot_raises_no_warning() -> TestResult {
    let (built, _) = build(Radar::Named, None)?;
    assert_eq!(slot_warnings(&built), 0);
    Ok(())
}

#[test]
fn without_radar_parameters_the_signature_is_null_and_a_warning_says_so() -> TestResult {
    assert_unknown(Radar::Missing)
}

#[test]
fn a_renamed_key_leaves_the_signature_null_and_warns() -> TestResult {
    assert_unknown(Radar::Renamed)
}

#[test]
fn a_duplicate_key_is_ambiguous_and_not_the_first_hit() -> TestResult {
    assert_unknown(Radar::Duplicate)
}

#[test]
fn a_filtered_build_does_not_warn_about_the_slot() -> TestResult {
    let filter = Filter::new("*providerpresets*hpp_unreached*");
    let (built, _) = build(Radar::Missing, Some(&filter))?;
    assert_eq!(slot_warnings(&built), 0);
    Ok(())
}
