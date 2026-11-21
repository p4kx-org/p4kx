// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::build_catalog;
use p4kx_export::catalog_queries::factions::{
    FactionDefinition, FactionSnapshot, FactionText, with_factions, with_factions_from,
};
use p4kx_export::catalog_queries::{CatalogLanguageSelection, CatalogRecordSelection};
use p4kx_extraction::DataForgeEntry;
use serde::Serialize;
use serde_json::{Value, json};

use super::{ROWS, fixture};
use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::health_origin_assertions as origin;

#[test]
fn faction_sdk_matches_canonical_rows_and_both_text_domains() -> TestResult {
    let (_directory, archive) = common::open(&fixture::sdk_neighbors(), DcbOrigin::Encrypted)?;
    let catalog: Value =
        serde_json::from_slice(build_catalog(&archive, &common::options())?.json())?;
    let entry = DataForgeEntry::read(&archive)?;
    with_factions_from(
        &archive,
        &entry,
        CatalogRecordSelection::All,
        CatalogLanguageSelection::AllAvailable,
        None,
        |snapshot| check_parity(&snapshot, &catalog),
    )??;
    Ok(())
}

fn check_parity(snapshot: &FactionSnapshot<'_>, catalog: &Value) -> TestResult {
    let [selected, _, _] = snapshot.rows() else {
        return Err("three original physical Faction definitions".into());
    };
    let coverage = snapshot.coverage();
    assert!(
        coverage.unreadable_headers.is_empty()
            && coverage.unreadable_bodies.is_empty()
            && coverage.incomplete_fields.is_empty(),
        "an unreadable different record family does not affect Faction coverage",
    );
    let availability = serde_json::to_value(snapshot.availability())?;
    assert_eq!(
        common::at(catalog, "/definitions/factions/availability")?,
        &availability,
        "canonical declared-family availability",
    );
    let provenance = json!({"provenance": snapshot.source_inventory()});
    assert_eq!(
        origin::with_origins(serde_json::to_value(snapshot.rows())?, &provenance)?,
        origin::with_origins(common::at(catalog, ROWS)?.clone(), catalog)?,
        "exact ordered Faction rows and their original immediate source uses",
    );
    assert_eq!(
        selected
            .able_to_arrest()
            .origin()
            .contains_encrypted_source(),
        Some(true),
        "inherited faction flag retains its encrypted original field",
    );
    check_links(selected)?;
    check_text_domains(selected, snapshot, catalog, &provenance)
}

fn check_links(row: &FactionDefinition<'_>) -> TestResult {
    let links = row.allied_factions().value().ok_or("original ally slots")?;
    let [first, dangling, repeated] = links.as_slice() else {
        return Err("three original ally link occurrences".into());
    };
    let first = first.value().ok_or("original ally target")?;
    let dangling = dangling.value().ok_or("original unresolved ally")?;
    let repeated = repeated.value().ok_or("original repeated ally")?;
    assert!(
        first.resolved_index().is_some() && first.resolved_index() == repeated.resolved_index(),
        "repeated physical target keeps both authored slots",
    );
    assert!(
        dangling.resolved_index().is_none(),
        "a dangling source link preserves its slot without a target"
    );
    Ok(())
}

fn check_text_domains(
    row: &FactionDefinition<'_>,
    snapshot: &FactionSnapshot<'_>,
    catalog: &Value,
    provenance: &Value,
) -> TestResult {
    let Some(FactionText::Localized { reference: name }) = row.name().value() else {
        return Err("original name reference in the public domain".into());
    };
    let Some(FactionText::Localized {
        reference: description,
    }) = row.description().value()
    else {
        return Err("original description reference in the protected domain".into());
    };
    let public = snapshot
        .public_name(name)
        .ok_or("selected public text row")?;
    let protected = snapshot
        .game_text(description)
        .ok_or("selected protected text row")?;
    assert_eq!(
        public.key().value(),
        Some(&fixture::SECURITY_NAME),
        "public name resolves its authored locale key",
    );
    assert_eq!(
        protected.key().value(),
        public.key().value(),
        "both domains retain the same original key"
    );
    assert_ne!(
        name.as_str(),
        description.as_str(),
        "separate domain identities"
    );
    check_text(
        public,
        common::text_row(catalog, "publicNames", &json!(name))?,
        catalog,
        provenance,
    )?;
    check_text(
        protected,
        common::text_row(catalog, "gameTexts", &json!(description))?,
        catalog,
        provenance,
    )
}

fn check_text(
    row: &impl Serialize,
    expected: &Value,
    catalog: &Value,
    provenance: &Value,
) -> TestResult {
    let row = serde_json::to_value(row)?;
    assert_eq!(
        common::at(&row, "/translations/value/de/displayText/value")?,
        "Gemeinsamer Text",
        "requested available translation belongs to the selected text row",
    );
    assert_eq!(
        origin::with_origins(row, provenance)?,
        origin::with_origins(expected.clone(), catalog)?,
        "canonical text, translation and original source uses",
    );
    Ok(())
}

#[test]
fn faction_sdk_reports_an_unreadable_authored_extension() -> TestResult {
    let (_directory, archive) = common::open(&fixture::unreadable_faction(), DcbOrigin::Encrypted)?;
    with_factions(
        &archive,
        CatalogRecordSelection::RecordIndex(1),
        CatalogLanguageSelection::Selected(&[]),
        None,
        |snapshot| -> TestResult {
            assert_eq!(
                snapshot.coverage().incomplete_fields,
                [1],
                "a failed authored extension affects its exact physical owner",
            );
            let [row] = snapshot.rows() else {
                return Err("the original Faction retains its partial row".into());
            };
            assert!(
                matches!(
                    row.name().value(),
                    Some(FactionText::Literal {
                        text: "Fixture Faction"
                    })
                ),
                "the independently readable original name remains available",
            );
            Ok(())
        },
    )??;
    Ok(())
}
