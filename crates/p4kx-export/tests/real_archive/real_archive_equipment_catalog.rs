// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::time::UNIX_EPOCH;

use p4kx_export::{
    CatalogOptions, CatalogPublicationMode, Kind, build_catalog_from, is_kind_leftover,
};
use p4kx_extraction::{Archive, ArchiveWatch, DataForgeEntry, Filter};
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde_json::Value;

use crate::real_archive_item_durability::rows;

#[path = "equipment_owner_bindings.rs"]
pub(super) mod owner_bindings;

pub(super) type TestResult<T = ()> = Result<T, Box<dyn Error>>;

pub(super) const WEAPONS: &str = "/configurations/weaponActions/rows";
pub(super) const CONTAINERS: &str = "/configurations/ammoContainers/rows";
pub(super) const BINDINGS: &str = "/declarations/ammoBindings/rows";
pub(super) const AMMO: &str = "/definitions/ammoDefinitions/rows";
pub(super) const MAGAZINES: &str = "/projections/weaponMagazineResolutions/rows";

pub(super) fn catalog() -> TestResult<Value> {
    let archive = Archive::open_with_key(real_archive_path()?, real_archive_key().ok())?;
    let watch = ArchiveWatch::new(&archive);
    let entry = DataForgeEntry::read(&archive)?;
    let catalog = publish(&archive, &entry, None)?;
    watch.check()?;
    Ok(catalog)
}

pub(super) fn item_catalog(class: &str) -> TestResult<Value> {
    let archive = Archive::open_with_key(real_archive_path()?, real_archive_key().ok())?;
    let watch = ArchiveWatch::new(&archive);
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let rule = Kind::Items.rule().ok_or("original Item admission rule")?;
    let mut selected = None;
    for record in forge.records() {
        let path = record.record_path()?;
        if !rule.matches_path(path) || record.type_name()? != rule.type_name {
            continue;
        }
        let name = record.name()?;
        let class_name = name
            .strip_prefix(rule.type_name)
            .and_then(|rest| rest.strip_prefix('.'))
            .filter(|value| !value.is_empty())
            .unwrap_or(name);
        if class_name.eq_ignore_ascii_case(class) && !is_kind_leftover(Kind::Items, path, name) {
            assert!(
                selected.replace(path).is_none(),
                "the Item name resolves to one physical Record"
            );
        }
    }
    let filter = Filter::new(selected.ok_or("the original Item name resolves to a Record")?);
    let catalog = publish(&archive, &entry, Some(&filter))?;
    watch.check()?;
    Ok(catalog)
}

fn publish(
    archive: &Archive,
    entry: &DataForgeEntry,
    filter: Option<&Filter>,
) -> TestResult<Value> {
    let options = CatalogOptions {
        filter,
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..CatalogOptions::new(UNIX_EPOCH)
    };
    let export = build_catalog_from(archive, entry, options.with_full_provenance())?;
    Ok(serde_json::from_slice(export.json())?)
}

pub(super) fn item<'a>(catalog: &'a Value, class: &str) -> TestResult<&'a Value> {
    rows(catalog, "/items")?
        .iter()
        .find(|item| {
            item.pointer("/className/value")
                .and_then(Value::as_str)
                .is_some_and(|name| name.eq_ignore_ascii_case(class))
        })
        .ok_or_else(|| format!("no item {class}").into())
}

pub(super) fn owner_rows<'a>(
    catalog: &'a Value,
    table: &str,
    item: &Value,
) -> impl Iterator<Item = &'a Value> {
    let validation = owner_bindings::check(catalog, table, item);
    assert!(
        validation.is_ok(),
        "physical Item owner bindings in {table}: {validation:?}"
    );
    let guid = item.get("guid");
    let record = item.get("recordIndex");
    catalog
        .pointer(table)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(move |row| row.get("ownerGuid") == guid && row.get("ownerRecordIndex") == record)
}

pub(super) fn owner<'a>(catalog: &'a Value, table: &str, item: &Value) -> TestResult<&'a Value> {
    owner_rows(catalog, table, item)
        .next()
        .ok_or_else(|| format!("original Item owner in {table}").into())
}

pub(super) fn definition<'a>(
    catalog: &'a Value,
    table: &str,
    index: &Value,
    index_path: &str,
) -> Option<&'a Value> {
    catalog
        .pointer(table)?
        .as_array()?
        .iter()
        .find(|row| !index.is_null() && row.pointer(index_path) == Some(index))
}

pub(super) fn action_nodes(weapon: &Value) -> impl Iterator<Item = &Value> {
    weapon
        .get("actionNodes")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
}

pub(super) fn scalar(row: &Value, path: &str, expected: f32) -> TestResult {
    let cell = row
        .pointer(path)
        .ok_or_else(|| format!("original numeric carrier {path}"))?;
    assert_eq!(
        cell["state"], "present",
        "readable original numeric field at {path}"
    );
    assert_eq!(
        cell.pointer("/value/rawBits").and_then(Value::as_str),
        Some(format!("{:08x}", expected.to_bits()).as_str()),
        "original Single value at {path}"
    );
    Ok(())
}

pub(super) struct Ammo<'catalog> {
    pub(super) container: &'catalog Value,
    pub(super) definition: Option<&'catalog Value>,
}

pub(super) fn primary_ammo<'catalog>(
    catalog: &'catalog Value,
    item: &Value,
) -> Option<Ammo<'catalog>> {
    let resolution = owner_rows(catalog, MAGAZINES, item).next()?;
    let outcome = resolution.pointer("/outcome/value")?;
    let record = match outcome.get("type")?.as_str()? {
        "ownContainer" => resolution.pointer("/ownerRecordIndex/value")?,
        "linkedContainer" => outcome.pointer("/magazineRecordIndex/value")?,
        _ => return None,
    };
    let slot = outcome.pointer("/containerOrdinal/value")?;
    let same_slot = |row: &Value| {
        row.pointer("/ownerRecordIndex/value") == Some(record)
            && row.pointer("/componentOrdinal/value") == Some(slot)
    };
    let container = rows(catalog, CONTAINERS)
        .ok()?
        .iter()
        .find(|row| same_slot(row))?;
    let binding = rows(catalog, BINDINGS)
        .ok()?
        .iter()
        .filter(|row| row["type"] == "containerSlots")
        .find_map(|row| row.get("value").filter(|row| same_slot(row)))?;
    let definition = binding
        .pointer("/primaryAmmo/value/targetRecordIndex/value")
        .and_then(|target| definition(catalog, AMMO, target, "/recordIndex/value"));
    Some(Ammo {
        container,
        definition,
    })
}

pub(super) fn bullet_damage(ammo: &Value) -> Option<&Value> {
    let projectile = ammo.pointer("/projectileParams/value/kind")?;
    (projectile["type"] == "bullet")
        .then(|| projectile.pointer("/value/damage/value"))
        .flatten()
}
