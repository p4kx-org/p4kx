// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::CatalogPublicationMode;
use p4kx_export::catalog_queries::equipment::{
    AttachmentConstraintSnapshot, EquipmentSnapshot, ItemBoundsSnapshot,
};
use p4kx_export::catalog_queries::{CatalogLanguageSelection, CatalogRecordSelection};
use p4kx_extraction::{DataForgeEntry, Language};
use serde_json::Value;

use super::{
    ATTACHMENTS, BOUNDS, attachment_walk, attachments, bounds, bounds_walk, catalog, common,
    entities, selected_rows,
};
use crate::canonical_header::wire::Wire;
use crate::commodity_v3_fixture::{DcbOrigin, TestResult};
use crate::sdk_full_proof as full_proof;

#[test]
fn configuration_queries_walk_every_canonical_field_and_origin_state() -> TestResult {
    for (origin, marker) in [
        (DcbOrigin::Plain, Some(false)),
        (DcbOrigin::Encrypted, Some(true)),
        (DcbOrigin::Unknown, None),
    ] {
        let (_folder, archive) = common::open(&attachments::original_restrictions(), origin)?;
        let expected = catalog(&archive, CatalogPublicationMode::DiagnosticPartial)?;
        let entry = DataForgeEntry::read(&archive)?;
        full_proof::from_entry(&archive, &entry, full_proof::NO_LANGUAGES, None)?
            .with_attachment_constraints(CatalogRecordSelection::All, None, |snapshot| {
                assert_eq!(
                    snapshot.rows().len(),
                    3,
                    "all original repeated attachment components survive"
                );
                assert!(
                    snapshot.rows().iter().all(|row| row
                        .owner_record_index()
                        .origin()
                        .contains_encrypted_source()
                        == marker),
                    "physical attachment ownership uses the actual DCB origin"
                );
                attachment(&snapshot, &expected, None)
            })??;
        let (_folder, archive) = common::open(&bounds::complete(), origin)?;
        let expected = catalog(&archive, CatalogPublicationMode::DiagnosticPartial)?;
        full_proof::open(&archive, full_proof::NO_LANGUAGES, None)?.with_item_bounds(
            CatalogRecordSelection::All,
            None,
            |snapshot| bounds_rows(&snapshot, &expected, None),
        )??;
    }
    Ok(())
}

#[test]
fn existing_inventory_states_keep_typed_bounds_and_exact_raw_numbers() -> TestResult {
    for database in [entities::database(), entities::wrong_dimension_type()] {
        let (_folder, archive) = common::open(&database, DcbOrigin::Encrypted)?;
        let expected = catalog(&archive, CatalogPublicationMode::DiagnosticPartial)?;
        let session = full_proof::open(
            &archive,
            CatalogLanguageSelection::Selected(&[Language::English, Language::German]),
            None,
        )?;
        let parsed = session.parsed(None)?;
        parsed.with_item_bounds(CatalogRecordSelection::All, None, |snapshot| {
            bounds_rows(&snapshot, &expected, None)
        })??;
    }
    Ok(())
}

#[test]
fn clean_normal_attachment_output_and_session_reader_share_the_same_cells() -> TestResult {
    let (_folder, archive) =
        common::open(&super::database(super::Layout::Complete), DcbOrigin::Plain)?;
    let expected = catalog(&archive, CatalogPublicationMode::Normal)?;
    let session = full_proof::open(&archive, CatalogLanguageSelection::AllAvailable, None)?;
    session.with_attachment_constraints(CatalogRecordSelection::All, None, |snapshot| {
        attachment(&snapshot, &expected, None)
    })??;
    assert!(
        expected
            .pointer("/exportHealth/status")
            .is_some_and(|state| state == "complete"),
        "the positive counterpart is a normal complete export"
    );
    Ok(())
}

pub(super) fn attachment(
    snapshot: &AttachmentConstraintSnapshot<'_>,
    expected: &Value,
    physical: Option<&[u32]>,
) -> TestResult {
    metadata(snapshot, expected, ATTACHMENTS)?;
    let rows = selected_rows(expected, ATTACHMENTS, physical)?;
    assert_eq!(
        snapshot.rows().len(),
        rows.len(),
        "all selected physical attachment components survive"
    );
    for (row, expected) in snapshot.rows().iter().zip(rows) {
        attachment_walk::row(row, snapshot.source_inventory(), expected)?;
    }
    Ok(())
}

pub(super) fn bounds_rows(
    snapshot: &ItemBoundsSnapshot<'_>,
    expected: &Value,
    physical: Option<&[u32]>,
) -> TestResult {
    metadata(snapshot, expected, BOUNDS)?;
    let rows = selected_rows(expected, BOUNDS, physical)?;
    assert_eq!(
        snapshot.rows().len(),
        rows.len(),
        "all selected physical bounds components survive"
    );
    for (row, expected) in snapshot.rows().iter().zip(rows) {
        bounds_walk::row(row, snapshot.source_inventory(), expected)?;
    }
    Ok(())
}

fn metadata<Row>(
    snapshot: &EquipmentSnapshot<'_, Row>,
    expected: &Value,
    table: &str,
) -> TestResult {
    Wire::new(
        expected
            .pointer(table)
            .ok_or("canonical configuration dataset")?,
        snapshot.source_inventory(),
    )
    .field("availability")?
    .raw(&snapshot.availability())?;
    assert!(
        snapshot.build().is_none()
            && expected
                .pointer("/source/build")
                .is_some_and(Value::is_null),
        "the synthetic Build is independently absent"
    );
    assert!(
        snapshot.public_names().is_empty() && snapshot.game_texts().is_empty(),
        "configuration queries register no localized text values"
    );
    assert!(
        snapshot.requested_languages().is_empty() && snapshot.available_languages().is_empty(),
        "unused session languages are not the configuration request scope"
    );
    assert!(
        snapshot.localization_notes().is_empty(),
        "configuration queries do not decode locale bodies"
    );
    assert!(
        snapshot.coverage().unreadable_headers.is_empty(),
        "all original fixture headers are readable"
    );
    assert!(
        snapshot.coverage().unreadable_bodies.is_empty(),
        "all original fixture bodies are readable"
    );
    assert_eq!(
        snapshot.coverage().skipped_leftovers,
        0,
        "the fixture declares no development leftovers"
    );
    Ok(())
}
