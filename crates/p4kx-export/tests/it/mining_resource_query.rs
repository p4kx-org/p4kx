// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::sdk_full_proof as full_proof;
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogRecordSelection, CompositionPart, MineableCompositionSnapshot,
    MineableElementSnapshot, ResourceNumber, ResourceTypeSnapshot, with_mineable_compositions,
    with_mineable_elements, with_resource_types,
};
use p4kx_export::{CatalogPublicationMode, build_catalog};
use p4kx_extraction::{
    Archive, CancelToken, DataForgeEntry, Language, SourceInventory, SourcedValue,
};
use serde_json::Value;

use crate::canonical_header::json::canonical_value;

use super::mining_resource_query_archive as archive_fixture;
use super::mining_resource_query_fixture as fixture;
use super::resource_consumer_walk::{visit_leaf, visit_resource};
use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};

#[test]
fn direct_mining_and_resource_queries_preserve_canonical_rows_and_all_origin_states() -> TestResult
{
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
        parsed.with_mineable_elements(CatalogRecordSelection::All, None, |snapshot| {
            verify_elements(&snapshot, &catalog, expected)
        })??;
        parsed.with_mineable_compositions(CatalogRecordSelection::All, None, |snapshot| {
            verify_compositions(&snapshot, &catalog, expected)
        })??;
        parsed.with_resource_types(
            CatalogRecordSelection::All,
            None,
            |snapshot| -> TestResult {
                assert_eq!(
                    canonical_value(snapshot.rows())?,
                    *at(&catalog, "/definitions/resourceTypes/rows")?,
                    "Resource rows match the canonical Catalog"
                );
                verify_resource_texts(&snapshot, expected)?;
                let canonical = at(&catalog, "/definitions/resourceTypes/rows")?
                    .as_array()
                    .ok_or("canonical Resource array")?;
                for (row, expected) in snapshot.rows().iter().zip(canonical) {
                    visit_resource(row, snapshot.source_inventory(), expected)?;
                }
                Ok(())
            },
        )??;
    }
    Ok(())
}

fn verify_elements(
    snapshot: &MineableElementSnapshot<'_>,
    catalog: &Value,
    expected: Option<bool>,
) -> TestResult {
    assert_eq!(
        canonical_value(snapshot.rows())?,
        *at(catalog, "/mineableElements")?,
        "Element rows match the canonical Catalog"
    );
    for row in snapshot.rows() {
        assert_eq!(
            row.instability.origin().contains_encrypted_source(),
            expected,
            "Tuning retains its original encryption state"
        );
        assert_eq!(
            row.instability.value().ok_or("instability")?.raw_bits(),
            fixture::INSTABILITY_BITS,
            "NaN payload bits are retained"
        );
        assert_eq!(
            row.instability.value().ok_or("instability")?.finite(),
            None,
            "NaN has no finite projection"
        );
        assert_eq!(
            row.resistance.value().ok_or("resistance")?.raw_bits(),
            (-0.0_f32).to_bits(),
            "Negative zero bits are retained"
        );
        let cells = [
            &row.instability,
            &row.resistance,
            &row.optimal_window_midpoint,
            &row.optimal_window_midpoint_randomness,
            &row.optimal_window_thinness,
            &row.explosion_multiplier,
            &row.cluster_factor,
        ];
        for cell in cells {
            assert_cell(cell, snapshot.source_inventory())?;
        }
        assert_cell(&row.resource_type, snapshot.source_inventory())?;
    }
    assert_eq!(
        snapshot.available_languages().len(),
        0,
        "Elements read no locale payloads"
    );
    assert_eq!(
        snapshot.dependency_coverage().unreadable_bodies.len(),
        0,
        "Elements demand no group bodies"
    );
    Ok(())
}

fn verify_compositions(
    snapshot: &MineableCompositionSnapshot<'_>,
    catalog: &Value,
    expected: Option<bool>,
) -> TestResult {
    assert_eq!(
        canonical_value(snapshot.rows())?,
        *at(catalog, "/mineableCompositions")?,
        "Composition rows match the canonical Catalog"
    );
    let [row] = snapshot.rows() else {
        return Err("one composition".into());
    };
    let text = snapshot
        .game_text(row.deposit_name.value().ok_or("deposit text identity")?)
        .ok_or("protected text row")?;
    assert_eq!(
        text.key().value().copied(),
        Some(fixture::DEPOSIT_KEY),
        "Deposit labels retain their protected key"
    );
    assert!(
        matches!(
            row.minimum_distinct_elements.value(),
            Some(ResourceNumber::UInt16 { value: 2 })
        ),
        "Original integer width is preserved"
    );
    assert_eq!(
        row.minimum_distinct_elements
            .origin()
            .contains_encrypted_source(),
        expected,
        "Composition tuning retains its original origin"
    );
    let parts = row.parts.value().ok_or("composition slots")?;
    assert_eq!(parts.len(), 3, "Repeated slots retain original positions");
    for part in parts {
        assert_cell(part, snapshot.source_inventory())?;
        verify_part(
            part.value().ok_or("original part")?,
            snapshot.source_inventory(),
        )?;
    }
    assert_cell(&row.parts, snapshot.source_inventory())?;
    assert_cell(&row.deposit_name, snapshot.source_inventory())?;
    assert_eq!(
        snapshot.public_names().len(),
        0,
        "Deposit labels stay in the protected domain"
    );
    Ok(())
}

fn verify_part(part: &CompositionPart, inventory: &SourceInventory) -> TestResult {
    assert!(
        matches!(
            part.min_percentage.value(),
            Some(ResourceNumber::UInt16 { .. })
        ),
        "Minimum retains its unsigned source width"
    );
    assert!(
        matches!(
            part.max_percentage.value(),
            Some(ResourceNumber::Int32 { .. })
        ),
        "Maximum retains its signed source width"
    );
    assert!(
        matches!(part.probability.value(), Some(ResourceNumber::Single(_))),
        "Probability retains its Single source kind"
    );
    let Some(ResourceNumber::Double(curve)) = part.curve_exponent.value() else {
        return Err("source Double curve".into());
    };
    assert_eq!(
        curve.raw_bits(),
        (-0.0_f64).to_bits(),
        "Double negative zero bits are retained"
    );
    let Some(ResourceNumber::Int64 { value: scale }) = part.quality_scale.value() else {
        return Err("source Int64 scale".into());
    };
    assert_eq!(
        scale.get(),
        fixture::WIDE_QUALITY_SCALE,
        "Wide integers retain every original digit"
    );
    for cell in [
        &part.min_percentage,
        &part.max_percentage,
        &part.probability,
        &part.curve_exponent,
        &part.quality_scale,
    ] {
        assert_cell(cell, inventory)?;
    }
    assert_cell(&part.mineable_element, inventory)?;
    Ok(())
}

fn at<'value>(value: &'value Value, pointer: &str) -> TestResult<&'value Value> {
    value
        .pointer(pointer)
        .ok_or_else(|| format!("actual canonical field {pointer}").into())
}

fn assert_cell<T>(cell: &SourcedValue<T>, inventory: &SourceInventory) -> TestResult {
    assert_ne!(
        cell.origin().source_uses_refs().len(),
        0,
        "Every checked datum has original evidence"
    );
    visit_leaf(cell, inventory)
}

fn verify_resource_texts(
    snapshot: &ResourceTypeSnapshot<'_>,
    expected: Option<bool>,
) -> TestResult {
    let row = snapshot.rows().first().ok_or("resource row")?;
    let name = snapshot
        .public_name(row.name().value().ok_or("public name identity")?)
        .ok_or("public name row")?;
    let description = snapshot
        .game_text(row.description().value().ok_or("protected identity")?)
        .ok_or("protected row")?;
    assert_eq!(
        name.key().value().copied(),
        Some("@fixture_resource_name"),
        "Names retain their public source key"
    );
    assert_eq!(
        description.key().value().copied(),
        Some("@fixture_resource_description"),
        "Descriptions retain their protected source key"
    );
    assert_eq!(
        name.key().origin().contains_encrypted_source(),
        expected,
        "The key retains its original encryption state"
    );
    let translations = name.translations().value().ok_or("name translations")?;
    let english = translations.get("en").ok_or("English name")?;
    let german = translations.get("de").ok_or("German name")?;
    assert_eq!(
        english.source_text.origin().contains_encrypted_source(),
        Some(true),
        "Encrypted English contributes to translated text"
    );
    assert_eq!(
        german.source_text.origin().contains_encrypted_source(),
        expected,
        "German retains the original key contribution"
    );
    assert_eq!(
        snapshot.available_languages(),
        [Language::English, Language::German],
        "Only present requested locales are opened"
    );
    assert_cell(&english.source_text, snapshot.source_inventory())?;
    assert_cell(description.key(), snapshot.source_inventory())?;
    Ok(())
}

#[test]
fn selected_definition_queries_leave_existing_locked_world_and_unused_locales_unread() -> TestResult
{
    let (_folder, archive) = archive_fixture::open(&fixture::database(), DcbOrigin::Plain, false)?;
    let before = archive.key_demand().snapshot().unread();
    let selected = CatalogRecordSelection::Guid(common::guid(fixture::ELEMENT_ALPHA).parse()?);
    with_mineable_elements(&archive, selected, None, |snapshot| {
        assert_eq!(snapshot.rows().len(), 1, "One selected element is returned");
    })?;
    with_mineable_compositions(
        &archive,
        CatalogRecordSelection::All,
        CatalogLanguageSelection::Selected(&[]),
        None,
        |snapshot| {
            assert_eq!(
                snapshot.rows().len(),
                1,
                "The composition is read without world traversal"
            );
        },
    )?;
    with_resource_types(
        &archive,
        CatalogRecordSelection::All,
        CatalogLanguageSelection::Selected(&[]),
        None,
        |snapshot| {
            assert_eq!(
                snapshot.rows().len(),
                3,
                "All three resource owners are read"
            );
        },
    )?;
    assert_eq!(
        archive.key_demand().snapshot().unread(),
        before,
        "Unneeded encrypted payloads stay unopened"
    );
    verify_foreign_family(&archive)?;
    Ok(())
}

fn verify_foreign_family(archive: &Archive) -> TestResult {
    let before = archive.key_demand().snapshot().unread();
    let unrelated = CatalogRecordSelection::Guid(common::guid(fixture::ELEMENT_ALPHA).parse()?);
    with_resource_types(
        archive,
        unrelated,
        CatalogLanguageSelection::Selected(&[Language::English]),
        None,
        |snapshot| {
            assert_eq!(
                snapshot.rows().len(),
                0,
                "Foreign family selection yields no resource rows"
            );
            assert_eq!(
                snapshot.available_languages().len(),
                0,
                "No usable owners means no locale reads"
            );
        },
    )?;
    assert_eq!(
        archive.key_demand().snapshot().unread(),
        before,
        "Foreign selection leaves encrypted inputs unread"
    );
    Ok(())
}

#[test]
fn cancelled_queries_do_not_call_consumer_callbacks() -> TestResult {
    let (_folder, archive) = archive_fixture::open(&fixture::database(), DcbOrigin::Plain, false)?;
    let cancel = CancelToken::new();
    cancel.cancel();
    let called = std::cell::Cell::new(false);
    let result = with_resource_types(
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
    assert!(!called.get(), "Cancelled queries never call consumers");
    Ok(())
}
