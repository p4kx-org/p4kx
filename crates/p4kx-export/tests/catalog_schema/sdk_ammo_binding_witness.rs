// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::UNIX_EPOCH;

use p4kx_export::catalog_queries::equipment::{
    AmmoBinding, ContainerAmmoSlots, WeaponSourceSnapshot, with_ammo_definitions_from,
};
use p4kx_export::catalog_queries::{CatalogLanguageSelection, CatalogRecordSelection};
use p4kx_export::{CatalogOptions, build_catalog_from};
use p4kx_extraction::{Archive, DataForgeEntry, FieldOrigin, SourceInventory};
use serde_json::Value;

use super::{AMMO_BINDINGS, TestResult};

const AMMO_ROWS: &str = "/definitions/ammoDefinitions/rows";

pub(super) fn normal(archive: &Archive, entry: &DataForgeEntry) -> TestResult<Value> {
    let catalog = build_catalog_from(archive, entry, &CatalogOptions::new(UNIX_EPOCH))?;
    Ok(serde_json::from_slice(catalog.json())?)
}

pub(super) fn verify(
    archive: &Archive,
    entry: &DataForgeEntry,
    snapshot: &WeaponSourceSnapshot<'_>,
    catalog: &Value,
) -> TestResult<()> {
    let slots = compare_bindings(snapshot, catalog)?;
    for (source, index) in [(slots.primary_ammo(), 0), (slots.secondary_ammo(), 1)] {
        let reference = source
            .value()
            .ok_or("the original authored Ammo Reference")?;
        assert_eq!(
            reference.resolved_index(),
            Some(index),
            "the actual authored slot points to its physical Ammo definition"
        );
        compare_origin(
            reference.guid().origin(),
            snapshot.source_inventory(),
            catalog,
        )?;
        with_ammo_definitions_from(
            archive,
            entry,
            CatalogRecordSelection::RecordIndex(index),
            CatalogLanguageSelection::Selected(&[]),
            None,
            |target| {
                let [ammo] = target.rows() else {
                    return Err("one original selected Ammo definition".into());
                };
                assert_eq!(
                    ammo.source().guid().value(),
                    reference.guid().value(),
                    "the indexed definition and original Reference identify the same GUID"
                );
                compare_definition(
                    &serde_json::to_value(ammo)?,
                    index,
                    ammo.speed().origin(),
                    target.source_inventory(),
                    catalog,
                )
            },
        )??;
    }
    Ok(())
}

fn compare_definition(
    sdk: &Value,
    index: u32,
    speed: &FieldOrigin,
    inventory: &SourceInventory,
    catalog: &Value,
) -> TestResult<()> {
    let rows = catalog
        .pointer(AMMO_ROWS)
        .and_then(Value::as_array)
        .ok_or("the canonical Ammo dataset")?;
    let canonical = rows
        .iter()
        .find(|row| {
            row.pointer("/recordIndex/value").and_then(Value::as_u64) == Some(u64::from(index))
        })
        .ok_or("the producer admits the same physical Ammo definition")?;
    assert_eq!(
        sdk, canonical,
        "the indexed target retains the complete canonical row"
    );
    compare_origin(speed, inventory, catalog)
}

fn compare_bindings<'snapshot, 'source>(
    snapshot: &'snapshot WeaponSourceSnapshot<'source>,
    catalog: &Value,
) -> TestResult<&'snapshot ContainerAmmoSlots<'source>> {
    let [owner] = snapshot.rows() else {
        return Err("one physical weapon Item in the selected scope".into());
    };
    let [
        AmmoBinding::WeaponMagazineSlots { .. },
        AmmoBinding::ContainerSlots { value },
    ] = owner.ammo_bindings.as_slice()
    else {
        return Err("the owner has the original weapon and container declarations".into());
    };
    let expected = catalog
        .pointer(&format!("{AMMO_BINDINGS}/rows"))
        .ok_or("the canonical authored bindings")?;
    assert_eq!(
        &serde_json::to_value(&owner.ammo_bindings)?,
        expected,
        "both original binding rows retain their values, order and direct provenance"
    );
    Ok(value)
}

fn compare_origin(origin: &FieldOrigin, sdk: &SourceInventory, catalog: &Value) -> TestResult<()> {
    let catalog_uses = catalog
        .pointer("/provenance/uses")
        .and_then(Value::as_array)
        .ok_or("the canonical original contributor inventory")?;
    assert_eq!(
        origin.contains_encrypted_source(),
        Some(true),
        "the field retains original encrypted source evidence"
    );
    assert!(
        origin.source_set_complete() && !origin.source_uses_refs().is_empty(),
        "the original operand has a complete and resolvable source set"
    );
    for reference in origin.source_uses_refs() {
        let site = sdk
            .iter_uses()
            .find(|site| site.id() == reference)
            .ok_or("the scoped SDK resolves the referenced original source use")?;
        let original = catalog_uses
            .iter()
            .find(|row| row.get("id").and_then(Value::as_str) == Some(reference.as_str()))
            .ok_or("the normal producer resolves the same original contributor")?;
        assert_eq!(
            &serde_json::to_value(site)?,
            original,
            "the source coordinate, provenance and role match the normal producer"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use p4kx_export::catalog_queries::DefinitionStatus;
    use p4kx_export::catalog_queries::equipment::with_weapon_sources_from;
    use p4kx_extraction::{Archive, DataForgeEntry, SourceValueState};
    use p4kx_testkit::{ArchiveBuilder, TestEntry, test_key};

    use super::super::fixture;
    use super::{TestResult, compare_bindings, compare_origin, normal};

    #[test]
    fn dangling_original_ammo_is_distinct_from_an_explicit_null_slot() -> TestResult<()> {
        let folder = tempfile::tempdir()?;
        for case in [fixture::Case::DanglingAmmo, fixture::Case::MissingAmmo] {
            let path = folder.path().join(format!("binding-{case:?}.p4k"));
            ArchiveBuilder::new()
                .entry(
                    TestEntry::stored("Data/Game2.dcb", fixture::build(case).build()).encrypted(),
                )
                .write_to(&path)?;
            let archive = Archive::open_with_key(path, Some(test_key()))?;
            let entry = DataForgeEntry::read(&archive)?;
            let catalog = normal(&archive, &entry)?;
            with_weapon_sources_from(&archive, &entry, 3, None, |snapshot| -> TestResult<()> {
                let slots = compare_bindings(&snapshot, &catalog)?;
                assert_eq!(
                    slots
                        .primary_ammo()
                        .value()
                        .and_then(|ammo| ammo.resolved_index()),
                    Some(0),
                    "the readable sibling retains its own physical target"
                );
                let secondary = slots.secondary_ammo();
                if case == fixture::Case::DanglingAmmo {
                    let missing = secondary
                        .value()
                        .ok_or("the stored GUID still has a value")?;
                    assert!(
                        matches!(missing.status(), DefinitionStatus::Dangling)
                            && missing.resolved_index().is_none()
                            && missing.guid().value().is_some(),
                        "a dangling target retains its authored GUID without a physical index"
                    );
                    compare_origin(
                        missing.guid().origin(),
                        snapshot.source_inventory(),
                        &catalog,
                    )?;
                } else {
                    assert_eq!(
                        secondary.state(),
                        SourceValueState::Null,
                        "the original explicit null stores no target"
                    );
                    compare_origin(secondary.origin(), snapshot.source_inventory(), &catalog)?;
                }
                Ok(())
            })??;
        }
        Ok(())
    }
}
