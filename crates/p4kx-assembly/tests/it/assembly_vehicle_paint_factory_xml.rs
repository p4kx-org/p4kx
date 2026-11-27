// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::LoadoutChanges;
use serde_json::Value;

use super::{TestResult, assemble, field_selectors, paint_evidence, string_at};

#[test]
fn an_encrypted_factory_loadout_include_contributes_to_every_painted_field() -> TestResult {
    let assembled = assemble("TEST_Hull_XmlPaint", &LoadoutChanges::new())?;
    let proof = paint_evidence(&assembled)?;
    let physical = proof
        .pointer("/sourceInventory/sources")
        .and_then(Value::as_array)
        .ok_or("physical source inventory")?;
    let origins = proof
        .get("fieldOrigins")
        .ok_or("fitted paint field origins")?;
    for field in [
        "/paintGuid",
        "/appearanceTag",
        "/variantKind",
        "/geometry",
        "/material",
        "/paletteGuid",
    ] {
        let origin = origins.get(field).ok_or("fitted field origin")?;
        assert_eq!(
            origin
                .get("containsEncryptedSource")
                .and_then(Value::as_bool),
            Some(true),
            "the actual encrypted Include was needed for {field}"
        );
        let selectors = field_selectors(&proof, field)?;
        for (path, encryption, attribute) in [
            (
                "Data/Scripts/Loadouts/fixture-paint.xml",
                "encrypted",
                "loadout",
            ),
            (
                "Data/Scripts/Loadouts/fixture-paint-items.xml",
                "unencrypted",
                "itemName",
            ),
        ] {
            let matching = selectors
                .iter()
                .filter(|selector| {
                    string_at(selector, "/location/namespace") == Some("textXmlAttribute")
                        && string_at(selector, "/location/field") == Some(attribute)
                        && string_at(selector, "/effectiveEncryption") == Some(encryption)
                        && physical.iter().any(|owner| {
                            let owner_path = string_at(owner, "/path/value");
                            owner.get("id") == selector.get("source") && owner_path == Some(path)
                        })
                })
                .count();
            assert_eq!(
                matching, 1,
                "one actual XML attribute from {path} contributes to {field}"
            );
        }
    }
    Ok(())
}
