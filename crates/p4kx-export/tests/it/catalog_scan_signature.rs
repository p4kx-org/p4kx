// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The scan signature of the mineable entities when the radar parameters do not name its slot.

use crate::harvest_fixture::{ENTITY, Radar, write_archive_with};

use std::error::Error;
use std::time::{Duration, UNIX_EPOCH};

use p4kx_export::{BuiltCatalog, CatalogOptions, CraftingWarning, Filter, build_catalog};
use p4kx_extraction::Archive;
use serde_json::Value;

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
    catalog
        .pointer("/definitions/mineableEntities/rows")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .find(|entity| {
            entity.pointer("/className/value").and_then(Value::as_str) == Some("MineableRock_Iron")
        })
        .ok_or_else(|| format!("no iron in {ENTITY}").into())
}

fn assert_unknown(radar: Radar) -> TestResult {
    let (built, catalog) = build(radar, None)?;
    let rock = iron(&catalog)?;
    let base = rock
        .pointer("/radar/0/properties/value/baseSignatureParams/value")
        .ok_or("base signature parameters")?;
    assert_eq!(
        base.pointer("/scanSignature/value"),
        Some(&Value::Null),
        "an unidentified scan slot has no derived value: {radar:?}"
    );
    let slots = base
        .pointer("/signatures/value")
        .and_then(Value::as_array)
        .ok_or("signatures")?;
    let values: Vec<_> = slots
        .iter()
        .map(|slot| slot.pointer("/value/finite").and_then(Value::as_f64))
        .collect();
    assert_eq!(
        values,
        [Some(0.0), Some(0.0), Some(4270.0), Some(0.0)],
        "{radar:?}"
    );
    assert_eq!(slot_warnings(&built), 1, "{radar:?}");
    Ok(())
}

#[test]
fn a_named_slot_raises_no_warning() -> TestResult {
    let (built, _) = build(Radar::Named, None)?;
    assert_eq!(slot_warnings(&built), 0, "the named scan slot is resolved");
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
    assert_eq!(
        slot_warnings(&built),
        0,
        "a filtered build has no admitted mineable requiring scan-slot resolution"
    );
    Ok(())
}
