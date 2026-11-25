// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::sdk_full_proof as full_proof;
use p4kx_export::catalog_queries::equipment::resource_network::{
    ResourceNetworkGlobalSnapshot, with_resource_network_globals,
};
use p4kx_export::catalog_queries::{CatalogRecordSelection, ResourceFloat32};
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};
use serde_json::{Value, json};

use super::fixture;
use crate::health_origin_assertions as origin;
use crate::health_origin_fixture::{Fixture, Input, TestResult};

const DATASET: &str = "/definitions/resourceNetworkGlobals";

#[test]
fn resource_network_global_sdk_retains_canonical_rows_and_per_value_evidence() -> TestResult {
    let fixture = Fixture::open(&fixture::database(), Input::Encrypted, Vec::new())?;
    let (_, catalog) = fixture.diagnostic()?;
    full_proof::from_entry(
        &fixture.archive,
        &fixture.entry,
        full_proof::NO_LANGUAGES,
        None,
    )?
    .with_resource_network_globals(
        CatalogRecordSelection::RecordIndex(0),
        None,
        |snapshot| check_row(&snapshot, &catalog),
    )??;
    with_resource_network_globals(
        &fixture.archive,
        CatalogRecordSelection::RecordIndex(u32::MAX),
        None,
        |snapshot| {
            assert!(
                snapshot.rows().is_empty(),
                "only the selected physical definition is read"
            );
        },
    )?;
    Ok(())
}

fn check_row(snapshot: &ResourceNetworkGlobalSnapshot<'_>, catalog: &Value) -> TestResult {
    let [row] = snapshot.rows() else {
        return Err("one original global resource definition".into());
    };
    assert_eq!(
        row.source().record_index().value(),
        Some(&0),
        "the physical definition Record index"
    );
    let distribution = row
        .default_power_distribution_params()
        .value()
        .ok_or("distribution defaults")?;
    let cooler = distribution.power_dist_cooler_min();
    assert_eq!(
        cooler.value().map(ResourceFloat32::raw_bits),
        Some((-0.0_f32).to_bits()),
        "the cooler allocation retains the authored Single bits"
    );
    assert_eq!(
        cooler.origin().contains_encrypted_source(),
        Some(true),
        "the specific allocation field retains its encrypted origin"
    );
    let power = row.power_params().value().ok_or("power configuration")?;
    let modifiers = power.modifiers().value().ok_or("original modifiers")?;
    let [first, second] = modifiers.as_slice() else {
        return Err("two original power modifier slots".into());
    };
    for (original, amount) in [(first, -2), (second, 0)] {
        let modifier = original.value().ok_or("original modifier")?;
        assert_eq!(
            modifier.additive_modifier().value(),
            Some(&amount),
            "the ordinal-specific additive amount"
        );
    }
    check_parity(snapshot, catalog)
}

fn check_parity(snapshot: &ResourceNetworkGlobalSnapshot<'_>, catalog: &Value) -> TestResult {
    let coverage = snapshot.coverage();
    assert!(
        snapshot.requested_languages().is_empty()
            && coverage.incomplete_fields.is_empty()
            && coverage.unreadable_bodies.is_empty(),
        "the selected global definition uses a complete DataForge-only scope"
    );
    let table = origin::at(catalog, DATASET)?;
    assert_eq!(
        table.get("availability"),
        Some(&serde_json::to_value(snapshot.availability())?),
        "the query and producer share source declaration availability"
    );
    let provenance = json!({"provenance": snapshot.source_inventory()});
    let rows = table.get("rows").ok_or("canonical resource-network rows")?;
    assert_eq!(
        origin::with_origins(serde_json::to_value(snapshot.rows())?, &provenance)?,
        origin::with_origins(rows.clone(), catalog)?,
        "the canonical rows and exact original field and slot origins agree"
    );
    Ok(())
}

#[test]
fn resource_network_global_sdk_reports_unreadable_owned_extensions() -> TestResult {
    let data = DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new(fixture::GLOBAL).field("fixtureString", TestType::String))
        .record(
            TestRecord::new(
                0x1166,
                fixture::GLOBAL,
                "GlobalResources.Malformed",
                "synthetic/elsewhere/malformed.xml",
            )
            .value(
                "fixtureString",
                TestValue::Raw((u32::MAX - 1).to_le_bytes().to_vec()),
            ),
        )
        .build();
    let fixture = Fixture::open(&data, Input::Encrypted, Vec::new())?;
    with_resource_network_globals(
        &fixture.archive,
        CatalogRecordSelection::All,
        None,
        |snapshot| {
            assert_eq!(
                snapshot.rows().len(),
                1,
                "the physical owner still has a partial typed row"
            );
            assert_eq!(
                snapshot.coverage().incomplete_fields,
                [0],
                "an unreadable retained extension affects the whole definition's coverage"
            );
        },
    )?;
    Ok(())
}
