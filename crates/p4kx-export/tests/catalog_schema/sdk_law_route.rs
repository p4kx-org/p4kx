// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::UNIX_EPOCH;

use p4kx_export::catalog_queries::law::{LawFamily, LawRows, LawSnapshot, with_law, with_law_from};
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection,
};
use p4kx_export::{CatalogOptions, build_catalog_from};
use p4kx_extraction::{Archive, DataForgeEntry, Language, SourceUseRole};
use schemars::JsonSchema;
use serde::Serialize;
use serde_json::{Value, json};

use super::{Route, TestResult};

#[path = "sdk_law_fixture.rs"]
mod fixture;

use fixture::{FixtureMode, archive};

const INFRACTIONS: &str = "/definitions/infractions";
const INFRACTION_SETS: &str = "/definitions/infractionSets";
const JURISDICTIONS: &str = "/definitions/jurisdictions";
const SECURITY: &str = "/definitions/securityNetworkManifests";
const PUBLIC_NAMES: &str = "/texts/publicNames";
const GAME_TEXTS: &str = "/texts/gameTexts";
pub(super) const PATHS: [&str; 6] = [
    INFRACTIONS,
    INFRACTION_SETS,
    JURISDICTIONS,
    SECURITY,
    PUBLIC_NAMES,
    GAME_TEXTS,
];

const TEXTS_REMAINING: &str =
    "owner-scoped synthetic producer parity witnessed; installed and full cross-owner parity open";

pub(super) fn read() -> TestResult<[Route; 6]> {
    let (_dir, archive) = archive(FixtureMode::Canonical)?;
    let entry = DataForgeEntry::read(&archive)?;
    let catalog = serde_json::from_slice(
        build_catalog_from(&archive, &entry, &CatalogOptions::new(UNIX_EPOCH))?.json(),
    )?;
    with_law_from(
        &archive,
        &entry,
        LawFamily::All,
        CatalogRecordSelection::All,
        CatalogLanguageSelection::AllAvailable,
        None,
        |snapshot| {
            check_text_and_payload(&snapshot, &catalog)?;
            Ok([
                witness(INFRACTIONS, snapshot.infractions, &catalog)?,
                witness(INFRACTION_SETS, snapshot.infraction_sets, &catalog)?,
                witness(JURISDICTIONS, snapshot.jurisdictions, &catalog)?,
                witness(SECURITY, snapshot.security_network_manifests, &catalog)?,
                Route::queried_rows(PUBLIC_NAMES, &snapshot.public_names, TEXTS_REMAINING)?,
                Route::queried_rows(GAME_TEXTS, &snapshot.game_texts, TEXTS_REMAINING)?,
            ])
        },
    )?
}

fn witness<Row: Serialize + JsonSchema>(
    path: &'static str,
    table: Option<LawRows<Row>>,
    catalog: &Value,
) -> TestResult<Route> {
    let table = table.ok_or("requested definition family")?;
    let projected = json!({"availability": table.availability, "rows": &table.rows});
    assert_eq!(
        Some(&projected),
        catalog.pointer(path),
        "SDK preserves canonical rows and origins for {path}"
    );
    Route::queried_rows(
        path,
        &table.rows,
        "synthetic definition, origin and producer parity witnessed; installed producer parity open",
    )
}

fn check_text_and_payload(snapshot: &LawSnapshot<'_>, catalog: &Value) -> TestResult {
    assert!(
        snapshot.coverage.unreadable_headers.is_empty()
            && snapshot.coverage.unreadable_bodies.is_empty()
            && snapshot.coverage.incomplete_fields.is_empty(),
        "law selection retains complete readable source coverage"
    );
    assert_eq!(
        snapshot.available_languages,
        [Language::English],
        "only the present locale is read"
    );
    assert!(
        !snapshot.public_names.is_empty(),
        "the authored names resolve to one public domain"
    );
    assert!(
        !snapshot.game_texts.is_empty(),
        "the authored description resolves to protected text"
    );
    assert_eq!(
        catalog.pointer(PUBLIC_NAMES),
        Some(&serde_json::to_value(&snapshot.public_names)?),
        "canonical public-name rows"
    );
    assert_eq!(
        catalog.pointer(GAME_TEXTS),
        Some(&serde_json::to_value(&snapshot.game_texts)?),
        "canonical protected text and translations"
    );
    let [row] = snapshot
        .infractions
        .as_ref()
        .ok_or("infraction table")?
        .rows
        .as_slice()
    else {
        return Err("one authored definition".into());
    };
    let origin = row.description.origin();
    assert_eq!(
        origin.contains_encrypted_source(),
        Some(true),
        "description key uses encrypted original DCB"
    );
    assert!(
        origin.source_set_complete(),
        "all description contributors are known"
    );
    assert!(
        snapshot.source_inventory.iter_uses().any(|used| {
            origin.source_uses_refs().contains(used.id()) && used.role() == SourceUseRole::Selector
        }),
        "the description reference cites the authored locale selector"
    );
    Ok(())
}

#[test]
fn law_sdk_scopes_exact_records_localization_and_reference_targets() -> TestResult {
    let (_dir, archive) = archive(FixtureMode::PhysicalAliases)?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let wanted = forge.record(0).ok_or("original infraction owner")?.guid();
    let session = CatalogQuerySession::from_entry(
        &archive,
        &entry,
        CatalogLanguageSelection::Selected(&[Language::English]),
        None,
    )?;
    let parsed = session.parsed(None)?;
    let other = parsed.with_law(
        LawFamily::Infractions,
        CatalogRecordSelection::Guid(wanted),
        None,
        |snapshot| -> TestResult<u32> {
            assert!(
                snapshot.infraction_sets.is_none(),
                "no unrelated family projected"
            );
            let definitions = snapshot
                .infractions
                .ok_or("requested infraction definitions")?;
            let [first, second] = definitions.rows.as_slice() else {
                return Err("both physical aliases".into());
            };
            assert_eq!(
                first.source.record_index().value(),
                Some(&0),
                "first physical declaration"
            );
            let index = second
                .source
                .record_index()
                .value()
                .copied()
                .ok_or("physical alias index")?;
            assert_ne!(index, 0, "same GUID retains a second physical owner");
            Ok(index)
        },
    )??;
    check_physical_alias(&session, other)?;
    check_security_scope(&archive)?;
    check_original_set_links(&session)?;
    Ok(())
}

fn check_physical_alias(session: &CatalogQuerySession<'_>, other: u32) -> TestResult {
    session.with_law(
        LawFamily::Infractions,
        CatalogRecordSelection::RecordIndex(other),
        None,
        |snapshot| -> TestResult {
            let rows = snapshot.infractions.ok_or("selected family")?.rows;
            let [row] = rows.as_slice() else {
                return Err("one selected occurrence".into());
            };
            assert_eq!(
                row.source.record_index().value(),
                Some(&other),
                "exact physical alias"
            );
            Ok(())
        },
    )??;
    Ok(())
}

fn check_security_scope(archive: &Archive) -> TestResult {
    with_law(
        archive,
        LawFamily::SecurityNetworkManifests,
        CatalogRecordSelection::All,
        CatalogLanguageSelection::Selected(&[Language::English]),
        None,
        |snapshot| -> TestResult {
            assert!(
                snapshot.available_languages.is_empty(),
                "security manifests have no locale inputs"
            );
            assert!(
                snapshot.infractions.is_none(),
                "infraction bodies stay outside the request"
            );
            let rows = snapshot
                .security_network_manifests
                .ok_or("requested security family")?
                .rows;
            let [row] = rows.as_slice() else {
                return Err("one security manifest".into());
            };
            assert_eq!(
                row.is_neutral_territory.value(),
                Some(&false),
                "explicit false remains present"
            );
            Ok(())
        },
    )?
}

fn check_original_set_links(session: &CatalogQuerySession<'_>) -> TestResult {
    session.with_law(
        LawFamily::InfractionSets,
        CatalogRecordSelection::All,
        None,
        |snapshot| -> TestResult {
            assert!(
                snapshot.infractions.is_none(),
                "reference does not request infraction bodies"
            );
            let rows = snapshot.infraction_sets.ok_or("requested sets")?.rows;
            let [row] = rows.as_slice() else {
                return Err("one reusable set".into());
            };
            let slots = row.infractions.value().ok_or("authored uses")?;
            let [first, second] = slots.as_slice() else {
                return Err("two original slots".into());
            };
            let first = first.value().ok_or("first original slot")?;
            let second = second.value().ok_or("second original slot")?;
            let first = first.definition.value().ok_or("first original reference")?;
            let second = second
                .definition
                .value()
                .ok_or("second original reference")?;
            assert_eq!(
                first.guid().value(),
                second.guid().value(),
                "repeated target GUIDs stay ordered"
            );
            assert_eq!(
                first.resolved_index(),
                second.resolved_index(),
                "a repeated link retains the original resolution"
            );
            Ok(())
        },
    )?
}
