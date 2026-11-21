// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, DatasetAvailability,
    HaulingTargetBinding, LocationEntityDeclaration, LocationEntityDeclarationSnapshot,
    LocationEntityKind, ParsedCatalogQueries, SourceArenaOutput, SourcePayload,
    SourceRecordReference, SourceValueRef, with_location_entity_declarations,
};
use p4kx_export::{CatalogPublicationMode, Filter, build_catalog};
use p4kx_extraction::{Language, SourceValueState};
use serde_json::Value;

use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult, at, guid};

#[path = "location_entity_fixture.rs"]
mod fixture;

use fixture::{DUPLICATE, GENERATOR, MISSING, database, hauling_database};

const TABLE: &str = "/definitions/locationEntityDeclarations";

#[test]
fn location_entity_declarations_match_catalog_and_preserve_source_slots() -> TestResult {
    let (_folder, archive) = common::open(&database(), DcbOrigin::Plain)?;
    let mut options = common::options();
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    let catalog: Value = serde_json::from_slice(build_catalog(&archive, &options)?.json())?;
    let expected = at(&catalog, &format!("{TABLE}/rows"))?;
    let parsed = CatalogQuerySession::open(&archive, CatalogLanguageSelection::AllAvailable, None)?;
    parsed.with_location_entity_declarations(
        CatalogRecordSelection::All,
        None,
        None,
        |snapshot| {
            assert!(
                matches!(snapshot.availability, DatasetAvailability::Available),
                "the declared source family is available"
            );
            assert!(
                snapshot.coverage.incomplete_fields.is_empty(),
                "typed fields did not lose source coverage"
            );
            assert_eq!(
                &serde_json::to_value(&snapshot.rows)?,
                expected,
                "the selected SDK rows match the catalog projection"
            );
            let [static_row, unknown, delivery] = snapshot.rows.as_slice() else {
                return Err("three independently retained physical rows".into());
            };
            verify_duplicate_owners(static_row, unknown)?;
            verify_tags(delivery)?;
            fixture::verify_authored_tag_field(delivery, &snapshot.source_inventory)?;
            verify_delivery(delivery, &snapshot)
        },
    )??;
    let schema = jsonschema::validator_for(&p4kx_export::catalog_schema())?;
    if let Err(error) = schema.validate(&catalog) {
        return Err(format!("{}: {error}", error.instance_path()).into());
    }
    Ok(())
}

fn verify_duplicate_owners(
    static_row: &LocationEntityDeclaration<'_>,
    other: &LocationEntityDeclaration<'_>,
) -> TestResult {
    let static_type = static_row.entity_type().value().ok_or("static type")?;
    let other_type = other.entity_type().value().ok_or("additional type")?;
    assert!(
        matches!(static_type.kind(), LocationEntityKind::StaticEntity),
        "the static source variant is typed as static"
    );
    assert!(
        matches!(other_type.kind(), LocationEntityKind::Original),
        "the future variant retains its original shape"
    );
    assert!(
        serde_json::to_string(other_type.additional_fields())?.contains("unmodeled"),
        "the future field remains in its source arena"
    );
    assert_eq!(
        static_row.source().guid().value(),
        other.source().guid().value(),
        "duplicate declarations share the original GUID"
    );
    assert_ne!(
        static_row.source().record_index().value(),
        other.source().record_index().value(),
        "duplicate declarations retain distinct physical owners"
    );
    assert!(
        static_row
            .entity_tags()
            .value()
            .ok_or("static tags")?
            .tags()
            .value()
            .ok_or("authored tag list")?
            .is_empty(),
        "the authored empty tag list is distinct from absence"
    );
    Ok(())
}

fn verify_tags(row: &LocationEntityDeclaration<'_>) -> TestResult {
    let tags = row.entity_tags().value().ok_or("tag block")?;
    let [first, null, repeated, dangling] = tags.tags().value().ok_or("tag values")?.as_slice()
    else {
        return Err("original tag array length".into());
    };
    let first_guid = first.value().ok_or("tag link")?.guid();
    let repeated_guid = repeated.value().ok_or("repeated tag link")?.guid();
    let missing = dangling.value().ok_or("dangling tag")?;
    assert_eq!(
        null.state(),
        SourceValueState::Null,
        "the authored null tag slot keeps its position"
    );
    assert_eq!(
        first_guid.value(),
        repeated_guid.value(),
        "repeated tags keep the same authored target"
    );
    assert_ne!(
        first.origin().source_uses_refs(),
        repeated.origin().source_uses_refs(),
        "repeated tag occurrences have independent origins"
    );
    assert_eq!(
        missing.guid().value().ok_or("original id")?.to_string(),
        guid(MISSING),
        "a dangling tag retains its authored GUID"
    );
    assert!(
        missing.resolved_index().is_none(),
        "a dangling tag does not acquire a physical target"
    );
    Ok(())
}

fn verify_delivery(
    delivery: &LocationEntityDeclaration<'_>,
    snapshot: &LocationEntityDeclarationSnapshot<'_>,
) -> TestResult {
    let slot = delivery.resource_slot().value().ok_or("resource slot")?;
    assert!(
        slot.resolved_index().is_some(),
        "a compatible resource slot is joined"
    );
    let ty = delivery.entity_type().value().ok_or("delivery type")?;
    let LocationEntityKind::DeliveryTarget {
        delivery_objective_type,
        target_description,
    } = ty.kind()
    else {
        return Err("delivery type".into());
    };
    assert_eq!(
        delivery_objective_type.value(),
        Some(&"FixtureDelivery"),
        "the delivery enum retains its authored value"
    );
    let text = snapshot
        .game_texts
        .iter()
        .find(|text| text.id().value() == target_description.value())
        .ok_or("protected delivery text")?;
    let row = serde_json::to_value(delivery)?;
    assert_eq!(
        at(
            &row,
            "/type/value/targetDescription/containsEncryptedSource"
        )?,
        false,
        "the description reference is sourced from plain DataForge"
    );
    let translations = text.translations().value().ok_or("translated text")?;
    let english = translations
        .get(Language::English.code())
        .ok_or("English translation")?;
    assert_eq!(
        english
            .source_text
            .value()
            .ok_or("original English text")?
            .as_ref(),
        "Resource English",
        "the English translation retains its original text"
    );
    assert_eq!(
        at(
            &serde_json::to_value(&english.source_text)?,
            "/containsEncryptedSource"
        )?,
        true,
        "the translation is sourced separately from encrypted localization"
    );
    assert!(
        snapshot.available_languages.contains(&Language::English),
        "the selected English localization is reported as readable"
    );
    Ok(())
}

#[test]
fn location_entity_sdk_selects_duplicates_and_keeps_empty_filtered_availability() -> TestResult {
    let (_folder, archive) = common::open(&database(), DcbOrigin::Encrypted)?;
    let duplicate = CatalogRecordSelection::Guid(guid(DUPLICATE).parse()?);
    let filter = Filter::new("fixture_future");
    let chosen = with_location_entity_declarations(
        &archive,
        duplicate,
        Some(&filter),
        CatalogLanguageSelection::Selected(&[]),
        None,
        |snapshot| {
            let [row] = snapshot.rows.as_slice() else {
                return Err("one filtered duplicate");
            };
            assert!(
                matches!(
                    row.entity_type().value().ok_or("type")?.kind(),
                    LocationEntityKind::Original
                ),
                "the filter chooses the future variant of the duplicate"
            );
            let json = serde_json::to_value(row).map_err(|_| "serialize selected row")?;
            assert_eq!(
                at(&json, "/type/containsEncryptedSource").map_err(|_| "type origin")?,
                true,
                "the encrypted DataForge is the selected row source"
            );
            row.source()
                .record_index()
                .value()
                .copied()
                .ok_or("physical index")
        },
    )??;
    let unmatched = Filter::new("fixture_not_present");
    with_location_entity_declarations(
        &archive,
        CatalogRecordSelection::RecordIndex(chosen),
        Some(&unmatched),
        CatalogLanguageSelection::AllAvailable,
        None,
        |snapshot| {
            assert!(
                matches!(snapshot.availability, DatasetAvailability::Available),
                "an empty filtered selection still reports a declared family"
            );
            assert!(
                snapshot.rows.is_empty(),
                "the unmatched filter supplies no rows"
            );
            assert!(
                snapshot.available_languages.is_empty(),
                "a read with no rows does not load localization"
            );
        },
    )?;
    Ok(())
}

#[test]
fn hauling_override_bindings_join_original_arrays_to_location_entities() -> TestResult {
    let database = hauling_database();
    let (_folder, archive) = common::open(&database, DcbOrigin::Encrypted)?;
    let query = CatalogQuerySession::open(&archive, CatalogLanguageSelection::Selected(&[]), None)?;
    let parsed = query.parsed(None)?;
    parsed.with_contract_generators(
        CatalogRecordSelection::Guid(guid(GENERATOR).parse()?),
        None,
        |snapshot| -> TestResult {
            let [generator] = snapshot.rows.as_slice() else {
                return Err("one generator source owner".into());
            };
            let [contract, sub] = generator.hauling_target_bindings() else {
                return Err("contract and sub-contract target bindings".into());
            };
            let [first, _, repeated] = verify_hauling_arrays(generator.source(), contract, sub)?
            else {
                return Err("exact contract pickup slots".into());
            };
            let source = generator
                .source()
                .value(*first)
                .ok_or("pickup element")?
                .value();
            let Some(SourcePayload::RecordReference { value: reference }) = source.value() else {
                return Err("original typed location reference".into());
            };
            let duplicate = generator
                .source()
                .value(*repeated)
                .ok_or("repeat element")?
                .value();
            assert_ne!(
                source.origin().source_uses_refs(),
                duplicate.origin().source_uses_refs(),
                "repeated pickup targets retain separate source uses"
            );
            verify_target_selection(&parsed, reference)
        },
    )??;
    Ok(())
}

fn verify_hauling_arrays<'view>(
    arena: &'view SourceArenaOutput<'_>,
    contract: &HaulingTargetBinding,
    sub: &HaulingTargetBinding,
) -> TestResult<&'view [SourceValueRef]> {
    let pickup = array(arena, contract.pick_up_target_types())?;
    let [_, null, _] = pickup else {
        return Err("exact contract pickup slots".into());
    };
    let dropoff = array(arena, sub.drop_off_target_types())?;
    assert_eq!(
        pickup.len(),
        3,
        "the contract pickup array retains its three slots"
    );
    assert_eq!(
        dropoff.len(),
        1,
        "the sub-contract drop-off array retains its single slot"
    );
    assert!(
        array(arena, sub.pick_up_target_types())?.is_empty(),
        "the sub-contract pickup array remains empty"
    );
    assert!(
        array(arena, contract.drop_off_target_types())?.is_empty(),
        "the contract drop-off array remains empty"
    );
    assert_eq!(
        arena.value(*null).ok_or("null array slot")?.value().state(),
        SourceValueState::Null,
        "the authored null pickup target keeps its original position"
    );
    Ok(pickup)
}

fn verify_target_selection(
    parsed: &ParsedCatalogQueries<'_>,
    reference: &SourceRecordReference<'_>,
) -> TestResult {
    let index = reference.resolved_index().ok_or("physical target")?;
    parsed.with_location_entity_declarations(
        CatalogRecordSelection::RecordIndex(index),
        None,
        None,
        |locations| {
            assert!(
                locations.resolve(reference).is_some(),
                "same physical owner"
            );
        },
    )?;
    let filter = Filter::new("fictional_excluded_target");
    parsed.with_location_entity_declarations(
        CatalogRecordSelection::All,
        Some(&filter),
        None,
        |locations| {
            assert!(
                locations.resolve(reference).is_none(),
                "the filtered target row is excluded from the join"
            );
            assert!(
                reference.resolved_index().is_some(),
                "source resolution remains independent"
            );
        },
    )?;
    Ok(())
}

fn array<'view>(
    arena: &'view SourceArenaOutput<'_>,
    reference: Option<SourceValueRef>,
) -> TestResult<&'view [SourceValueRef]> {
    let node = arena
        .value(reference.ok_or("authored array binding")?)
        .ok_or("array cell")?;
    let Some(SourcePayload::Array { elements }) = node.value().value() else {
        return Err("original array".into());
    };
    Ok(elements)
}
