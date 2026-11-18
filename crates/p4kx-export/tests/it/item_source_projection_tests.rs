// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::equipment;
use p4kx_export::catalog_queries::{CatalogLanguageSelection, CatalogRecordSelection};
use p4kx_export::{CatalogPublicationMode, build_catalog};
use p4kx_extraction::{DataForgeEntry, Language};
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::health_origin_assertions as proof;

#[path = "item_source_projection_fixture.rs"]
mod fixture;
use fixture::Layout;

#[test]
fn item_projection_links_original_families_and_sources_each_classification() -> TestResult {
    for (origin, marker) in origins() {
        let catalog = catalog(Layout::Normal, origin, true, false, complete_mode(origin))?;
        let row = only_item(&catalog)?;
        header_and_selection(row, marker, 1, 3, 4)?;
        proof::present(row, "itemClass", &json!("Military"), Some(true))?;
        let playability = proof::cell(row, "playability", "absent")?;
        assert_eq!(
            playability["containsEncryptedSource"],
            json!(marker),
            "key-only playability has no English contributor"
        );
        assert!(
            row["variantOf"].is_null(),
            "the unchanged singleton variant choice is null"
        );
        original_attachment(&catalog, 0, 1, marker, false)?;
        proof::closure(&catalog)?;
    }
    Ok(())
}

#[test]
fn foreign_attachment_fields_remain_complete_without_constraint_admission() -> TestResult {
    for (origin, marker) in origins() {
        let catalog = catalog(Layout::Foreign, origin, false, false, complete_mode(origin))?;
        let row = only_item(&catalog)?;
        header_and_selection(row, marker, 1, 3, 4)?;
        original_attachment(&catalog, 0, 1, marker, true)?;
        assert!(
            proof::rows(&catalog, "/configurations/itemAttachmentConstraints/rows")?.is_empty(),
            "foreign source targets do not enter SItemDefinition constraints"
        );
        proof::closure(&catalog)?;
    }
    Ok(())
}

#[test]
fn first_null_or_unreadable_definition_never_falls_forward_to_later_values() -> TestResult {
    for (layout, state, mode) in [
        (Layout::NullFirst, "null", CatalogPublicationMode::Normal),
        (
            Layout::UnreadableFirst,
            "unreadable",
            CatalogPublicationMode::DiagnosticPartial,
        ),
    ] {
        let catalog = catalog(layout, DcbOrigin::Encrypted, false, false, mode)?;
        let row = only_item(&catalog)?;
        proof::present(
            &row["selection"],
            "attachmentComponentOrdinal",
            &json!(1),
            Some(true),
        )?;
        let first = component(&catalog, 0, 1)?;
        proof::cell(first, "value", state)?;
        let later = component(&catalog, 0, 2)?;
        assert_eq!(
            later.pointer("/value/value/itemType/value"),
            Some(&json!("FixtureType")),
            "an independently readable later original exists"
        );
        assert!(
            row.pointer("/itemClass/value").is_some_and(Value::is_null),
            "classification never uses that later source"
        );
        proof::closure(&catalog)?;
    }
    Ok(())
}

#[test]
fn original_unknown_prefix_keeps_known_values_and_honest_selection_uncertainty() -> TestResult {
    let catalog = catalog(
        Layout::UnknownPrefix,
        DcbOrigin::Plain,
        false,
        false,
        CatalogPublicationMode::DiagnosticPartial,
    )?;
    let row = only_item(&catalog)?;
    let selection = proof::cell(&row["selection"], "attachmentComponentOrdinal", "present")?;
    assert_eq!(
        selection["value"], 2,
        "the exact first readable original slot survives"
    );
    assert_eq!(
        selection["sourceSetComplete"], false,
        "a prior unknown component can affect first-family choice"
    );
    assert!(
        selection["containsEncryptedSource"].is_null(),
        "plain known contributors plus unknown scope are not certified plain"
    );
    assert_eq!(
        row.pointer("/itemClass/value"),
        Some(&json!("Military")),
        "retain the value of the observed selected source"
    );
    assert_eq!(
        row.pointer("/itemClass/sourceSetComplete"),
        Some(&json!(false)),
        "derived classification retains selection uncertainty"
    );
    proof::closure(&catalog)
}

#[test]
fn source_bound_locale_pointer_and_physical_same_guid_aliases_preserve_each_original() -> TestResult
{
    let pointer = catalog(
        Layout::LocalePointer,
        DcbOrigin::Plain,
        true,
        false,
        CatalogPublicationMode::Normal,
    )?;
    proof::present(
        only_item(&pointer)?,
        "itemClass",
        &json!("Military"),
        Some(true),
    )?;
    let aliases = catalog(
        Layout::Aliases,
        DcbOrigin::Encrypted,
        false,
        false,
        CatalogPublicationMode::Normal,
    )?;
    let [first, second] = proof::rows(&aliases, "/items")? else {
        return Err("two original physical Items".into());
    };
    for (row, index, class) in [(first, 0, "Military"), (second, 1, "Stealth")] {
        proof::present(
            row,
            "guid",
            &json!(common::guid(fixture::OWNER)),
            Some(true),
        )?;
        proof::present(row, "recordIndex", &json!(index), Some(true))?;
        proof::present(row, "itemClass", &json!(class), Some(true))?;
        original_attachment(&aliases, index, 1, Some(true), false)?;
    }
    proof::closure(&pointer)?;
    proof::closure(&aliases)
}

#[test]
fn actual_english_dependency_and_key_only_playability_do_not_adopt_unrelated_locale_encryption()
-> TestResult {
    for (layout, expected) in [
        (Layout::Player, "player"),
        (Layout::Mission, "mission"),
        (Layout::MissingName, "special"),
    ] {
        for (english_encrypted, german_encrypted, class_marker) in
            [(true, false, true), (false, true, false)]
        {
            let catalog = catalog(
                layout,
                DcbOrigin::Plain,
                english_encrypted,
                german_encrypted,
                CatalogPublicationMode::Normal,
            )?;
            let row = only_item(&catalog)?;
            proof::present(row, "itemClass", &json!("Military"), Some(class_marker))?;
            proof::present(row, "playability", &json!(expected), Some(false))?;
            proof::closure(&catalog)?;
        }
    }
    Ok(())
}

fn catalog(
    layout: Layout,
    origin: DcbOrigin,
    english: bool,
    german: bool,
    mode: CatalogPublicationMode,
) -> TestResult<Value> {
    let (_folder, archive) = fixture::open(layout, origin, english, german)?;
    let mut options = common::options();
    options.publication_mode = mode;
    Ok(serde_json::from_slice(
        build_catalog(&archive, &options)?.json(),
    )?)
}

fn only_item(catalog: &Value) -> TestResult<&Value> {
    let [row] = proof::rows(catalog, "/items")? else {
        return Err("one selected physical Item".into());
    };
    Ok(row)
}

fn header_and_selection(
    row: &Value,
    marker: Option<bool>,
    attachment: u32,
    physics: u32,
    purchasable: u32,
) -> TestResult {
    proof::present(row, "guid", &json!(common::guid(fixture::OWNER)), marker)?;
    proof::present(row, "recordIndex", &json!(0), marker)?;
    for (field, ordinal) in [
        ("attachmentComponentOrdinal", attachment),
        ("physicsComponentOrdinal", physics),
        ("purchasableComponentOrdinal", purchasable),
    ] {
        proof::present(&row["selection"], field, &json!(ordinal), marker)?;
    }
    for field in [
        "type",
        "subType",
        "size",
        "grade",
        "mass",
        "manufacturerGuid",
        "tags",
    ] {
        assert!(
            row.get(field).is_none(),
            "{field} belongs to its canonical original family"
        );
    }
    Ok(())
}

fn component(catalog: &Value, index: u32, slot: u32) -> TestResult<&Value> {
    let definition = proof::rows(catalog, "/definitions/items/rows")?
        .iter()
        .find(|row| row.pointer("/recordIndex/value") == Some(&json!(index)))
        .ok_or("exact physical Item definition")?;
    proof::rows(definition, "/components/value")?
        .iter()
        .find(|row| row.pointer("/componentOrdinal/value") == Some(&json!(slot)))
        .ok_or_else(|| "exact original component slot".into())
}

fn original_attachment(
    catalog: &Value,
    index: u32,
    slot: u32,
    marker: Option<bool>,
    foreign: bool,
) -> TestResult {
    let row = proof::at(component(catalog, index, slot)?, "/value/value")?;
    for (field, value) in [
        ("itemType", "FixtureType"),
        ("itemSubtype", "FixtureSub"),
        ("tags", "TagOne TagTwo"),
        ("requiredTags", "RequiredOne RequiredTwo"),
    ] {
        proof::present(row, field, &json!(value), marker)?;
    }
    let size = if foreign {
        json!({"type":"uint64", "value":u64::MAX.to_string()})
    } else {
        json!({"type":"int32", "value":2})
    };
    proof::present(row, "size", &size, marker)?;
    Ok(())
}

const fn origins() -> [(DcbOrigin, Option<bool>); 3] {
    [
        (DcbOrigin::Plain, Some(false)),
        (DcbOrigin::Encrypted, Some(true)),
        (DcbOrigin::Unknown, None),
    ]
}

const fn complete_mode(origin: DcbOrigin) -> CatalogPublicationMode {
    match origin {
        DcbOrigin::Unknown => CatalogPublicationMode::DiagnosticPartial,
        DcbOrigin::Plain | DcbOrigin::Encrypted => CatalogPublicationMode::Normal,
    }
}

#[test]
fn unreadable_original_definition_still_refuses_normal_publication() -> TestResult {
    let (_folder, archive) =
        fixture::open(Layout::UnreadableFirst, DcbOrigin::Plain, false, false)?;
    let result = build_catalog(&archive, &common::options());
    assert!(
        matches!(result, Err(p4kx_export::CatalogError::IncompleteExport(_))),
        "the actual original read failure still guards normal publication"
    );
    Ok(())
}

#[test]
fn foreign_source_attachment_all_new_getters_match_the_actual_canonical_producer() -> TestResult {
    for (origin, _) in origins() {
        let (_folder, archive) = fixture::open(Layout::Foreign, origin, false, false)?;
        let mut options = common::options();
        options.publication_mode = complete_mode(origin);
        let catalog: Value = serde_json::from_slice(build_catalog(&archive, &options)?.json())?;
        let entry = DataForgeEntry::read(&archive)?;
        equipment::with_item_definitions_from(
            &archive,
            &entry,
            CatalogRecordSelection::All,
            CatalogLanguageSelection::Selected(&[Language::English, Language::German]),
            None,
            |snapshot| {
                let [row] = snapshot.rows() else {
                    return Err("one physical source Item".into());
                };
                let [canonical] = proof::rows(&catalog, "/definitions/items/rows")? else {
                    return Err("one actual canonical original".into());
                };
                super::equipment_sdk::walk::definition(row, snapshot.source_inventory(), canonical)
            },
        )??;
    }
    Ok(())
}
