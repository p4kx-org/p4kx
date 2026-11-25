// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_extraction::{Archive, DataForgeEntry, Guid};
use p4kx_testkit::{ArchiveBuilder, TestEntry, TestInstance, TestRecord, TestValue};
use serde_json::{Value, json};

use super::{TestResult, options};
use crate::catalog_fixture::{self, Maker};
use crate::health_origin_assertions as proof;
use crate::loadouts_support;

const MAKER_A: u128 = 0x2100_0000_0000_4000_8000_0000_0001;
const MAKER_B: u128 = 0x2100_0000_0000_4000_8000_0000_0002;
const SHIP_A: u128 = 0x9500_0000_0000_4000_8000_0000_0001;
const SHIP_B: u128 = 0x9500_0000_0000_4000_8000_0000_0002;

fn catalog_of(maker: Maker) -> Result<Value, Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let archive = Archive::open_with_key(
        catalog_fixture::write_archive_with(dir.path(), &[catalog_fixture::english()], maker)?,
        Some(p4kx_testkit::test_key()),
    )?;
    let built = p4kx_export::build_catalog(&archive, options().with_full_provenance())?;
    Ok(serde_json::from_slice(built.json())?)
}

fn manufacturer_guids<'c>(
    catalog: &'c Value,
    kind: &str,
) -> Result<Vec<&'c Value>, Box<dyn Error>> {
    let mut guids = Vec::new();
    for entry in catalog[kind].as_array().ok_or("catalog entities")? {
        if super::entity_field(entry, kind, "className").and_then(Value::as_str)
            == Some("Counter_Shop")
        {
            continue;
        }
        let guid = if kind == "ships" {
            entry.pointer("/vehicleManufacturer/manufacturer/value/guid/value")
        } else {
            super::selected_attachment(catalog, entry)?
                .pointer("/manufacturer/value/value/guid/value")
        };
        guids.push(guid.unwrap_or(&Value::Null));
    }
    Ok(guids)
}

#[test]
fn a_manufacturer_given_by_reference_is_its_guid() -> TestResult {
    let catalog = catalog_of(Maker::Reference)?;
    assert!(catalog.get("manufacturers").is_none());
    let maker = &catalog["definitions"]["manufacturers"]["rows"][0]["guid"]["value"];
    assert_eq!(maker, "10000000-0000-4000-8000-000000000001");
    for kind in ["items", "ships"] {
        let guids = manufacturer_guids(&catalog, kind)?;
        assert!(!guids.is_empty(), "{kind} present");
        assert!(guids.iter().all(|guid| *guid == maker), "{kind}: {guids:?}");
    }
    Ok(())
}

#[test]
fn manufacturer_pointers_keep_authored_storage_without_a_record_guid() -> TestResult {
    for maker in [Maker::StrongPointer, Maker::WeakPointer] {
        let catalog = catalog_of(maker)?;
        let rows = catalog["definitions"]["items"]["rows"]
            .as_array()
            .ok_or("canonical items")?;
        let item = rows
            .iter()
            .find(|row| row.pointer("/className/value") == Some(&json!("POWR_ACAS_S01")))
            .ok_or("canonical item")?;
        let attachment = item
            .pointer("/components/value")
            .and_then(Value::as_array)
            .and_then(|rows| rows.iter().find(|row| row["kind"] == "attachment"))
            .ok_or("source attachment")?;
        let manufacturer = attachment
            .pointer("/value/value/manufacturer")
            .ok_or("authored manufacturer pointer")?;
        let raw = manufacturer
            .pointer("/value/value/values/0")
            .ok_or("original pointer fact")?;
        assert_eq!(
            manufacturer["kind"], "authored",
            "retain the original pointer shape"
        );
        match maker {
            Maker::StrongPointer => assert_eq!(
                raw.pointer("/value/value/type"),
                Some(&json!("struct")),
                "retain an owned manufacturer value without inventing a Record reference",
            ),
            Maker::WeakPointer => assert_eq!(
                raw.pointer("/originalError/type"),
                Some(&json!("outsideRecord")),
                "retain the original outside-owner fact without blocking publication",
            ),
            Maker::Reference => return Err("a pointer fixture is required".into()),
        }
        for ship in proof::rows(&catalog, "/ships")? {
            assert!(
                ship.pointer("/vehicleManufacturer/manufacturer/value/authored")
                    .is_some(),
                "vehicle manufacturer pointers retain their original storage"
            );
        }
        for kind in ["items", "ships"] {
            let guids = manufacturer_guids(&catalog, kind)?;
            assert!(!guids.is_empty(), "{kind} present");
            assert!(guids.iter().all(|guid| guid.is_null()), "{kind}: {guids:?}");
        }
    }
    Ok(())
}

#[test]
fn opposite_ship_manufacturers_preserve_both_component_selectors_and_targets() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = opposite_manufacturer_archive(&dir.path().join("Data.p4k"))?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let built =
        p4kx_export::build_catalog_from(&archive, &entry, options().with_full_provenance())?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    for (ship_guid, vehicle_maker, attachment_maker, vehicle_slot, attach_slot) in [
        (SHIP_A, MAKER_A, MAKER_B, 1, 0),
        (SHIP_B, MAKER_B, MAKER_A, 0, 1),
    ] {
        let owner = forge
            .record_by_guid(loadouts_support::guid(ship_guid).parse()?)
            .ok_or("original ship Record")?;
        let row = proof::rows(&catalog, "/ships")?
            .iter()
            .find(|row| row["recordIndex"]["value"] == owner.index())
            .ok_or("projected ship")?;
        assert!(
            row.get("manufacturerGuid").is_none(),
            "one carrier per source"
        );
        let vehicle = proof::at(row, "/vehicleManufacturer")?;
        let attachment = proof::at(row, "/attachmentManufacturer")?;
        proof::present(
            vehicle,
            "componentOrdinal",
            &json!(vehicle_slot),
            Some(true),
        )?;
        proof::present(
            attachment,
            "componentOrdinal",
            &json!(attach_slot),
            Some(true),
        )?;
        let attachment_link = check_attach_def(attachment)?;
        let vehicle_link = proof::cell(vehicle, "manufacturer", "present")?;
        for (cell, expected) in [
            (vehicle_link, vehicle_maker),
            (attachment_link, attachment_maker),
        ] {
            let target = forge
                .record_by_guid(loadouts_support::guid(expected).parse()?)
                .ok_or("original manufacturer Record")?;
            let link = &cell["value"];
            assert_eq!(link["status"], "present");
            assert_link(&catalog, link, owner.index(), target.index(), target.guid())?;
        }
        assert_ne!(
            vehicle_link.pointer("/value/guid/sourceUsesRefs"),
            attachment_link.pointer("/value/guid/sourceUsesRefs"),
            "the reciprocal references belong to distinct original members"
        );
    }
    Ok(())
}

fn check_attach_def(attachment: &Value) -> Result<&Value, Box<dyn Error>> {
    let attach_def = proof::present(attachment, "attachDefPresent", &json!(true), Some(true))?;
    let attachment_link = proof::cell(attachment, "manufacturer", "present")?;
    for reference in proof::rows(attach_def, "/sourceUsesRefs")? {
        assert!(
            proof::rows(attachment_link, "/sourceUsesRefs")?.contains(reference),
            "the original AttachDef evidence participates in the attachment manufacturer"
        );
    }
    Ok(attachment_link)
}

fn opposite_manufacturer_archive(path: &std::path::Path) -> Result<Archive, Box<dyn Error>> {
    let vehicle = |maker| {
        TestValue::Struct(
            TestInstance::new("VehicleComponentParams")
                .value("manufacturer", TestValue::Reference(maker)),
        )
    };
    let attachment = |maker| {
        TestValue::Struct(
            TestInstance::new("SAttachableComponentParams").value(
                "AttachDef",
                TestValue::Struct(
                    TestInstance::new("SItemDefinition")
                        .value("Manufacturer", TestValue::Reference(maker)),
                ),
            ),
        )
    };
    let database = catalog_fixture::database(Maker::Reference)
        .record(manufacturer(MAKER_A, "FixtureVendorA"))
        .record(manufacturer(MAKER_B, "FixtureVendorB"))
        .record(ship(
            SHIP_A,
            "FixtureShipA",
            vec![attachment(MAKER_B), vehicle(MAKER_A)],
        ))
        .record(ship(
            SHIP_B,
            "FixtureShipB",
            vec![vehicle(MAKER_B), attachment(MAKER_A)],
        ))
        .build();
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("Data/Game2.dcb", database).encrypted())
        .entry(catalog_fixture::english())
        .write_to(path)?;
    Ok(Archive::open_with_key(
        path,
        Some(p4kx_testkit::test_key()),
    )?)
}

fn assert_link(catalog: &Value, link: &Value, owner: u32, target: u32, guid: Guid) -> TestResult {
    let value = proof::present(link, "guid", &json!(guid.to_string()), Some(true))?;
    let owner = owner.to_string();
    assert!(
        proof::uses(catalog, value)?.iter().any(|used| {
            used["role"] == "payload"
                && used.pointer("/location/namespace").and_then(Value::as_str)
                    == Some("dataforgeValue")
                && used.pointer("/location/indices/0").and_then(Value::as_str)
                    == Some(owner.as_str())
        }),
        "the stored GUID uses the original ship Record body"
    );
    let value = proof::present(link, "targetRecordIndex", &json!(target), Some(true))?;
    let target_position = target.to_string();
    assert!(
        proof::uses(catalog, value)?.iter().any(|used| {
            used.pointer("/location/namespace").and_then(Value::as_str)
                == Some("dataforgeRecordHeader")
                && used.pointer("/location/indices/0").and_then(Value::as_str)
                    == Some(target_position.as_str())
        }),
        "the resolved physical target uses its original manufacturer header"
    );
    Ok(())
}

fn manufacturer(guid: u128, name: &str) -> TestRecord {
    TestRecord::new(
        guid,
        "SCItemManufacturer",
        format!("SCItemManufacturer.{name}"),
        format!("libs/foundry/records/scitemmanufacturer/{name}.xml"),
    )
    .value("Code", TestValue::text(name))
}

fn ship(guid: u128, name: &str, components: Vec<TestValue>) -> TestRecord {
    TestRecord::new(
        guid,
        "EntityClassDefinition",
        format!("EntityClassDefinition.{name}"),
        format!("libs/foundry/records/entities/spaceships/{name}.xml"),
    )
    .value("Components", TestValue::Array(components))
}
