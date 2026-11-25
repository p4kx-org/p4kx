// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::game_modes::{
    ResourceNetworkParametersSnapshot, with_resource_network_parameters,
};
use p4kx_export::catalog_queries::{CatalogRecordSelection, ResourceFloat32};
use p4kx_export::{CatalogOptions, CatalogPublicationMode, build_catalog};
use p4kx_extraction::{DataForgeEntry, SourceValueState};
use serde_json::{Value, json};

use super::fixture::{self, Layout};
use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::health_origin_assertions as proof;
use crate::sdk_full_proof as full_proof;

const DATASET: &str = "/configurations/resourceNetworkParameters";

#[test]
fn network_sdk_matches_canonical_rules_despite_an_unsupported_radar_sibling() -> TestResult {
    let (_folder, archive) = common::open(
        &fixture::database(Layout::WrongRadarType),
        DcbOrigin::Encrypted,
    )?;
    let options = CatalogOptions {
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..common::options()
    };
    let catalog: Value =
        serde_json::from_slice(build_catalog(&archive, options.with_full_provenance())?.json())?;
    let radar = proof::at(&catalog, "/configurations/radarOptions/rows/0/params/value")?;
    proof::cell(radar, "radarOptionParams", "unsupported")?;
    let entry = DataForgeEntry::read(&archive)?;
    full_proof::from_entry(&archive, &entry, full_proof::NO_LANGUAGES, None)?
        .with_resource_network_parameters(CatalogRecordSelection::All, None, |snapshot| {
            check_rules(&snapshot)?;
            parity(&snapshot, &catalog)
        })??;
    Ok(())
}

fn check_rules(snapshot: &ResourceNetworkParametersSnapshot<'_>) -> TestResult {
    let [first, second, alternate, vacant] = snapshot.rows() else {
        return Err("four physical game-mode owners".into());
    };
    for (row, index, enabled) in [(first, 0, true), (second, 1, false)] {
        assert_eq!(
            row.source().record_index().value(),
            Some(&index),
            "original game-mode Record index"
        );
        let params = row.params().value().ok_or("owned game-mode parameters")?;
        let network = params
            .resource_network_params()
            .value()
            .ok_or("owned resource-network parameters")?;
        let flag = network.mode().enable_item_resource_network();
        assert_eq!(flag.value(), Some(&enabled), "authored mode-specific flag");
        assert_eq!(
            flag.origin().contains_encrypted_source(),
            Some(true),
            "the flag's own encryption marker"
        );
        assert_eq!(
            network
                .destruction()
                .health_threshold()
                .value()
                .map(ResourceFloat32::raw_bits),
            Some(fixture::THRESHOLD_BITS),
            "the threshold retains the authored floating-point payload",
        );
    }
    let alternate = alternate
        .params()
        .value()
        .ok_or("original alternate parameters")?;
    assert_eq!(
        alternate.resource_network_params().state(),
        SourceValueState::Absent,
        "the alternate mode declares no network rules",
    );
    assert_eq!(
        vacant.params().state(),
        SourceValueState::Null,
        "explicit null original pointer"
    );
    Ok(())
}

fn parity(snapshot: &ResourceNetworkParametersSnapshot<'_>, catalog: &Value) -> TestResult {
    assert!(
        snapshot.requested_languages().is_empty()
            && snapshot.coverage().incomplete_fields.is_empty()
            && snapshot.coverage().unreadable_bodies.is_empty(),
        "the unrelated radar branch cannot affect network scope coverage",
    );
    let table = proof::at(catalog, DATASET)?;
    assert_eq!(
        table.get("availability"),
        Some(&serde_json::to_value(snapshot.availability())?),
        "identical source-family availability",
    );
    let provenance = json!({"provenance": snapshot.source_inventory()});
    let canonical = table.get("rows").ok_or("original network rows")?;
    assert_eq!(
        proof::with_origins(serde_json::to_value(snapshot.rows())?, &provenance)?,
        proof::with_origins(canonical.clone(), catalog)?,
        "all original source uses and scoped rows match the producer",
    );
    Ok(())
}

#[test]
fn network_sdk_marks_an_unreadable_authored_extension_incomplete() -> TestResult {
    let (_folder, archive) = common::open(
        &fixture::database(Layout::UnreadableNetworkExtension),
        DcbOrigin::Encrypted,
    )?;
    with_resource_network_parameters(
        &archive,
        CatalogRecordSelection::RecordIndex(0),
        None,
        |snapshot| {
            assert_eq!(
                snapshot.coverage().incomplete_fields,
                [0],
                "an unmodeled field read failure affects the selected owner's coverage",
            );
            let [row] = snapshot.rows() else {
                return Err("the selected physical owner retains its usable fields".into());
            };
            let params = row.params().value().ok_or("owned game-mode parameters")?;
            let network = params
                .resource_network_params()
                .value()
                .ok_or("owned resource-network parameters")?;
            assert_eq!(
                network.mode().enable_item_resource_network().value(),
                Some(&true),
                "readable flags remain available",
            );
            Ok::<_, Box<dyn std::error::Error>>(())
        },
    )??;
    Ok(())
}
