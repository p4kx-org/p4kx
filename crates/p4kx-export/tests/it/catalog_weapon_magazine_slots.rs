// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogPublicationMode, build_catalog};
use p4kx_extraction::{DataForge, RecordTree, Value as Raw, ValueError};
use p4kx_testkit::DataForgeBuilder;
use serde_json::{Value, json};

use super::{check_weapons, fixture};
use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::health_origin_assertions as proof;
use crate::health_reference_origins as origins;

#[path = "catalog_weapon_magazine_condition_assertions.rs"]
mod condition;
#[path = "catalog_weapon_magazine_condition_numbers.rs"]
mod numbers;
#[path = "catalog_weapon_magazine_slots_source.rs"]
mod source;

const ROWS: &str = "/declarations/ammoBindings/rows";
const IGNORE: &str = "shouldIgnorePrimaryAmmoContainer";

#[test]
fn weapon_magazine_slots_preserve_order_duplicate_targets_conditions_and_raw_ignore() -> TestResult
{
    let builder = fixture::magazine_database(fixture::MagazineCase::Resolved);
    let bytes = builder.build();
    let forge = DataForge::open(&bytes)?;
    let direct_tree = source::tree(&forge, source::DIRECT_INDEX)?;
    let personal_tree = source::tree(&forge, source::PERSONAL_INDEX)?;
    let direct = source::weapon(&forge, &direct_tree)?;
    let personal = source::weapon(&forge, &personal_tree)?;
    source::primary(direct.primary)?;
    source::primary(personal.primary)?;
    assert!(
        matches!(direct.secondary, Some([])),
        "the direct weapon stores an empty secondary array"
    );
    assert!(
        matches!(direct.ignore_primary, Some(Raw::Bool(true))),
        "the direct flag is stored true"
    );
    assert!(
        matches!(personal.ignore_primary, Some(Raw::Bool(false))),
        "the personal flag is stored false"
    );
    let original_slots = source::slots(&forge, &personal, fixture::MagazineCase::Resolved)?;

    for (origin, marker) in [(DcbOrigin::Plain, false), (DcbOrigin::Encrypted, true)] {
        let catalog = catalog(&builder, origin, CatalogPublicationMode::Normal)?;
        let [direct_row, personal_row] = rows(&catalog)?;
        for (tree, row, original) in [
            (&direct_tree, direct_row, &direct),
            (&personal_tree, personal_row, &personal),
        ] {
            identity(&catalog, row, tree, original.component, marker)?;
            flag(
                &catalog,
                row,
                tree,
                original.component,
                original.ignore_primary.ok_or("stored flag")?,
                marker,
            )?;
            link(&catalog, row, tree, original.primary, "present", marker)?;
        }
        proof::present(
            direct_row,
            "secondaryAmmoContainers",
            &json!([]),
            Some(marker),
        )?;
        check_slots(
            &catalog,
            personal_row,
            &personal_tree,
            &original_slots,
            ["present"; 3],
            marker,
        )?;
        check_weapons(&catalog)?;
        proof::closure(&catalog)?;
    }
    Ok(())
}

#[test]
fn weapon_magazine_slots_preserve_dangling_wrong_type_and_null_in_original_order() -> TestResult {
    let builder = fixture::magazine_database(fixture::MagazineCase::BadLinks);
    let bytes = builder.build();
    let forge = DataForge::open(&bytes)?;
    let tree = source::tree(&forge, source::PERSONAL_INDEX)?;
    let weapon = source::weapon(&forge, &tree)?;
    let original = source::slots(&forge, &weapon, fixture::MagazineCase::BadLinks)?;

    let catalog = catalog(
        &builder,
        DcbOrigin::Encrypted,
        CatalogPublicationMode::DiagnosticPartial,
    )?;
    let [_, row] = rows(&catalog)?;
    check_slots(
        &catalog,
        row,
        &tree,
        &original,
        ["dangling", "unexpectedTargetType", "null"],
        true,
    )?;
    proof::closure(&catalog)
}

#[test]
fn weapon_magazine_slots_distinguish_absent_declarations_from_stored_empty_and_null() -> TestResult
{
    let builder = fixture::database(0.0);
    let bytes = builder.build();
    let forge = DataForge::open(&bytes)?;
    let tree = source::tree(&forge, source::DIRECT_INDEX)?;
    let weapon = source::weapon(&forge, &tree)?;
    assert!(
        weapon.secondary.is_none(),
        "the old original has no secondary declaration"
    );
    assert!(
        weapon.ignore_primary.is_none(),
        "the old original has no raw ignore declaration"
    );
    assert!(
        matches!(weapon.primary, Raw::Null),
        "primary is declared but physically null"
    );

    let catalog = catalog(&builder, DcbOrigin::Plain, CatalogPublicationMode::Normal)?;
    let [row, _] = rows(&catalog)?;
    proof::cell(row, "secondaryAmmoContainers", "absent")?;
    proof::cell(row, IGNORE, "absent")?;
    link(&catalog, row, &tree, weapon.primary, "null", false)?;
    proof::closure(&catalog)
}

fn catalog(
    builder: &DataForgeBuilder,
    origin: DcbOrigin,
    mode: CatalogPublicationMode,
) -> TestResult<Value> {
    let (_folder, archive) = common::open(builder, origin)?;
    let mut options = common::options();
    options.publication_mode = mode;
    Ok(serde_json::from_slice(
        build_catalog(&archive, &options)?.json(),
    )?)
}

fn rows(catalog: &Value) -> TestResult<[&Value; 2]> {
    let rows = proof::rows(catalog, ROWS)?;
    let weapons = rows
        .iter()
        .filter(|row| row.get("type").and_then(Value::as_str) == Some("weaponMagazineSlots"))
        .map(|row| proof::at(row, "/value"))
        .collect::<TestResult<Vec<_>>>()?;
    weapons
        .try_into()
        .map_err(|_| "expected two physical weapon components in original order".into())
}

fn identity<'source>(
    catalog: &Value,
    row: &Value,
    tree: &RecordTree<'source>,
    component: &Raw<'source>,
    marker: bool,
) -> TestResult {
    let index = tree.record_index();
    let guid = proof::present(
        row,
        "ownerGuid",
        &json!(tree.guid().to_string()),
        Some(marker),
    )?;
    origins::exact(catalog, guid, &[origins::header(index, "guid", "payload")])?;
    let owner = proof::present(row, "ownerRecordIndex", &json!(index), Some(marker))?;
    origins::exact(
        catalog,
        owner,
        &[origins::header(index, "recordIndex", "selector")],
    )?;
    let ordinal = proof::present(row, "componentOrdinal", &json!(1), Some(marker))?;
    origins::exact(
        catalog,
        ordinal,
        &[body(tree, component, "$value", "conversion")?],
    )
}

fn flag<'source>(
    catalog: &Value,
    row: &Value,
    tree: &RecordTree<'source>,
    component: &Raw<'source>,
    original: &Raw<'source>,
    marker: bool,
) -> TestResult {
    let Raw::Bool(value) = original else {
        return Err("original ignore flag".into());
    };
    let cell = proof::present(row, IGNORE, &json!(value), Some(marker))?;
    origins::exact(
        catalog,
        cell,
        &[
            body(tree, original, "$value", "payload")?,
            body(tree, original, "$fieldName", "selector")?,
            body(tree, component, "$type", "selector")?,
        ],
    )
}

fn check_slots<'source>(
    catalog: &Value,
    row: &Value,
    tree: &RecordTree<'source>,
    originals: &[source::Slot<'_, 'source>; 3],
    statuses: [&str; 3],
    marker: bool,
) -> TestResult {
    let array = proof::cell(row, "secondaryAmmoContainers", "present")?;
    assert_eq!(
        array.get("containsEncryptedSource"),
        Some(&json!(marker)),
        "the actual ordered array comes from the same Entry"
    );
    let slots = proof::rows(array, "/value")?;
    assert_eq!(
        slots.len(),
        originals.len(),
        "ordered repeated slots are never deduplicated"
    );
    for ((slot, original), status) in slots.iter().zip(originals).zip(statuses) {
        assert_eq!(
            slot.get("state"),
            Some(&json!("present")),
            "each original Class occurrence stays present"
        );
        let value = proof::at(slot, "/value")?;
        link(catalog, value, tree, original.magazine, status, marker)?;
        condition::check(catalog, value, tree, original.condition, marker)?;
    }
    Ok(())
}

fn link<'source>(
    catalog: &Value,
    row: &Value,
    tree: &RecordTree<'source>,
    original: &Raw<'source>,
    status: &str,
    marker: bool,
) -> TestResult {
    if matches!(original, Raw::Null) {
        proof::cell(row, "ammoContainerRecord", "null")?;
        return Ok(());
    }
    let carrier = proof::cell(row, "ammoContainerRecord", "present")?;
    let payload = proof::at(carrier, "/value")?;
    assert_eq!(
        payload.get("status"),
        Some(&json!(status)),
        "the selected original Reference result retains its status"
    );
    match original {
        Raw::Reference(target) => {
            target_identity(catalog, payload, tree, original, target, marker)?;
        }
        Raw::Error(ValueError::DanglingReference { guid }) => {
            let stored = proof::present(payload, "guid", &json!(guid.to_string()), Some(marker))?;
            origins::exact(
                catalog,
                stored,
                &[body(tree, original, "$error.guid", "payload")?],
            )?;
            proof::cell(payload, "targetRecordIndex", "absent")?;
        }
        _ => return Err("original magazine reference".into()),
    }
    Ok(())
}

fn target_identity<'source>(
    catalog: &Value,
    payload: &Value,
    tree: &RecordTree<'source>,
    original: &Raw<'source>,
    target: &p4kx_extraction::Reference<'source>,
    marker: bool,
) -> TestResult {
    let guid = proof::present(
        payload,
        "guid",
        &json!(target.guid().to_string()),
        Some(marker),
    )?;
    origins::exact(catalog, guid, &[body(tree, original, "$value", "payload")?])?;
    let target_index = proof::present(
        payload,
        "targetRecordIndex",
        &json!(target.record_index()),
        Some(marker),
    )?;
    origins::exact(
        catalog,
        target_index,
        &[
            body(tree, original, "$value", "selector")?,
            origins::header(target.record_index(), "recordIndex", "selector"),
        ],
    )?;
    proof::present(
        payload,
        "targetType",
        &json!(target.type_name()),
        Some(marker),
    )?;
    Ok(())
}

fn body<'source>(
    tree: &RecordTree<'source>,
    value: &Raw<'source>,
    field: &str,
    role: &str,
) -> TestResult<(String, Vec<String>, String, String)> {
    Ok((
        "dataforgeValue".to_owned(),
        vec![
            tree.record_index().to_string(),
            source::ordinal(tree, value)?.to_string(),
        ],
        field.to_owned(),
        role.to_owned(),
    ))
}
