// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::{Archive, Language};
use p4kx_testkit::{ArchiveBuilder, TestEntry, test_key};
use serde_json::{Value, json};

use super::ReducedError;
use super::declared_equipment::{self, EquipmentTargets};
use super::declared_ports;
use super::record_targets::RecordTargets;
use super::selected_texts::SelectedTexts;
use crate::Kind;
use crate::build::original_input::test_support::TestResult;
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection,
};
use crate::catalog_queries::{ParsedCatalogQueries, ProvenanceMode};

#[path = "declared_equipment_fixture.rs"]
mod fixture;

#[test]
fn declared_equipment_retains_unmounted_parts_and_physical_port_slots() -> TestResult {
    with_queries(true, |parsed, scratch| {
        let targets = targets(parsed)?;
        let mut texts = SelectedTexts::default();
        let output =
            declared_equipment::collect(parsed, scratch, None, &targets.borrow(), &mut texts)?;
        let ports = serde_json::to_value(output.ports)?;
        let [ship, item] = rows(&ports)? else {
            return Err("Ship and standalone Item port containers".into());
        };
        assert_eq!(
            ship.pointer("/ownerGuid"),
            item.pointer("/ownerGuid"),
            "nominal aliases"
        );
        verify_component(ship, 0, 0)?;
        verify_component(item, 1, 1)?;
        let texts = serde_json::to_value(texts)?;
        let display_name = verify_label(&texts)?;
        verify_ports(ship, display_name)?;
        verify_ports(item, display_name)?;

        let attachments = serde_json::to_value(output.attachments)?;
        let [rule] = rows(&attachments)? else {
            return Err("standalone Item's authored attachment constraint".into());
        };
        verify_component(rule, 1, 0)?;
        for (key, value) in [
            ("/definition/value/itemType/value", json!("MiningModifier")),
            ("/definition/value/itemSubtype/value", json!("Active")),
            ("/definition/value/size/value/value", json!(2)),
            ("/definition/value/tags/value", json!("compatible")),
            ("/definition/value/requiredTags/value", json!("required")),
        ] {
            assert_eq!(rule.pointer(key), Some(&value), "attachment {key}");
        }
        assert!(
            rule.pointer("/definition/value/attachAxesToAlign")
                .is_none()
        );

        Ok(())
    })
}

fn verify_label(texts: &Value) -> Result<&str, Box<dyn std::error::Error>> {
    let [label] = texts
        .get("publicNames")
        .and_then(Value::as_array)
        .ok_or("public names")?
        .as_slice()
    else {
        return Err("unique selected label with two translations".into());
    };
    assert_eq!(
        label.pointer("/key/value"),
        Some(&json!(format!("@{}", fixture::DISPLAY_KEY))),
        "retain the exact authored locale key"
    );
    assert_eq!(
        label.pointer("/id/containsEncryptedSource"),
        Some(&json!(true)),
        "the public-name identifier retains its encrypted DataForge selector"
    );
    for (language, label_text) in [("en", "Mount"), ("de", "Halterung")] {
        let path = format!("/translations/value/{language}/displayText/value");
        assert_eq!(
            label.pointer(&path).and_then(Value::as_str),
            Some(label_text),
            "exact display text for {language}"
        );
    }
    Ok(label
        .pointer("/id/value")
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty())
        .ok_or("canonical public-name identifier")?)
}

fn verify_component(component: &Value, index: u32, ordinal: u32) -> TestResult {
    let observed_index = component
        .pointer("/ownerRecordIndex/value")
        .ok_or("physical owner")?;
    let observed_ordinal = component
        .pointer("/componentOrdinal/value")
        .ok_or("physical ordinal")?;
    assert_eq!(observed_index, &json!(index), "owner may share its GUID");
    assert_eq!(
        observed_ordinal,
        &json!(ordinal),
        "original component position"
    );
    assert_eq!(
        component
            .pointer("/ownerGuid/containsEncryptedSource")
            .and_then(Value::as_bool),
        Some(true),
        "original physical owner's DataForge marker",
    );
    Ok(())
}

fn verify_ports(component: &Value, display_name_ref: &str) -> TestResult {
    let [first, second] = component
        .pointer("/ports/value")
        .and_then(Value::as_array)
        .ok_or("ordered slots")?
        .as_slice()
    else {
        return Err("two authored port occurrences with the same name".into());
    };
    for (index, port) in [(0, first), (1, second)] {
        assert_eq!(
            port.pointer("/value/portOrdinal/value"),
            Some(&json!(index))
        );
        assert_eq!(
            port.pointer("/value/name/value"),
            Some(&json!("same_mount"))
        );
        let display_name = port
            .pointer("/value/displayName")
            .ok_or("port display name")?;
        assert_eq!(
            display_name.get("value").and_then(Value::as_str),
            Some(display_name_ref),
            "port display name closes over the canonical public-name identifier"
        );
        assert_eq!(
            display_name.get("containsEncryptedSource"),
            Some(&json!(true)),
            "the port display name retains its own encrypted DataForge selector"
        );
        assert_eq!(
            port.pointer("/value/restrictions/requiredPortTags/value"),
            Some(&json!("compatible")),
        );
        assert_eq!(
            port.pointer("/value/restrictions/maxSize/state"),
            Some(&json!("absent"))
        );
        let subtypes = "/value/restrictions/types/value/0/value/subTypes/value";
        let [subtype, repeated] = port
            .pointer(subtypes)
            .and_then(Value::as_array)
            .ok_or("ordered subtype array")?
            .as_slice()
        else {
            return Err("duplicate physical subtype occurrences".into());
        };
        assert_eq!(
            subtype, repeated,
            "the same declared subtype at two original slots"
        );
        assert_eq!(subtype.pointer("/value/sourceText"), Some(&json!("Active")));
        assert_eq!(
            subtype.pointer("/containsEncryptedSource"),
            Some(&json!(true))
        );
        assert!(port.pointer("/value/restrictions/tagReferences").is_none());
        assert!(port.pointer("/value/attachedItem").is_none());
    }
    Ok(())
}

#[test]
fn declared_equipment_distinguishes_empty_present_from_absent() -> TestResult {
    with_queries(false, |parsed, scratch| {
        let targets = targets(parsed)?;
        let mut texts = SelectedTexts::default();
        let output =
            declared_equipment::collect(parsed, scratch, None, &targets.borrow(), &mut texts)?;
        let ports = serde_json::to_value(output.ports)?;
        let attachments = serde_json::to_value(output.attachments)?;
        assert_eq!(ports.get("availability"), Some(&json!("available")));
        assert_eq!(attachments.get("availability"), Some(&json!("absent")));
        assert!(rows(&ports)?.is_empty(), "no present port containers");
        assert!(
            rows(&attachments)?.is_empty(),
            "no attachment type in source"
        );
        Ok(())
    })
}

#[test]
fn declared_equipment_rejects_an_aliased_target_in_the_wrong_physical_family() -> TestResult {
    with_queries(true, |parsed, scratch| {
        let targets = targets(parsed)?;
        let wrong = EquipmentTargets {
            ships: &targets.items,
            items: &targets.ships,
        };
        let error = declared_equipment::collect(
            parsed,
            scratch,
            None,
            &wrong,
            &mut SelectedTexts::default(),
        );
        assert!(
            matches!(
                error,
                Err(ReducedError::UnresolvedRecordLink {
                    path: declared_ports::PATH,
                    index: 0,
                })
            ),
            "the matching nominal GUID in a different source Record cannot satisfy the join",
        );
        Ok(())
    })
}

fn rows(table: &Value) -> Result<&[Value], Box<dyn std::error::Error>> {
    Ok(table
        .get("rows")
        .and_then(Value::as_array)
        .ok_or("staged rows")?)
}

#[derive(Default)]
struct Targets {
    ships: RecordTargets,
    items: RecordTargets,
}

impl Targets {
    const fn borrow(&self) -> EquipmentTargets<'_> {
        EquipmentTargets {
            ships: &self.ships,
            items: &self.items,
        }
    }
}

fn targets(parsed: &ParsedCatalogQueries<'_>) -> Result<Targets, ReducedError> {
    let mut targets = Targets::default();
    let _ = parsed.visit_declared_equipment(CatalogRecordSelection::All, None, |snapshot| {
        let [row] = snapshot.rows() else {
            return Err(ReducedError::IncompleteScope("fixture equipment"));
        };
        let output = match row.kind() {
            Kind::Ships => &mut targets.ships,
            Kind::Items => &mut targets.items,
            _ => return Err(ReducedError::IncompleteScope("fixture equipment")),
        };
        output.insert("fixture equipment", row.source())
    })?;
    Ok(targets)
}

fn with_queries(
    include_attachments: bool,
    inspect: impl FnOnce(&ParsedCatalogQueries<'_>, &Path) -> TestResult,
) -> TestResult {
    let scratch = tempfile::tempdir()?;
    let input = scratch.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(
            TestEntry::stored("Data/Game2.dcb", fixture::database(include_attachments)).encrypted(),
        )
        .entry(fixture::localization(Language::English, "Mount"))
        .entry(fixture::localization(Language::German, "Halterung"))
        .write_to(&input)?;
    let archive = Archive::open_with_key(input, Some(test_key()))?;
    let session = CatalogQuerySession::open(
        &archive,
        CatalogLanguageSelection::Selected(&[Language::English, Language::German]),
        None,
    )?
    .with_provenance_mode(ProvenanceMode::MarkersOnly);
    inspect(&session.parsed(None)?, scratch.path())
}
