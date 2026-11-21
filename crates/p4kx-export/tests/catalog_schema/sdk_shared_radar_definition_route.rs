// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::UNIX_EPOCH;

use p4kx_export::catalog_queries::equipment::component_definitions::{
    RadarSystemSharedParams, RadarSystemSharedParamsSnapshot,
};
use p4kx_export::catalog_queries::{CatalogRecordSelection, ParsedCatalogQueries, SourcePayload};
use p4kx_export::{CatalogOptions, CatalogPublicationMode, build_catalog};
use p4kx_extraction::{Archive, Filter, InputScope};
use serde_json::Value;

use super::fixture;
use super::{Route, TestResult, parity};

const DATASET: &str = "/definitions/radarSystemSharedParams";

pub(super) fn read(archive: &Archive, parsed: &ParsedCatalogQueries<'_>) -> TestResult<Route> {
    check_original_link(parsed)?;
    let options = CatalogOptions {
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..CatalogOptions::new(UNIX_EPOCH)
    };
    let catalog: Value = serde_json::from_slice(build_catalog(archive, &options)?.json())?;
    let route = parsed.with_radar_system_shared_params(
        CatalogRecordSelection::All,
        None,
        |snapshot| -> TestResult<Route> {
            check_definitions(&snapshot, &catalog)?;
            Route::queried_rows(
                DATASET,
                snapshot.rows(),
                "complete authored arena and encrypted synthetic producer parity available. Installed producer parity open",
            )
        },
    )??;
    check_filtered_target(archive, parsed, options)?;
    Ok(route)
}

fn check_original_link(parsed: &ParsedCatalogQueries<'_>) -> TestResult<()> {
    parsed.with_item_radars(
        CatalogRecordSelection::RecordIndex(fixture::FIRST),
        None,
        |snapshot| -> TestResult<()> {
            let [radar] = snapshot.rows() else {
                return Err("one original radar component".into());
            };
            let link = radar
                .shared_params()
                .ok_or("original shared parameter Reference")?;
            assert_eq!(
                link.resolved_index(),
                Some(fixture::RADAR_TARGET),
                "the first physical target admitted by DataForge",
            );
            assert_eq!(
                link.target_record_index()
                    .origin()
                    .contains_encrypted_source(),
                Some(true),
                "the original target identity retains encryption evidence",
            );
            let arena = radar.source().additional_fields();
            let experimental = arena
                .roots()
                .iter()
                .find(|field| field.name().value().copied() == Some("sharedExperimentalParams"))
                .ok_or("original experimental Reference")?;
            assert!(
                arena
                    .value(experimental.value())
                    .is_some_and(|node| node.original_error().is_some()),
                "the unrelated dangling experimental Reference retains its original error",
            );
            Ok(())
        },
    )?
}

fn check_definitions(
    snapshot: &RadarSystemSharedParamsSnapshot<'_>,
    catalog: &Value,
) -> TestResult<()> {
    parity(DATASET, snapshot, catalog)?;
    let [first, second] = snapshot.rows() else {
        return Err("both physical definitions of the same GUID".into());
    };
    assert_eq!(
        first.source().guid().value(),
        second.source().guid().value(),
        "the authored GUID can be repeated",
    );
    assert_eq!(
        first.source().record_index().value(),
        Some(&fixture::RADAR_TARGET),
        "exact referenced Record definition",
    );
    assert_eq!(
        second.source().record_index().value(),
        Some(&fixture::RADAR_TARGET_ALIAS),
        "same-GUID sibling remains a distinct physical definition",
    );
    check_arena_value(snapshot, first, true)?;
    check_arena_value(snapshot, second, false)
}

fn check_arena_value(
    snapshot: &RadarSystemSharedParamsSnapshot<'_>,
    row: &RadarSystemSharedParams<'_>,
    expected: bool,
) -> TestResult<()> {
    let arena = row.authored_fields();
    let [settings] = arena.roots() else {
        return Err("one authored settings root".into());
    };
    let root = arena
        .value(settings.value())
        .ok_or("the original settings value")?;
    let Some(SourcePayload::Struct { fields, .. }) = root.value().value() else {
        return Err("the authored settings structure".into());
    };
    let [flag] = fields.as_slice() else {
        return Err("one original Boolean field".into());
    };
    let flag = arena
        .value(flag.value())
        .ok_or("the original Boolean")?
        .value();
    assert!(
        matches!(flag.value(), Some(SourcePayload::Boolean { value }) if *value == expected),
        "the original nested Boolean stays bound to its physical owner",
    );
    let origin = flag.origin();
    assert_eq!(
        origin.contains_encrypted_source(),
        Some(true),
        "encrypted DataForge value"
    );
    assert!(
        origin.source_set_complete()
            && snapshot.source_inventory().iter_uses().any(|usage| {
                usage.scope() == InputScope::Payload
                    && origin.source_uses_refs().contains(usage.id())
            }),
        "the original field resolves in the selective query inventory",
    );
    assert_eq!(
        snapshot.available_languages(),
        [],
        "the radar definition needs no locale"
    );
    Ok(())
}

fn check_filtered_target(
    archive: &Archive,
    parsed: &ParsedCatalogQueries<'_>,
    options: CatalogOptions<'_>,
) -> TestResult<()> {
    let filter = Filter::parse("fixtures/worn/first")?;
    let options = CatalogOptions {
        filter: Some(&filter),
        ..options
    };
    let filtered: Value = serde_json::from_slice(build_catalog(archive, &options)?.json())?;
    parsed.with_radar_system_shared_params(
        CatalogRecordSelection::RecordIndex(fixture::RADAR_TARGET),
        None,
        |snapshot| {
            assert_eq!(snapshot.rows().len(), 1, "one exact target definition");
            parity(DATASET, &snapshot, &filtered)
        },
    )?
}
