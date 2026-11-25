// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use p4kx_export::catalog_queries::equipment::{self, ItemDefinitionSnapshot};
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogRecordSelection, DatasetAvailability,
};
use p4kx_export::{CatalogError, CatalogPublicationMode};
use p4kx_extraction::{Archive, CancelToken, DataForgeEntry, Language};
use serde_json::Value;

use super::{fixture, values, walk};
use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::health_origin_assertions as proof;
use crate::sdk_full_proof as full_proof;

#[test]
fn item_definition_query_getters_match_complete_canonical_cells_for_all_origin_states() -> TestResult
{
    for layout in fixture::ALL {
        for (origin, marker) in origins() {
            let (_folder, archive) = common::open(&fixture::database(layout), origin)?;
            let entry = DataForgeEntry::read(&archive)?;
            let catalog = canonical(&archive)?;
            full_proof::from_entry(
                &archive,
                &entry,
                CatalogLanguageSelection::Selected(&[Language::English, Language::German]),
                None,
            )?
            .with_item_definitions(
                CatalogRecordSelection::Guid(common::guid(fixture::OWNER).parse()?),
                None,
                |snapshot| verify(&snapshot, &catalog, &[0, 1], layout, marker),
            )??;
            proof::closure(&catalog)?;
        }
    }
    Ok(())
}

#[test]
fn item_definition_query_keeps_physical_alias_selection_in_a_reused_parsed_view() -> TestResult {
    for (origin, marker) in origins() {
        let (_folder, archive) =
            common::open(&fixture::database(fixture::Layout::Complete), origin)?;
        let entry = DataForgeEntry::read(&archive)?;
        let catalog = canonical(&archive)?;
        let session = full_proof::from_entry(
            &archive,
            &entry,
            CatalogLanguageSelection::Selected(&[Language::English, Language::German]),
            None,
        )?;
        let parsed = session.parsed(None)?;
        parsed.with_item_definitions(
            CatalogRecordSelection::RecordIndex(1),
            None,
            |snapshot| verify(&snapshot, &catalog, &[1], fixture::Layout::Complete, marker),
        )??;
    }
    Ok(())
}

#[test]
fn item_definition_query_can_return_original_keys_without_loading_locales() -> TestResult {
    let (_folder, archive) = common::open(
        &fixture::database(fixture::Layout::Complete),
        DcbOrigin::Plain,
    )?;
    let catalog = canonical(&archive)?;
    full_proof::open(&archive, CatalogLanguageSelection::Selected(&[]), None)?
        .with_item_definitions(CatalogRecordSelection::All, None, |snapshot| {
            assert!(
                snapshot.requested_languages().is_empty(),
                "no locale was requested"
            );
            assert!(
                snapshot.available_languages().is_empty(),
                "no locale was read for the query"
            );
            for row in snapshot.rows() {
                let index = *row
                    .source()
                    .record_index()
                    .value()
                    .ok_or("physical Item index")?;
                walk::definition(
                    row,
                    snapshot.source_inventory(),
                    canonical_row(&catalog, index)?,
                )?;
            }
            for text in snapshot.public_names() {
                assert!(
                    text.translations().value().is_some_and(BTreeMap::is_empty),
                    "the original public key has no invented translation"
                );
            }
            Ok::<_, Box<dyn std::error::Error>>(())
        })??;
    Ok(())
}

#[test]
fn cancelled_item_definition_query_has_no_consumer_callback() -> TestResult {
    let (_folder, archive) = common::open(
        &fixture::database(fixture::Layout::Complete),
        DcbOrigin::Plain,
    )?;
    let cancel = CancelToken::new();
    cancel.cancel();
    let result: Result<(), CatalogError> = equipment::with_item_definitions(
        &archive,
        CatalogRecordSelection::All,
        CatalogLanguageSelection::Selected(&[]),
        Some(&cancel),
        |_| panic!("a cancelled definition query cannot expose a result"),
    );
    assert!(
        matches!(result, Err(CatalogError::Cancelled)),
        "cancellation propagates before callback admission"
    );
    Ok(())
}

fn verify(
    snapshot: &ItemDefinitionSnapshot<'_>,
    catalog: &Value,
    selected: &[u32],
    layout: fixture::Layout,
    marker: Option<bool>,
) -> TestResult {
    assert!(
        matches!(snapshot.availability(), DatasetAvailability::Available),
        "the original Item family is available"
    );
    assert_eq!(
        snapshot.rows().len(),
        selected.len(),
        "retain every selected physical alias"
    );
    for (row, &index) in snapshot.rows().iter().zip(selected) {
        assert_eq!(
            row.source().record_index().value(),
            Some(&index),
            "physical selection and canonical order"
        );
        assert_eq!(
            row.source().guid().origin().contains_encrypted_source(),
            marker,
            "the original header keeps its own marker"
        );
        walk::definition(
            row,
            snapshot.source_inventory(),
            canonical_row(catalog, index)?,
        )?;
        values::verify(row, layout)?;
    }
    verify_metadata(snapshot);
    verify_texts(snapshot, catalog)
}

fn verify_metadata(snapshot: &ItemDefinitionSnapshot<'_>) {
    let requested = snapshot.requested_languages();
    assert_eq!(requested.len(), 2, "both requested language codes survive");
    for language in [Language::English, Language::German] {
        assert!(
            requested.contains(&language),
            "the requested locale is retained"
        );
        assert!(
            snapshot.available_languages().contains(&language),
            "the actual locale payload was read"
        );
    }
    assert_eq!(
        snapshot.available_languages().len(),
        2,
        "only the two fixture locales are admitted"
    );
    assert!(
        snapshot.localization_notes().is_empty(),
        "the locale sources decode without loss"
    );
    assert!(
        snapshot.source_inventory().iter_sources().next().is_some(),
        "actual source metadata is retained"
    );
    assert!(
        snapshot.build().is_none(),
        "the fixture supplies no Build manifest"
    );
    let coverage = snapshot.coverage();
    assert!(
        coverage.unreadable_headers.is_empty(),
        "the fixture headers are readable"
    );
    assert!(
        coverage.unreadable_bodies.is_empty(),
        "field errors do not erase the original readable owner body"
    );
    assert_eq!(
        coverage.skipped_leftovers, 0,
        "the fixture contains no development leftovers"
    );
}

fn verify_texts(snapshot: &ItemDefinitionSnapshot<'_>, catalog: &Value) -> TestResult {
    for text in snapshot.public_names() {
        let id = text.id().value().ok_or("public text ID")?;
        assert!(
            snapshot.public_name(id).is_some(),
            "public IDs resolve inside the same snapshot"
        );
        walk::text(text, snapshot.source_inventory(), catalog, "publicNames")?;
    }
    for text in snapshot.game_texts() {
        let id = text.id().value().ok_or("protected text ID")?;
        assert!(
            snapshot.game_text(id).is_some(),
            "protected IDs resolve inside the same snapshot"
        );
        walk::text(text, snapshot.source_inventory(), catalog, "gameTexts")?;
    }
    Ok(())
}

fn canonical(archive: &Archive) -> TestResult<Value> {
    let mut options = common::options();
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    let built = p4kx_export::build_catalog(archive, options.with_full_provenance())?;
    Ok(serde_json::from_slice(built.json())?)
}

fn canonical_row(catalog: &Value, index: u32) -> TestResult<&Value> {
    proof::rows(catalog, fixture::ROWS)?
        .iter()
        .find(|row| {
            row.pointer("/recordIndex/value").and_then(Value::as_u64) == Some(u64::from(index))
        })
        .ok_or_else(|| "selected canonical physical definition".into())
}

const fn origins() -> [(DcbOrigin, Option<bool>); 3] {
    [
        (DcbOrigin::Plain, Some(false)),
        (DcbOrigin::Encrypted, Some(true)),
        (DcbOrigin::Unknown, None),
    ]
}
