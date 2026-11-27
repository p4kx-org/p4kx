// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{ArchiveBuilder, CryXmlByteOrder, CryXmlSignature, TestElement, TestEntry};
use serde_json::{Value, json};

use super::{EXCHANGE, LOOSE, ROOT, TestResult, build};
use crate::health_origin_assertions as health;
use crate::shops_fixture::REGISTRY;
use crate::starmap_fixture;

const LOCATION: &str = "41000000-0000-4000-8000-000000000017";

fn registry() -> Vec<u8> {
    let key = format!("{ROOT},{LOCATION},{EXCHANGE}");
    let malformed = TestElement::new("ShopBackup")
        .attribute("ID", &key)
        .attribute("SuperName", "WrongLength");
    let winner = TestElement::new("ShopBackup")
        .attribute("ShopName", "  Authored winner  ")
        .attribute("ID", key.to_ascii_uppercase())
        .attribute("DefaultValue", "d0000000-0000-4000-8000-000000000001")
        .attribute("SuperName", "Root,Body,Town,SCShop_CommEx_TDD_Fixture");
    let short_key = format!("4a269932-183e-362d-aacc-188e7debbea9,{LOOSE}");
    let short = TestElement::new("ShopBackup")
        .attribute("ID", short_key)
        .attribute("DefaultValue", " ")
        .attribute("SuperName", "Root,SCShop_Fixture");
    let duplicate = TestElement::new("ShopBackup")
        .attribute("ID", &key)
        .attribute("SuperName", "OtherRoot,OtherBody,OtherTown,SCShop_Other")
        .attribute("ShopName", "Losing duplicate");
    TestElement::new("BackupData")
        .child(
            TestElement::new("LayerBackups")
                .child(
                    TestElement::new("LayerBackup")
                        .attribute("LayerName", "First")
                        .attribute("PlatformName", "FixtureSystem")
                        .child(
                            TestElement::new("ShopBackups")
                                .child(malformed)
                                .child(winner)
                                .child(short),
                        ),
                )
                .child(
                    TestElement::new("LayerBackup")
                        .attribute("PlatformName", "OtherSystem")
                        .child(TestElement::new("ShopBackups").child(duplicate)),
                ),
        )
        .to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little)
}

fn origin(
    catalog: &Value,
    cell: &Value,
    encrypted: bool,
    field: &str,
    indices: &[&str],
    role: &str,
) -> TestResult {
    assert_eq!(
        cell["containsEncryptedSource"], encrypted,
        "{field} carries the encryption of the physical registry Entry"
    );
    assert_eq!(cell["sourceSetComplete"], true, "{field} is complete");
    let uses = health::uses(catalog, cell)?;
    let [evidence] = uses.as_slice() else {
        return Err(format!("{field} should use one original registry attribute").into());
    };
    assert_eq!(
        evidence["location"],
        json!({"type": "subresource", "namespace": "xml", "indices": indices, "field": field}),
        "{field} uses the winning exact XML attribute position"
    );
    assert_eq!(
        evidence["scope"], "payload",
        "{field} reads the XML payload"
    );
    assert_eq!(evidence["role"], role, "{field} retains its actual role");
    assert_eq!(
        health::source(catalog, evidence)?.pointer("/path/value"),
        Some(&json!(REGISTRY)),
        "{field} is linked to the original registry Entry"
    );
    Ok(())
}

fn winning_row(catalog: &Value, shop: &Value, encrypted: bool) -> TestResult {
    for (name, value, field, indices) in [
        (
            "key",
            json!(format!("{ROOT},{LOCATION},{EXCHANGE}")),
            "@ID",
            ["5", "1"],
        ),
        ("entityGuid", json!(EXCHANGE), "@ID", ["5", "1"]),
        (
            "entityName",
            json!("SCShop_CommEx_TDD_Fixture"),
            "@SuperName",
            ["5", "3"],
        ),
        ("system", json!("fixture"), "@PlatformName", ["2", "1"]),
        (
            "registryName",
            json!("Authored winner"),
            "@ShopName",
            ["5", "0"],
        ),
        (
            "defaultValue",
            json!("d0000000-0000-4000-8000-000000000001"),
            "@DefaultValue",
            ["5", "2"],
        ),
        ("body", json!("Body"), "@SuperName", ["5", "3"]),
        ("location", json!("Town"), "@SuperName", ["5", "3"]),
        ("commodityExchange", json!(true), "@SuperName", ["5", "3"]),
    ] {
        let cell = health::present(shop, name, &value, Some(encrypted))?;
        origin(catalog, cell, encrypted, field, &indices, "conversion")?;
    }
    let names = health::cell(shop, "containers", "present")?;
    origin(
        catalog,
        names,
        encrypted,
        "@SuperName",
        &["5", "3"],
        "conversion",
    )?;
    let [root, body, location] = health::rows(names, "/value")? else {
        return Err("original root, body, and location slots".into());
    };
    for (cell, name) in [(root, "Root"), (body, "Body"), (location, "Town")] {
        assert_eq!(cell["value"], name, "author order is preserved");
        origin(
            catalog,
            cell,
            encrypted,
            "@SuperName",
            &["5", "3"],
            "conversion",
        )?;
    }
    Ok(())
}

fn optional_fields(catalog: &Value, row: &Value, encrypted: bool) -> TestResult {
    for (name, field, indices, role) in [
        ("registryName", "$absent:@ShopName", &["6"][..], "selector"),
        (
            "defaultValue",
            "@DefaultValue",
            &["6", "1"][..],
            "conversion",
        ),
        ("body", "@SuperName", &["6", "2"][..], "conversion"),
        ("location", "@SuperName", &["6", "2"][..], "conversion"),
    ] {
        let cell = health::cell(row, name, "absent")?;
        origin(catalog, cell, encrypted, field, indices, role)?;
    }
    let no_location = health::cell(row, "starmapObjectGuid", "absent")?;
    origin(
        catalog,
        no_location,
        encrypted,
        "@SuperName",
        &["6", "2"],
        "conversion",
    )?;
    Ok(())
}

#[test]
fn selected_registry_fields_retain_original_positions_and_encryption() -> TestResult {
    for encrypted in [false, true] {
        let (built, catalog) = build(
            |dir| {
                let path = dir.join("Data.p4k");
                let registry = TestEntry::stored(REGISTRY, registry());
                let entry = if encrypted {
                    registry.encrypted()
                } else {
                    registry.with_encryption_field(0)
                };
                ArchiveBuilder::v2()
                    .entry(
                        TestEntry::stored("Data\\Game2.dcb", starmap_fixture::database().build())
                            .with_encryption_field(0),
                    )
                    .entry(entry)
                    .write_to(&path)?;
                Ok(path)
            },
            None,
        )?;
        let summary = built.summary().shops();
        assert_eq!(
            summary.malformed, 1,
            "the first declaration of the winning key is malformed"
        );
        assert_eq!(
            summary.duplicate, 1,
            "the later valid duplicate is not emitted"
        );
        let rows = health::rows(&catalog, "/shops")?;
        assert_eq!(rows.len(), 2, "both distinct valid shops survive");
        let find = |guid| {
            rows.iter()
                .find(|row| row["entityGuid"]["value"] == guid)
                .ok_or("admitted shop")
        };
        winning_row(&catalog, find(EXCHANGE)?, encrypted)?;
        optional_fields(&catalog, find(LOOSE)?, encrypted)?;
        health::closure(&catalog)?;
    }
    Ok(())
}
