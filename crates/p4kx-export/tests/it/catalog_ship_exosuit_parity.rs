// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Catalog and SDK parity for playable exosuits and physical Record aliases.

use crate::sdk_full_proof as full_proof;
use std::collections::BTreeMap;
use std::time::UNIX_EPOCH;

use p4kx_export::catalog_queries::equipment::ShipSource;
use p4kx_export::catalog_queries::{CatalogLanguageSelection, CatalogRecordSelection};
use p4kx_export::{ShipBaseOptions, VehicleKind, build_catalog_from, build_ship_base_from};
use p4kx_extraction::{DataForge, DataForgeEntry, Record};
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::health_origin_assertions as proof;

#[path = "catalog_ship_exosuit_availability.rs"]
mod availability;
#[path = "catalog_ship_exosuit_fixture.rs"]
mod fixture;

#[test]
fn exosuit_catalog_and_sdk_admit_the_same_physical_owners_and_sources() -> TestResult {
    for (origin, encrypted) in [(DcbOrigin::Plain, false), (DcbOrigin::Encrypted, true)] {
        let (_directory, archive) = common::open(&fixture::database(), origin)?;
        let entry = DataForgeEntry::read(&archive)?;
        let forge = entry.open()?;
        let expected = expected_owners(&forge)?;
        let built = build_catalog_from(&archive, &entry, common::options().with_full_provenance())?;
        let catalog: Value = serde_json::from_slice(built.json())?;
        let ships = catalog_rows(&catalog, &expected, encrypted)?;
        let standalone = build_ship_base_from(&archive, &entry, &ShipBaseOptions::new(UNIX_EPOCH))?;
        check_standalone(standalone.ships(), &expected)?;
        full_proof::from_entry(
            &archive,
            &entry,
            CatalogLanguageSelection::Selected(&[]),
            None,
        )?
        .with_ship_sources(
            CatalogRecordSelection::All,
            None,
            |snapshot| -> TestResult {
                assert_eq!(
                    snapshot.rows().len(),
                    ships.len(),
                    "same admitted physical rows"
                );
                let context = json!({"provenance": snapshot.source_inventory()});
                let mut remaining = ships;
                for row in snapshot.rows() {
                    let index = *row
                        .source()
                        .record_index()
                        .value()
                        .ok_or("SDK physical owner")?;
                    let producer = remaining
                        .remove(&index)
                        .ok_or("unique admitted Catalog row")?;
                    compare_row(row, producer, &context, &catalog)?;
                    let authored = expected.get(&index).ok_or("expected Actor presentation")?;
                    let name = row.name().value().ok_or("SDK authored Actor name")?;
                    let text = snapshot.public_name(name).ok_or("SDK retained name key")?;
                    assert_eq!(text.key().value().copied(), Some(authored.name_key));
                }
                assert!(
                    remaining.is_empty(),
                    "the SDK reached every admitted Catalog row"
                );
                Ok(())
            },
        )??;
        proof::closure(&catalog)?;
    }
    Ok(())
}

struct ExpectedOwner<'source> {
    owner: Record<'source>,
    class: &'static str,
    driver_index: u32,
    name_key: &'static str,
    manufacturer: Option<fixture::Manufacturer>,
}

fn expected_owners<'source>(
    forge: &'source DataForge<'_>,
) -> TestResult<BTreeMap<u32, ExpectedOwner<'source>>> {
    let mut expected = BTreeMap::new();
    for owner in &fixture::OWNERS {
        let guid = super::guid(owner.guid);
        let driver_name = format!("EntityClassDefinition.{}", owner.driver_class());
        let driver = forge
            .records()
            .find(|record| record.name().is_ok_and(|name| name == driver_name))
            .ok_or("synthetic driver Record")?;
        let candidates: Vec<_> = forge
            .records()
            .filter(|record| record.guid().to_string() == guid)
            .collect();
        let count = if owner.guid == fixture::ALIASED_OWNER {
            2
        } else {
            1
        };
        assert_eq!(
            candidates.len(),
            count,
            "independent fixture rows for {}",
            owner.class
        );
        let owner_name = format!("EntityClassDefinition.{}", owner.class);
        for record in candidates {
            assert_eq!(
                record.name()?,
                owner_name,
                "each fixture selects the original Actor class"
            );
            expected.insert(
                record.index(),
                ExpectedOwner {
                    owner: record,
                    class: owner.class,
                    driver_index: driver.index(),
                    name_key: owner.name_key,
                    manufacturer: owner.manufacturer,
                },
            );
        }
    }
    Ok(expected)
}

fn catalog_rows<'catalog>(
    catalog: &'catalog Value,
    expected: &BTreeMap<u32, ExpectedOwner<'_>>,
    encrypted: bool,
) -> TestResult<BTreeMap<u32, &'catalog Value>> {
    let rows = proof::rows(catalog, "/ships")?;
    assert_eq!(
        rows.len(),
        expected.len(),
        "the Actor, development, and seat controls are excluded"
    );
    let mut emitted = BTreeMap::new();
    for row in rows {
        let index = u32::try_from(
            proof::at(row, "/recordIndex/value")?
                .as_u64()
                .ok_or("Catalog physical owner")?,
        )?;
        let original = expected
            .get(&index)
            .ok_or("admitted original exosuit owner")?;
        let owner = &original.owner;
        proof::present(row, "recordIndex", &json!(index), Some(encrypted))?;
        proof::present(
            row,
            "guid",
            &json!(owner.guid().to_string()),
            Some(encrypted),
        )?;
        proof::present(row, "className", &json!(original.class), Some(encrypted))?;
        proof::present(
            row,
            "recordPath",
            &json!(owner.record_path()?),
            Some(encrypted),
        )?;
        proof::present(row, "vehicleKind", &json!("exosuit"), Some(encrypted))?;
        check_presentation(catalog, row, original, encrypted)?;
        if proof::at(row, "/variantOf/state")? != "absent" {
            return Err("the independent Actor has a variant base".into());
        }
        let playability = proof::present(row, "playability", &json!("player"), Some(encrypted))?;
        let uses = proof::uses(catalog, playability)?;
        assert!(
            uses_record(&uses, index),
            "the player's original Actor owner is retained"
        );
        assert!(
            uses_record(&uses, original.driver_index),
            "the original linked driver Record is retained"
        );
        assert!(
            emitted.insert(index, row).is_none(),
            "each physical owner appears once"
        );
    }
    Ok(emitted)
}

fn check_presentation(
    catalog: &Value,
    row: &Value,
    owner: &ExpectedOwner<'_>,
    encrypted: bool,
) -> TestResult {
    let name = proof::cell(row, "name", "present")?;
    let index = owner.owner.index();
    assert!(
        name["containsEncryptedSource"] == encrypted
            && uses_record(&proof::uses(catalog, name)?, index),
        "the Name points to its authored Actor field and encryption state"
    );
    let text = proof::rows(catalog, "/texts/publicNames")?
        .iter()
        .find(|text| text.pointer("/id/value") == name.get("value"))
        .ok_or("public name from the original Actor locale")?;
    proof::present(text, "key", &json!(owner.name_key), Some(encrypted))?;

    let vehicle = proof::at(row, "/vehicleManufacturer")?;
    proof::cell(vehicle, "manufacturer", "absent")?;
    let attachment = proof::at(row, "/attachmentManufacturer")?;
    proof::present(
        attachment,
        "attachDefPresent",
        &json!(true),
        Some(encrypted),
    )?;
    let Some(manufacturer) = owner.manufacturer else {
        let cell = proof::cell(attachment, "manufacturer", "null")?;
        assert!(
            cell["containsEncryptedSource"] == encrypted
                && uses_record(&proof::uses(catalog, cell)?, index),
            "the stored null manufacturer retains its Actor field and encryption state"
        );
        return Ok(());
    };
    let link = proof::at(
        proof::cell(attachment, "manufacturer", "present")?,
        "/value",
    )?;
    assert_eq!(
        link["status"], "present",
        "the manufacturer reference resolves"
    );
    let guid = json!(super::guid(manufacturer.guid));
    proof::present(link, "guid", &guid, Some(encrypted))?;
    let original = proof::rows(catalog, "/definitions/manufacturers/rows")?
        .iter()
        .find(|definition| definition.pointer("/guid/value") == Some(&guid))
        .ok_or("definition of the stored manufacturer")?;
    proof::present(
        link,
        "targetRecordIndex",
        proof::at(original, "/recordIndex/value")?,
        Some(encrypted),
    )?;
    proof::present(original, "code", &json!(manufacturer.code), Some(encrypted))?;
    Ok(())
}

fn check_standalone(
    ships: &[p4kx_export::ShipBase],
    expected: &BTreeMap<u32, ExpectedOwner<'_>>,
) -> TestResult {
    let mut original = Vec::new();
    for admitted in expected.values() {
        original.push((
            admitted.owner.guid(),
            admitted.class,
            admitted.owner.record_path()?,
        ));
    }
    let mut emitted = Vec::new();
    for ship in ships {
        assert_eq!(
            ship.vehicle_kind,
            Some(VehicleKind::Exosuit),
            "the standalone reader classifies each proved Actor"
        );
        emitted.push((
            ship.guid,
            ship.class_name.as_str(),
            ship.record_path.as_str(),
        ));
    }
    original.sort_unstable();
    emitted.sort_unstable();
    assert_eq!(
        emitted, original,
        "standalone vehicle export retains the same owners"
    );
    Ok(())
}

fn uses_record(uses: &[&Value], index: u32) -> bool {
    let index = index.to_string();
    uses.iter().any(|used| {
        matches!(
            used.pointer("/location/namespace").and_then(Value::as_str),
            Some("dataforgeRecordHeader" | "dataforgeRecordBody" | "dataforgeValue")
        ) && used.pointer("/location/indices/0").and_then(Value::as_str) == Some(index.as_str())
    })
}

fn compare_row(row: &ShipSource<'_>, producer: &Value, sdk: &Value, catalog: &Value) -> TestResult {
    for (field, selected) in [
        (
            "recordIndex",
            serde_json::to_value(row.source().record_index())?,
        ),
        (
            "guid",
            serde_json::to_value(row.source().guid().map_ref(ToString::to_string))?,
        ),
        (
            "className",
            serde_json::to_value(row.source().class_name())?,
        ),
        (
            "recordPath",
            serde_json::to_value(row.source().record_path())?,
        ),
        ("vehicleKind", serde_json::to_value(row.vehicle_kind())?),
        ("playability", serde_json::to_value(row.playability())?),
        ("name", serde_json::to_value(row.name())?),
        (
            "vehicleManufacturer",
            serde_json::to_value(row.vehicle_manufacturer())?,
        ),
        (
            "attachmentManufacturer",
            serde_json::to_value(row.attachment_manufacturer())?,
        ),
    ] {
        assert_eq!(
            proof::with_origins(selected, sdk)?,
            proof::with_origins(
                producer.get(field).ok_or("Catalog source cell")?.clone(),
                catalog
            )?,
            "the SDK and Catalog agree on {field} and its original evidence"
        );
    }
    Ok(())
}
