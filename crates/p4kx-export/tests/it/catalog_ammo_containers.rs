// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogOptions, CatalogPublicationMode, build_catalog};
use p4kx_extraction::Filter;
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::health_origin_assertions as proof;

#[path = "catalog_ammo_binding_declaration.rs"]
mod declaration;
#[path = "catalog_ammo_containers_fixture.rs"]
mod fixture;

const CONFIGURATIONS: &str = "/configurations/ammoContainers/rows";
const BINDINGS: &str = "/declarations/ammoBindings/rows";
const DEFINITIONS: &str = "/definitions/ammoDefinitions/rows";

#[test]
fn ammo_containers_preserve_ordered_settings_and_physical_binding_slots() -> TestResult {
    let (_folder, archive) = common::open(
        &fixture::database(fixture::Case::Valid),
        DcbOrigin::Encrypted,
    )?;
    let catalog: Value =
        serde_json::from_slice(build_catalog(&archive, &common::options())?.json())?;
    let [first, second] = proof::rows(&catalog, CONFIGURATIONS)? else {
        return Err("both original container component occurrences".into());
    };
    check_owner(first, 1, true)?;
    check_owner(second, 2, true)?;
    check_settings(first, second)?;

    let [first_binding, second_binding] = bindings(&catalog)? else {
        return Err("one declaration row per physical container".into());
    };
    let first_binding = slots(first_binding, 1, true)?;
    let second_binding = slots(second_binding, 2, true)?;
    check_link(&catalog, first_binding, "primaryAmmo", fixture::PRIMARY, 0)?;
    check_link(
        &catalog,
        first_binding,
        "secondaryAmmo",
        fixture::SECONDARY,
        1,
    )?;
    check_link(
        &catalog,
        second_binding,
        "primaryAmmo",
        fixture::SECONDARY,
        1,
    )?;
    proof::cell(second_binding, "secondaryAmmo", "null")?;

    assert_eq!(
        proof::at(&catalog, "/exportHealth/status")?,
        "complete",
        "valid typed source"
    );
    let validator = jsonschema::validator_for(&p4kx_export::catalog_schema())?;
    validator
        .validate(&catalog)
        .map_err(|error| error.to_string())?;
    proof::closure(&catalog)
}

#[test]
fn ammo_container_bindings_keep_headers_when_definition_filter_excludes_the_targets() -> TestResult
{
    let (_folder, archive) = common::open(
        &fixture::database(fixture::Case::NoSecondaryDeclaration),
        DcbOrigin::Plain,
    )?;
    let filter = Filter::parse("*/entities/scitem/*")?;
    let options = CatalogOptions {
        filter: Some(&filter),
        ..common::options()
    };
    let catalog: Value = serde_json::from_slice(build_catalog(&archive, &options)?.json())?;
    assert!(
        proof::rows(&catalog, DEFINITIONS)?.is_empty(),
        "binding cannot auto-select Ammo"
    );
    let [row, _] = bindings(&catalog)? else {
        return Err("filtered Item still owns both container slots".into());
    };
    let row = slots(row, 1, false)?;
    let link = target(row, "primaryAmmo", "present")?;
    proof::present(link, "targetRecordIndex", &json!(0), Some(false))?;
    proof::cell(row, "secondaryAmmo", "absent")?;
    proof::closure(&catalog)
}

#[test]
fn ammo_container_bindings_distinguish_dangling_and_wrong_type_references() -> TestResult {
    let (_folder, archive) = common::open(
        &fixture::database(fixture::Case::BadLinks),
        DcbOrigin::Encrypted,
    )?;
    let options = CatalogOptions {
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..common::options()
    };
    let catalog: Value = serde_json::from_slice(build_catalog(&archive, &options)?.json())?;
    let [row, _] = bindings(&catalog)? else {
        return Err("unavailable variants retain the original container".into());
    };
    let row = slots(row, 1, true)?;
    let dangling = target(row, "primaryAmmo", "dangling")?;
    proof::present(
        dangling,
        "guid",
        &json!(common::guid(fixture::MISSING)),
        Some(true),
    )?;
    let wrong = target(row, "secondaryAmmo", "unexpectedTargetType")?;
    proof::present(
        wrong,
        "guid",
        &json!(common::guid(fixture::OWNER)),
        Some(true),
    )?;
    proof::present(wrong, "targetRecordIndex", &json!(2), Some(true))?;
    proof::present(
        wrong,
        "targetType",
        &json!("EntityClassDefinition"),
        Some(true),
    )?;
    proof::closure(&catalog)
}

fn check_settings(first: &Value, second: &Value) -> TestResult {
    proof::present(
        first,
        "sourceType",
        &json!("SAmmoContainerComponentParams"),
        Some(true),
    )?;
    proof::present(first, "initialAmmoCount", &json!(0), Some(true))?;
    proof::present(first, "maxAmmoCount", &json!(-7), Some(true))?;
    proof::present(first, "maxRestockCount", &json!(u32::MAX), Some(true))?;
    proof::present(
        first,
        "lowAmmoWarningPercentage",
        &json!({"rawBits":"80000000","finite":-0.0}),
        Some(true),
    )?;
    proof::present(
        first,
        "ammoCountAnimationBlendTime",
        &json!({"rawBits":"3f800000","finite":1.0}),
        Some(true),
    )?;
    proof::present(
        first,
        "ammoContainerType",
        &json!("FixtureMagazine"),
        Some(true),
    )?;
    proof::present(
        first,
        "despawnEmptyAmmoContainer",
        &json!(false),
        Some(true),
    )?;
    proof::present(first, "allowAmmoRepool", &json!(true), Some(true))?;
    proof::present(
        first,
        "emptyGeometryTag",
        &text("fixture-empty"),
        Some(true),
    )?;
    let fragment = proof::at(
        proof::cell(first, "ammoCountFragment", "present")?,
        "/value",
    )?;
    proof::present(fragment, "fragment", &text("fixture-fragment"), Some(true))?;
    proof::present(fragment, "forceWeaponController", &json!(false), Some(true))?;
    check_attachment_arrays(first, second)
}

fn check_attachment_arrays(first: &Value, second: &Value) -> TestResult {
    let hidden = proof::cell(first, "hideAttachments", "present")?;
    let [before, after] = proof::rows(hidden, "/value")? else {
        return Err("both ordered hide-attachment entries".into());
    };
    for (entry, name, threshold, visible) in [
        (before, "fixture-before", 7, false),
        (after, "fixture-after", 0, true),
    ] {
        let value = proof::at(entry, "/value")?;
        proof::present(value, "attachmentName", &text(name), Some(true))?;
        proof::present(value, "ammoThreshold", &json!(threshold), Some(true))?;
        proof::present(value, "visible", &json!(visible), Some(true))?;
    }
    proof::present(first, "attachableEntities", &json!([]), Some(true))?;
    let attachments = proof::cell(second, "attachableEntities", "present")?;
    let [attachment] = proof::rows(attachments, "/value")? else {
        return Err("one original attachable entity".into());
    };
    let attachment = proof::at(attachment, "/value")?;
    for (field, value) in [
        ("sourcePort", "fixture-source"),
        ("targetPort", "fixture-target"),
        ("entityClassName", "fixture-class"),
    ] {
        proof::present(attachment, field, &text(value), Some(true))?;
    }
    Ok(())
}

fn bindings(catalog: &Value) -> TestResult<&[Value]> {
    proof::rows(catalog, BINDINGS)
}

fn slots(binding: &Value, slot: u32, encrypted: bool) -> TestResult<&Value> {
    assert_eq!(
        proof::at(binding, "/type")?,
        "containerSlots",
        "the authored binding family"
    );
    let binding = proof::at(binding, "/value")?;
    check_owner(binding, slot, encrypted)?;
    Ok(binding)
}

fn check_owner(row: &Value, slot: u32, encrypted: bool) -> TestResult {
    proof::present(
        row,
        "ownerGuid",
        &json!(common::guid(fixture::OWNER)),
        Some(encrypted),
    )?;
    proof::present(row, "ownerRecordIndex", &json!(2), Some(encrypted))?;
    proof::present(row, "componentOrdinal", &json!(slot), Some(encrypted))?;
    Ok(())
}

fn target<'row>(row: &'row Value, role: &str, status: &str) -> TestResult<&'row Value> {
    let target = proof::at(proof::cell(row, role, "present")?, "/value")?;
    assert_eq!(
        proof::at(target, "/status")?,
        status,
        "{role} physical reference outcome"
    );
    Ok(target)
}

fn check_link(catalog: &Value, row: &Value, role: &str, guid: u128, index: u32) -> TestResult {
    let link = target(row, role, "present")?;
    let guid = json!(common::guid(guid));
    proof::present(link, "guid", &guid, Some(true))?;
    proof::present(link, "targetRecordIndex", &json!(index), Some(true))?;
    let definition = proof::rows(catalog, DEFINITIONS)?
        .iter()
        .find(|row| {
            row.pointer("/recordIndex/value").and_then(Value::as_u64) == Some(u64::from(index))
        })
        .ok_or("physically joined Ammo definition")?;
    proof::present(definition, "guid", &guid, Some(true))?;
    Ok(())
}

fn text(value: &str) -> Value {
    json!({"type": "gameText", "sourceText": value})
}
