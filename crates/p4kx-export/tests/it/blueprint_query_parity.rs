// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::{
    BlueprintPresentation, BlueprintSnapshot, CatalogGuid, CatalogLanguageSelection,
    CatalogRecordSelection, with_blueprints,
};
use p4kx_export::{CatalogPublicationMode, build_catalog};
use p4kx_extraction::{CancelToken, DataForgeEntry, SourcedValue};
use serde_json::Value;

use crate::canonical_header::json::assert_canonical;
use crate::sdk_full_proof as full_proof;

use super::blueprint_consumer_walk::{visit_definition, visit_presentation, visit_tier};
use super::blueprint_query_archive as archive_fixture;
use super::blueprint_query_fixture as fixture;
use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};

#[test]
fn selected_blueprints_match_full_canonical_rows_and_all_origin_states() -> TestResult {
    for (origin, expected) in [
        (DcbOrigin::Plain, Some(false)),
        (DcbOrigin::Encrypted, Some(true)),
        (DcbOrigin::Unknown, None),
    ] {
        let (_folder, archive) = archive_fixture::open(&fixture::database(), origin, true)?;
        let mut options = common::options();
        options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
        let built = build_catalog(&archive, options.with_full_provenance())?;
        let catalog: Value = serde_json::from_slice(built.json())?;
        let entry = DataForgeEntry::read(&archive)?;
        let session = full_proof::from_entry(
            &archive,
            &entry,
            CatalogLanguageSelection::AllAvailable,
            None,
        )?;
        let parsed = session.parsed(None)?;
        parsed.with_blueprints(
            CatalogRecordSelection::Guid(common::guid(fixture::OWNER).parse()?),
            None,
            |snapshot| verify_snapshot(&snapshot, &catalog, expected),
        )??;
    }
    Ok(())
}

fn verify_snapshot(
    snapshot: &BlueprintSnapshot<'_>,
    catalog: &Value,
    expected: Option<bool>,
) -> TestResult {
    let [definition] = snapshot.definitions() else {
        return Err("one selected physical Blueprint".into());
    };
    let physical = definition
        .header()
        .record_index()
        .value()
        .copied()
        .ok_or("Blueprint owner index")?;
    let definition_rows = rows(catalog, "/definitions/blueprints/rows")?;
    let canonical = definition_rows
        .iter()
        .find(|row| {
            row.pointer("/recordIndex/value").and_then(Value::as_u64) == Some(u64::from(physical))
        })
        .ok_or("canonical selected definition")?;
    visit_definition(definition, snapshot.source_inventory(), canonical)?;
    assert_canonical(
        definition,
        canonical,
        "Selected definition equals the canonical owner",
    )?;
    verify_tiers(snapshot, catalog, physical)?;
    verify_presentation(snapshot, catalog, physical, expected)?;
    Ok(())
}

fn verify_presentation(
    snapshot: &BlueprintSnapshot<'_>,
    catalog: &Value,
    physical: u32,
    expected: Option<bool>,
) -> TestResult {
    let definition = snapshot
        .definitions()
        .first()
        .ok_or("physical Blueprint owner")?;
    let presentations = rows(catalog, "/projections/blueprintPresentation/rows")?;
    let canonical = presentations
        .iter()
        .find(|row| {
            row.pointer("/blueprint/recordIndex")
                .and_then(Value::as_u64)
                == Some(u64::from(physical))
        })
        .ok_or("canonical selected presentation")?;
    let [presentation] = snapshot.presentations() else {
        return Err("one selected Blueprint presentation".into());
    };
    assert_canonical(
        presentation,
        canonical,
        "Selected presentation equals the canonical choice",
    )?;
    visit_presentation(
        presentation,
        definition,
        snapshot.source_inventory(),
        canonical,
    )?;
    assert_eq!(
        presentation
            .default_available()
            .origin()
            .contains_encrypted_source(),
        expected,
        "Default availability retains all original rule contributors"
    );
    verify_texts(snapshot, presentation)?;
    Ok(())
}

fn verify_texts(
    snapshot: &BlueprintSnapshot<'_>,
    presentation: &BlueprintPresentation<SourcedValue<CatalogGuid>>,
) -> TestResult {
    let text = snapshot
        .game_text(
            presentation
                .name()
                .value()
                .ok_or("protected Blueprint name")?,
        )
        .ok_or("protected name row")?;
    assert_eq!(
        text.key().value().copied(),
        Some(fixture::NAME_KEY),
        "The original protected name key is preserved"
    );
    assert_eq!(
        text.translations()
            .value()
            .ok_or("name translations")?
            .get("en")
            .ok_or("English name")?
            .source_text
            .origin()
            .contains_encrypted_source(),
        Some(true),
        "The encrypted locale contributes to the translated name"
    );
    assert_eq!(
        snapshot.public_names().len(),
        0,
        "Blueprint names stay in the protected text domain"
    );
    Ok(())
}

fn verify_tiers(snapshot: &BlueprintSnapshot<'_>, catalog: &Value, physical: u32) -> TestResult {
    let definition = snapshot
        .definitions()
        .first()
        .ok_or("physical Blueprint owner")?;
    let canonical = rows(catalog, "/configurations/blueprintTiers/rows")?
        .iter()
        .filter(|row| {
            row.pointer("/blueprint/recordIndex")
                .and_then(Value::as_u64)
                == Some(u64::from(physical))
        })
        .collect::<Vec<_>>();
    assert_eq!(
        snapshot.tiers().len(),
        canonical.len(),
        "All tiers of the selected owner are retained"
    );
    for (tier, expected) in snapshot.tiers().iter().zip(canonical) {
        assert_canonical(
            tier,
            expected,
            "The complete tier equals the canonical projection",
        )?;
        visit_tier(tier, definition, snapshot.source_inventory(), expected)?;
    }
    Ok(())
}

fn rows<'value>(catalog: &'value Value, pointer: &str) -> TestResult<&'value [Value]> {
    catalog
        .pointer(pointer)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| format!("canonical Blueprint table {pointer}").into())
}

#[test]
fn blueprint_definition_queries_do_not_open_existing_locked_world_or_unused_locales() -> TestResult
{
    let (_folder, archive) = archive_fixture::open(&fixture::database(), DcbOrigin::Plain, false)?;
    let before = archive.key_demand().snapshot().unread();
    let selected = CatalogRecordSelection::Guid(common::guid(fixture::OWNER).parse()?);
    with_blueprints(
        &archive,
        selected,
        CatalogLanguageSelection::Selected(&[]),
        None,
        |snapshot| {
            assert_eq!(
                snapshot.definitions().len(),
                1,
                "The selected Blueprint can be read without locale payloads"
            );
        },
    )?;
    assert_eq!(
        archive.key_demand().snapshot().unread(),
        before,
        "World and unused locale payloads remain unopened"
    );
    with_blueprints(
        &archive,
        CatalogRecordSelection::Guid(common::guid(fixture::CATEGORY).parse()?),
        CatalogLanguageSelection::AllAvailable,
        None,
        |snapshot| {
            assert_eq!(
                snapshot.definitions().len(),
                0,
                "Foreign family selection has no Blueprint owner"
            );
            assert_eq!(
                snapshot.available_languages().len(),
                0,
                "No successful owner means no locale payload reads"
            );
        },
    )?;
    assert_eq!(
        archive.key_demand().snapshot().unread(),
        before,
        "Foreign selection leaves encrypted entries untouched"
    );
    Ok(())
}

#[test]
fn cancelled_blueprint_queries_do_not_call_consumers() -> TestResult {
    let (_folder, archive) = archive_fixture::open(&fixture::database(), DcbOrigin::Plain, false)?;
    let cancel = CancelToken::new();
    cancel.cancel();
    let called = std::cell::Cell::new(false);
    let result = with_blueprints(
        &archive,
        CatalogRecordSelection::All,
        CatalogLanguageSelection::Selected(&[]),
        Some(&cancel),
        |_| called.set(true),
    );
    assert!(
        matches!(result, Err(p4kx_export::CatalogError::Cancelled)),
        "Cancellation remains an explicit query failure"
    );
    assert!(
        !called.get(),
        "Cancelled queries never enter the consumer callback"
    );
    Ok(())
}
