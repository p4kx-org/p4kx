// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::CatalogRecordSelection;
use p4kx_export::catalog_queries::planet_audio::{
    ProceduralPlanetAudioSnapshot, with_procedural_planet_audio, with_procedural_planet_audio_from,
};
use p4kx_extraction::SourceValueState;
use serde_json::{Value, json};

use super::fixture::{self, Layout};
use crate::health_origin_assertions as origin;
use crate::health_origin_fixture::{Fixture, Input, TestResult};

const TABLE: &str = "/definitions/proceduralPlanetAudio";

#[test]
fn planet_audio_sdk_matches_the_canonical_table_and_ignores_an_unrelated_record() -> TestResult {
    let fixture = Fixture::open(
        &fixture::database(Layout::Complete),
        Input::Encrypted,
        Vec::new(),
    )?;
    let (_, catalog) = fixture.diagnostic()?;
    with_procedural_planet_audio_from(
        &fixture.archive,
        &fixture.entry,
        CatalogRecordSelection::All,
        None,
        |snapshot| check_parity(&snapshot, &catalog),
    )??;
    with_procedural_planet_audio(
        &fixture.archive,
        CatalogRecordSelection::RecordIndex(1),
        None,
        |snapshot| {
            assert!(
                snapshot.rows().is_empty() && snapshot.coverage().incomplete_fields.is_empty(),
                "the adjacent Record uses a different original type"
            );
        },
    )?;
    Ok(())
}

fn check_parity(snapshot: &ProceduralPlanetAudioSnapshot<'_>, catalog: &Value) -> TestResult {
    let [row] = snapshot.rows() else {
        return Err("one original procedural planet audio definition".into());
    };
    assert_eq!(
        row.source().record_index().value(),
        Some(&0),
        "original physical audio Record"
    );
    let carrier = row.controls().first_room_is_planet_room();
    let control = carrier.value().ok_or("authored audio control")?;
    let name = control.rtpc();
    assert_eq!(
        name.value().map(String::as_str),
        Some("synthetic_existing"),
        "authored identifier"
    );
    assert_eq!(
        name.origin().contains_encrypted_source(),
        Some(true),
        "per-value encrypted source"
    );
    assert!(
        snapshot.requested_languages().is_empty()
            && snapshot.coverage().unreadable_bodies.is_empty()
            && snapshot.coverage().incomplete_fields.is_empty(),
        "the query is complete within its original audio definition scope",
    );
    let table = origin::at(catalog, TABLE)?;
    assert_eq!(
        table.get("availability"),
        Some(&serde_json::to_value(snapshot.availability())?),
        "source declaration availability",
    );
    let provenance = json!({"provenance": snapshot.source_inventory()});
    let rows = table.get("rows").ok_or("canonical definition rows")?;
    assert_eq!(
        origin::with_origins(serde_json::to_value(snapshot.rows())?, &provenance)?,
        origin::with_origins(rows.clone(), catalog)?,
        "the query retains canonical rows and exact field source uses",
    );
    Ok(())
}

#[test]
fn planet_audio_sdk_retains_partial_rows_for_unreadable_owned_controls() -> TestResult {
    let fixture = Fixture::open(
        &fixture::database(Layout::Unreadable),
        Input::Encrypted,
        Vec::new(),
    )?;
    with_procedural_planet_audio(
        &fixture.archive,
        CatalogRecordSelection::RecordIndex(0),
        None,
        |snapshot| -> TestResult {
            assert_eq!(
                snapshot.coverage().incomplete_fields,
                [0],
                "selected owner had an unreadable field"
            );
            let [row] = snapshot.rows() else {
                return Err("the physical owner still has its canonical partial row".into());
            };
            let control = row
                .controls()
                .first_room_is_planet_room()
                .value()
                .ok_or("the original owned class")?;
            assert_eq!(
                control.rtpc().state(),
                SourceValueState::Unreadable,
                "the failed source value is distinct from absence"
            );
            assert_eq!(
                row.entries().state(),
                SourceValueState::Present,
                "other original declarations remain usable"
            );
            Ok(())
        },
    )??;
    Ok(())
}
