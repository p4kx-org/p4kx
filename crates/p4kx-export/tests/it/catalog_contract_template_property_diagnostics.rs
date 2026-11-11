// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Catalog diagnostics for unknown Template structures and vocabulary values.

use std::time::{Duration, UNIX_EPOCH};

use p4kx_export::{
    BuiltCatalog, CatalogOptions, CatalogPublicationMode, CraftingWarning, build_catalog,
};
use p4kx_extraction::Archive;
use serde_json::Value;

use crate::spawns_fixture;

use super::TestResult;

#[test]
fn readable_phases_and_flow_survive_while_unknown_struct_failures_stay_explicit() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = Archive::open_with_key(
        spawns_fixture::write_archive(dir.path())?,
        Some(p4kx_testkit::test_key()),
    )?;
    let mut options = CatalogOptions::new(UNIX_EPOCH + Duration::from_secs(1_792_000_000));
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    let phases = crate::template_phase_legacy::source(&archive)?;
    let built = build_catalog(&archive, &options)?;
    let failed = failures(&built);
    assert_eq!(failed.len(), UNKNOWN_STRUCTS.len(), "{failed:?}");
    assert_failures(&failed, &UNKNOWN_STRUCTS);
    assert!(
        !failed.iter().any(|(path, _)| path.ends_with("/deep_flow")),
        "readable flow has no depth quota"
    );
    let catalog: Value = serde_json::from_slice(built.json())?;
    let rows = catalog
        .pointer("/definitions/contractTemplates/rows")
        .and_then(Value::as_array)
        .ok_or("canonical physical Template rows")?;
    assert_eq!(
        rows.len(),
        12,
        "all readable D owners survive, including unknown required C meanings"
    );
    crate::template_phase_legacy::output(&catalog, &phases)?;
    crate::template_fixture_deep_flow::check(&catalog)?;
    Ok(())
}

const ORGANIZATION: &str =
    "organization condition holds the unknown struct DataSetMatchCondition_Mystery";
const ITEM: &str = "item condition holds the unknown struct DataSetMatchCondition_Mystery";
const LOCATION: &str = "location condition holds the unknown struct DataSetMatchCondition_Mystery";
const VALUE: &str = "property value holds the unknown struct MissionPropertyValue_Mystery";

/// The Records of the fixture that fail on an unknown struct: a record path suffix and what the
/// reason states.
const UNKNOWN_STRUCTS: [(&str, &str); 12] = [
    ("/unknown_condition", ORGANIZATION),
    (
        "/unknown_flow",
        "holds the unknown struct MissionFlowCondition_Unknown",
    ),
    (
        "/unknown_modifier",
        "modifiers holds the unknown struct MissionModifier",
    ),
    ("/unknown_value", VALUE),
    ("/unknown_contract_property", VALUE),
    ("/unknown_item_condition", ITEM),
    ("/unknown_location_condition", LOCATION),
    ("/unknown_location_after_known", LOCATION),
    ("/contractgenerator/unknown_location", LOCATION),
    (
        "/missionbroker/bad",
        "holds the unknown struct SReputationMissionRequirementsUnknown",
    ),
    ("/missionbroker/bad_property", VALUE),
    ("/missionbroker/bad_location", LOCATION),
];

#[test]
fn a_struct_the_catalog_does_not_know_fails_the_record_instead_of_hiding_behind_other() -> TestResult
{
    let dir = tempfile::tempdir()?;
    let archive = Archive::open_with_key(
        spawns_fixture::write_archive(dir.path())?,
        Some(p4kx_testkit::test_key()),
    )?;
    let mut options = CatalogOptions::new(UNIX_EPOCH + Duration::from_secs(1_792_000_000));
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    let built = build_catalog(&archive, &options)?;
    assert_failures(&failures(&built), &UNKNOWN_STRUCTS);
    Ok(())
}

/// The record path and reason of every failed Record.
fn failures(built: &BuiltCatalog) -> Vec<(&str, &str)> {
    let mut failed: Vec<(&str, &str)> = built
        .summary()
        .failures()
        .iter()
        .map(|failure| (failure.record_path(), failure.reason()))
        .collect();
    failed.sort_unstable();
    failed
}

/// Asserts that a failed Record ends with each suffix and states each text in its reason.
fn assert_failures(failed: &[(&str, &str)], expected: &[(&str, &str)]) {
    for (suffix, text) in expected {
        let found = failed
            .iter()
            .any(|(path, reason)| path.ends_with(suffix) && reason.contains(text));
        assert!(found, "{suffix} {text}: {failed:?}");
    }
}

#[test]
fn stored_names_the_catalog_does_not_know_are_counted_in_a_warning() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = Archive::open_with_key(
        spawns_fixture::write_archive(dir.path())?,
        Some(p4kx_testkit::test_key()),
    )?;
    let mut options = CatalogOptions::new(UNIX_EPOCH + Duration::from_secs(1_792_000_000));
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    let built = build_catalog(&archive, &options)?;
    let warning = built
        .summary()
        .warnings()
        .iter()
        .find(|warning| warning.kind() == "unknown_template_values")
        .ok_or("the warning")?;
    assert_eq!(warning.count(), Some(5), "{:?}", warning.details());
    assert_eq!(
        warning.details(),
        [
            "comparison Sideways (1)",
            "end comms reason Vanish (1)",
            "flow state Sleeping (1)",
            "month Smarch (1)",
            "reaction Gloating (1)"
        ]
    );
    assert!(matches!(
        warning,
        CraftingWarning::UnknownTemplateValues { .. }
    ));
    Ok(())
}
