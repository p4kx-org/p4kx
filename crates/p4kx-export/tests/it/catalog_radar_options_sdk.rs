// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::CatalogRecordSelection;
use p4kx_export::catalog_queries::game_modes::{RadarOptionsSnapshot, with_radar_options};
use p4kx_export::{CatalogOptions, CatalogPublicationMode, build_catalog};
use p4kx_extraction::{DataForgeEntry, SourceValueState};
use serde_json::{Value, json};

use super::fixture::{self, Layout};
use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::health_origin_assertions as proof;
use crate::sdk_full_proof as full_proof;

const DATASET: &str = "/configurations/radarOptions";

#[test]
fn radar_sdk_matches_the_producer_with_a_malformed_resource_network_sibling() -> TestResult {
    let (_folder, archive) =
        common::open(&fixture::database(Layout::Invalid), DcbOrigin::Encrypted)?;
    let options = CatalogOptions {
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..common::options()
    };
    let catalog: Value =
        serde_json::from_slice(build_catalog(&archive, options.with_full_provenance())?.json())?;
    let network = proof::at(
        &catalog,
        "/configurations/resourceNetworkParameters/rows/0/params/value",
    )?;
    proof::cell(network, "resourceNetworkParams", "invalid")?;
    let entry = DataForgeEntry::read(&archive)?;
    full_proof::from_entry(&archive, &entry, full_proof::NO_LANGUAGES, None)?.with_radar_options(
        CatalogRecordSelection::All,
        None,
        |snapshot| check_rows(&snapshot, &catalog),
    )??;
    Ok(())
}

fn check_rows(snapshot: &RadarOptionsSnapshot<'_>, catalog: &Value) -> TestResult {
    let [first, second, alternative, vacant] = snapshot.rows() else {
        return Err("four original physical GameMode Records".into());
    };
    let coverage = snapshot.coverage();
    assert!(
        coverage.incomplete_fields.is_empty() && coverage.unreadable_bodies.is_empty(),
        "a malformed ResourceNetwork sibling cannot make the original radar fields incomplete"
    );
    for (index, row, expected) in [(0, first, true), (1, second, false)] {
        assert_eq!(
            row.source().record_index().value(),
            Some(&index),
            "original physical GameMode Record"
        );
        let mode = row.params().value().ok_or("game-mode parameters")?;
        let radar = mode.radar_option_params().value().ok_or("radar options")?;
        let flag = radar.allow_fps_ping_wave();
        assert_eq!(flag.value(), Some(&expected), "the authored per-mode flag");
        assert_eq!(
            flag.origin().contains_encrypted_source(),
            Some(true),
            "the individual flag originates in the encrypted Entry"
        );
    }
    let alternative = alternative
        .params()
        .value()
        .ok_or("alternative-owned parameters")?;
    assert_eq!(
        alternative.radar_option_params().state(),
        SourceValueState::Absent,
        "an alternative mode has no radar declaration"
    );
    assert_eq!(
        vacant.params().state(),
        SourceValueState::Null,
        "the original owned pointer is explicitly null"
    );
    parity(snapshot, catalog)
}

#[test]
fn radar_sdk_flags_a_malformed_selected_radar_branch_and_retains_its_raw_value() -> TestResult {
    let (_folder, archive) = common::open(
        &fixture::database(Layout::WrongRadarType),
        DcbOrigin::Encrypted,
    )?;
    with_radar_options(
        &archive,
        CatalogRecordSelection::RecordIndex(0),
        None,
        |snapshot| -> TestResult {
            let [row] = snapshot.rows() else {
                return Err("exact physical GameMode selection".into());
            };
            assert_eq!(
                snapshot.coverage().incomplete_fields,
                [0],
                "the SDK reports the malformed selected radar branch"
            );
            let mode = row
                .params()
                .value()
                .ok_or("selected game-mode parameters")?;
            assert_eq!(
                mode.radar_option_params().state(),
                SourceValueState::Unsupported,
                "the alternate source class is unsupported"
            );
            let raw = mode
                .raw_radar_option_params()
                .ok_or("retained original radar branch")?;
            assert_eq!(
                raw.state(),
                SourceValueState::Present,
                "raw source data is readable"
            );
            assert_eq!(
                raw.origin().contains_encrypted_source(),
                Some(true),
                "the fallback carries the original source provenance"
            );
            Ok(())
        },
    )??;
    Ok(())
}

fn parity(snapshot: &RadarOptionsSnapshot<'_>, catalog: &Value) -> TestResult {
    assert!(
        snapshot.requested_languages().is_empty(),
        "physical game modes require no localization inputs"
    );
    let table = proof::at(catalog, DATASET)?;
    assert_eq!(
        table.get("availability"),
        Some(&serde_json::to_value(snapshot.availability())?),
        "identical source declaration availability"
    );
    let provenance = json!({"provenance": snapshot.source_inventory()});
    let rows = table.get("rows").ok_or("canonical radar-option rows")?;
    assert_eq!(
        proof::with_origins(serde_json::to_value(snapshot.rows())?, &provenance)?,
        proof::with_origins(rows.clone(), catalog)?,
        "physical radar rows and original per-field origins match the producer"
    );
    Ok(())
}
