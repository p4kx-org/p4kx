// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogOptions, CatalogPublicationMode, build_catalog_from};
use p4kx_extraction::{DataForge, DataForgeError, Filter, Record, RecordTree, Value as Raw};
use p4kx_testkit::TestEntry;
use serde_json::{Value, json};

use super::{DcbOrigin, TestResult, body, catalog, fixture, origins, proof, source};
use crate::commodity_v3_fixture as common;
use crate::health_origin_fixture::{self as archive_fixture, Fixture, Input};

const ROWS: &str = "/projections/weaponMagazineResolutions/rows";
const RECORD_TABLE_WORDS: std::ops::Range<usize> = 4..8;
const HEADER_BYTES: usize = 120;
const LEADING_ROW_BYTES: [usize; 4] = [16, 12, 8, 8];
const V8_RECORD_BYTES: usize = 36;
const V8_INSTANCE_INDEX: usize = 32;

#[test]
fn weapon_magazine_resolution_observes_the_legacy_own_and_linked_container_routes() -> TestResult {
    let builder = fixture::magazine_database(fixture::MagazineCase::Resolved);
    let bytes = builder.build();
    let forge = DataForge::open(&bytes)?;
    let direct = source::tree(&forge, source::DIRECT_INDEX)?;
    let personal = source::tree(&forge, source::PERSONAL_INDEX)?;
    let target = original_target(&forge, &personal)?;
    let target_tree = target.tree()?;
    let direct_container = original_container(&direct, 2)?;
    let linked_container = original_container(&target_tree, 1)?;
    source::primary(source::weapon(&forge, &direct)?.primary)?;

    let output = catalog(
        &builder,
        DcbOrigin::Encrypted,
        CatalogPublicationMode::Normal,
    )?;
    super::check_weapons(&output)?;
    let own = outcome(&output, direct.record_index(), "ownContainer")?;
    let own_ordinal = proof::present(own, "containerOrdinal", &json!(2), Some(true))?;
    origins::exact(
        &output,
        own_ordinal,
        &[body(&direct, direct_container, "$value", "conversion")?],
    )?;
    assert!(
        own.get("magazineRecordIndex").is_none(),
        "the direct route never consumes the external primary magazine"
    );
    let linked = outcome(&output, personal.record_index(), "linkedContainer")?;
    target_index(&output, linked, &forge, &personal, &target)?;
    let linked_ordinal = proof::present(linked, "containerOrdinal", &json!(1), Some(true))?;
    origins::exact(
        &output,
        linked_ordinal,
        &[body(
            &target_tree,
            linked_container,
            "$value",
            "conversion",
        )?],
    )?;
    proof::closure(&output)
}

#[test]
fn weapon_magazine_resolution_distinguishes_readable_target_without_container() -> TestResult {
    let builder = fixture::empty_primary_magazine_database();
    let bytes = builder.build();
    let forge = DataForge::open(&bytes)?;
    let personal = source::tree(&forge, source::PERSONAL_INDEX)?;
    let target = original_target(&forge, &personal)?;
    let tree = target.tree()?;
    let Some(Raw::Array(components)) = tree.field("Components") else {
        return Err("readable magazine Components".into());
    };
    let [attachment] = components.as_ref() else {
        return Err("only the actual attachment remains on the target".into());
    };
    assert_eq!(
        source::structure(attachment)?.type_name(),
        "SAttachableComponentParams",
        "the magazine body is readable and its only element is unrelated"
    );

    let output = catalog(
        &builder,
        DcbOrigin::Encrypted,
        CatalogPublicationMode::Normal,
    )?;
    no_legacy_ammo(&output)?;
    let missing = outcome(&output, personal.record_index(), "linkedWithoutContainer")?;
    target_index(&output, missing, &forge, &personal, &target)?;
    proof::closure(&output)
}

#[test]
fn weapon_magazine_resolution_keeps_the_physical_header_when_target_body_is_unreadable()
-> TestResult {
    let builder = fixture::magazine_database(fixture::MagazineCase::Resolved);
    let mut bytes = builder.build();
    let original = DataForge::open(&bytes)?;
    let original_target = original
        .record(source::PRIMARY_INDEX)
        .ok_or("intact primary magazine")?;
    original_container(&original_target.tree()?, 1)?;
    break_instance(&mut bytes, source::PRIMARY_INDEX)?;

    let forge = DataForge::open(&bytes)?;
    let personal = source::tree(&forge, source::PERSONAL_INDEX)?;
    let target = target_with_unreadable_instance(&forge, &personal)?;
    let filter = Filter::parse("*/fixture_personal")?;
    assert!(
        filter.matches(personal.record_path()),
        "the referring weapon is selected"
    );
    assert!(
        !filter.matches(target.record_path()?),
        "the unreadable magazine is demanded only by the real legacy link"
    );
    let output = diagnostic(&bytes, &filter)?;
    no_legacy_ammo(&output)?;
    let unreadable = outcome(&output, personal.record_index(), "linkedTargetUnreadable")?;
    target_index(&output, unreadable, &forge, &personal, &target)?;
    assert_eq!(
        proof::at(&output, "/exportHealth/status")?,
        "incomplete",
        "the actual attempted dependency remains visible to publication policy"
    );
    proof::closure(&output)
}

#[test]
fn weapon_magazine_resolution_retains_a_self_link_without_recursive_selection() -> TestResult {
    let builder = fixture::self_linked_magazine_database();
    let bytes = builder.build();
    let forge = DataForge::open(&bytes)?;
    let personal = source::tree(&forge, source::PERSONAL_INDEX)?;
    let original = source::weapon(&forge, &personal)?;
    let Raw::Reference(reference) = original.primary else {
        return Err("the authored self Reference".into());
    };
    let target = reference.original_target().record()?;
    assert_eq!(
        target.index(),
        personal.record_index(),
        "the actual physical owner links to itself"
    );
    assert_eq!(
        target.guid(),
        personal.guid(),
        "the stored self Reference retains its GUID"
    );
    let Some(Raw::Array(components)) = personal.field("Components") else {
        return Err("actual Components of the self target".into());
    };
    assert_eq!(
        components.len(),
        2,
        "the original has an attachment and the weapon with no container"
    );

    let output = catalog(
        &builder,
        DcbOrigin::Encrypted,
        CatalogPublicationMode::Normal,
    )?;
    no_legacy_ammo(&output)?;
    let linked = outcome(&output, personal.record_index(), "linkedWithoutContainer")?;
    target_index(&output, linked, &forge, &personal, &target)?;
    proof::closure(&output)
}

fn original_target<'source>(
    forge: &'source DataForge<'_>,
    personal: &RecordTree<'source>,
) -> TestResult<Record<'source>> {
    let original = source::weapon(forge, personal)?;
    source::primary(original.primary)?;
    let Raw::Reference(reference) = original.primary else {
        return Err("original primary magazine Reference".into());
    };
    let target = forge
        .record(reference.record_index())
        .ok_or("the physical header of the admitted magazine")?;
    assert_eq!(
        target.guid(),
        reference.guid(),
        "the target header has this original Reference's GUID"
    );
    assert_eq!(
        target.type_name()?,
        reference.type_name(),
        "the same physical header type"
    );
    Ok(target)
}

fn original_container<'read, 'source>(
    tree: &'read RecordTree<'source>,
    index: usize,
) -> TestResult<&'read Raw<'source>> {
    let Some(Raw::Array(values)) = tree.field("Components") else {
        return Err("actual source Components".into());
    };
    let selected = values
        .get(index)
        .ok_or("actual selected container occurrence")?;
    assert_eq!(
        source::structure(selected)?.type_name(),
        "SAmmoContainerComponentParams",
        "the requested source position contains the physical container"
    );
    Ok(selected)
}

fn target_with_unreadable_instance<'source>(
    forge: &'source DataForge<'_>,
    personal: &RecordTree<'source>,
) -> TestResult<Record<'source>> {
    let target = original_target(forge, personal)?;
    let failure = target
        .tree()
        .err()
        .ok_or("the isolated target body fails")?;
    let expected_index = usize::try_from(source::PRIMARY_INDEX)?;
    assert!(
        matches!(
            failure,
            DataForgeError::IndexOutOfRange { part: "Record", index, target: "instance", target_index }
                if index == expected_index && target_index == u64::from(u16::MAX)
        ),
        "the actual header remains valid and its original instance cannot be read: {failure}"
    );
    Ok(target)
}

fn diagnostic(bytes: &[u8], filter: &Filter) -> TestResult<Value> {
    let fixture = Fixture::open(
        bytes,
        Input::Encrypted,
        vec![TestEntry::stored(
            archive_fixture::REGISTRY,
            b"<LayerBackups/>",
        )],
    )?;
    let options = CatalogOptions {
        filter: Some(filter),
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..common::options()
    };
    let built = build_catalog_from(&fixture.archive, &fixture.entry, &options)?;
    Ok(serde_json::from_slice(built.json())?)
}

fn break_instance(bytes: &mut [u8], record_index: u32) -> TestResult {
    let (words, _) = bytes.as_chunks::<4>();
    assert_eq!(
        words.get(1).copied().map(u32::from_le_bytes),
        Some(8),
        "the fixture uses the original tagged row layout"
    );
    let counts = words
        .get(RECORD_TABLE_WORDS)
        .ok_or("four preceding original table counts")?;
    let first = counts
        .iter()
        .zip(LEADING_ROW_BYTES)
        .try_fold(HEADER_BYTES, |offset, (count, size)| {
            usize::try_from(u32::from_le_bytes(*count))
                .ok()?
                .checked_mul(size)?
                .checked_add(offset)
        })
        .ok_or("original table byte bounds")?;
    let start = usize::try_from(record_index)?
        .checked_mul(V8_RECORD_BYTES)
        .and_then(|offset| offset.checked_add(first))
        .and_then(|offset| offset.checked_add(V8_INSTANCE_INDEX))
        .ok_or("original Record instance index offset")?;
    let end = start
        .checked_add(size_of::<u16>())
        .ok_or("original instance index field bound")?;
    bytes
        .get_mut(start..end)
        .ok_or("original Record instance index bytes")?
        .copy_from_slice(&u16::MAX.to_le_bytes());
    Ok(())
}

fn outcome<'catalog>(
    catalog: &'catalog Value,
    owner: u32,
    expected: &str,
) -> TestResult<&'catalog Value> {
    let row = proof::rows(catalog, ROWS)?
        .iter()
        .find(|row| row.pointer("/ownerRecordIndex/value") == Some(&json!(owner)))
        .ok_or("observed legacy weapon resolution for this physical owner")?;
    proof::present(row, "componentOrdinal", &json!(1), Some(true))?;
    let outcome = proof::cell(row, "outcome", "present")?;
    let value = proof::at(outcome, "/value")?;
    assert_eq!(
        value.get("type"),
        Some(&json!(expected)),
        "the actual consulted route determines the outcome"
    );
    Ok(value)
}

fn target_index<'source>(
    catalog: &Value,
    outcome: &Value,
    forge: &DataForge<'source>,
    personal: &RecordTree<'source>,
    target: &Record<'source>,
) -> TestResult {
    let source = source::weapon(forge, personal)?;
    let selected = proof::present(
        outcome,
        "magazineRecordIndex",
        &json!(target.index()),
        Some(true),
    )?;
    origins::exact(
        catalog,
        selected,
        &[
            body(personal, source.primary, "$value", "selector")?,
            origins::header(target.index(), "recordIndex", "selector"),
        ],
    )
}

fn no_legacy_ammo(catalog: &Value) -> TestResult {
    let weapon = super::super::weapon(catalog, fixture::PERSONAL)?;
    for field in ["ammoCapacity", "ammo"] {
        assert_eq!(
            weapon.get(field),
            Some(&Value::Null),
            "{field} keeps its existing absent primary outcome"
        );
    }
    Ok(())
}
