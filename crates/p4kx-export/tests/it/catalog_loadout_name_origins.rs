// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::CatalogError;
use p4kx_export::catalog_queries::CatalogRecordSelection;
use p4kx_export::catalog_queries::equipment::read_loadouts_from;
use p4kx_extraction::{SourceValueState, SourcedValue};
use p4kx_testkit::{DataForgeBuilder, TestEntity, TestEntry, TestLoadout, TestPort, entity_schema};
use serde_json::{Value, json};

use crate::health_origin_assertions as check;
use crate::health_origin_fixture::{Fixture, Input, TestResult};
use crate::sdk_full_proof as full_proof;

const PATH: &str = "Data/synthetic/names.xml";
const XML: &str = r#"<Loadout><Items><Item portName="shared"/></Items></Loadout>"#;

#[test]
fn actual_name_cells_keep_mixed_physical_origins_and_sdk_parity() -> TestResult {
    for input in [Input::Plain, Input::Encrypted] {
        for encrypted_xml in [false, true] {
            compare_fixture(input, encrypted_xml)?;
        }
    }
    Ok(())
}

fn compare_fixture(input: Input, encrypted_xml: bool) -> TestResult {
    let fixture = fixture(input, XML, encrypted_xml)?;
    let (_, catalog) = fixture.diagnostic()?;
    let snapshot = full_proof::from_entry(
        &fixture.archive,
        &fixture.entry,
        full_proof::NO_LANGUAGES,
        None,
    )?
    .read_loadouts(CatalogRecordSelection::All, None)?;
    let serialized = serde_json::to_value(&snapshot.rows)?;
    assert_eq!(
        Some(&serialized),
        catalog.get("loadouts"),
        "one canonical interpretation"
    );
    let scoped = json!({"loadouts": serialized, "provenance": snapshot.source_inventory});
    let [dcb, first_xml, reused_xml] = snapshot.rows.as_slice() else {
        return Err("three physical synthetic owners".into());
    };
    let [first, second] = dcb.ports() else {
        return Err("two original equal-name DCB slots".into());
    };
    check_typed_name(first.name(), input.marker());
    assert_ne!(
        first.name().origin().source_uses_refs(),
        second.name().origin().source_uses_refs(),
        "equal names from distinct physical slots remain distinct"
    );
    let [dcb_wire, first_wire, reused_wire] = check::rows(&scoped, "/loadouts")? else {
        return Err("three serialized synthetic owners".into());
    };
    check_dcb(&scoped, dcb_wire, input.marker())?;
    for (wire, owner) in [(first_wire, first_xml), (reused_wire, reused_xml)] {
        let [port] = owner.ports() else {
            return Err("one actually read or cached XML slot".into());
        };
        assert_eq!(
            port.name().value().map(String::as_str),
            Some("shared"),
            "exact XML spelling"
        );
        check_xml(&scoped, wire, encrypted_xml)?;
    }
    let ([first_name], [reused_name]) = (first_xml.ports(), reused_xml.ports()) else {
        return Err("one original name in each reused occurrence".into());
    };
    assert_eq!(
        first_name.name(),
        reused_name.name(),
        "reuse retains the original physical XML name instead of minting another source"
    );
    check::closure(&scoped)?;
    Ok(())
}

#[test]
fn standalone_names_keep_unknown_origin_and_the_sdk_preserves_archive_admission() -> TestResult {
    for encrypted_xml in [false, true] {
        let fixture = fixture(Input::Standalone, XML, encrypted_xml)?;
        let (_, catalog) = fixture.diagnostic()?;
        let [dcb, first_xml, reused_xml] = catalog["loadouts"]
            .as_array()
            .ok_or("original producer Loadouts")?
            .as_slice()
        else {
            return Err("three physical standalone producer owners".into());
        };
        check_dcb(&catalog, dcb, None)?;
        check_xml(&catalog, first_xml, encrypted_xml)?;
        check_xml(&catalog, reused_xml, encrypted_xml)?;
        assert!(
            matches!(
                read_loadouts_from(
                    &fixture.archive,
                    &fixture.entry,
                    CatalogRecordSelection::All,
                    None,
                ),
                Err(CatalogError::SourceOriginMismatch)
            ),
            "standalone bytes cannot impersonate the SDK's archive-bound DataForge input"
        );
        check::closure(&catalog)?;
    }
    Ok(())
}

#[test]
fn a_missing_original_name_receipt_keeps_present_spelling_with_unknown_origin() -> TestResult {
    let fixture = fixture(
        Input::Plain,
        "<Loadout><Items><Item/></Items></Loadout>",
        true,
    )?;
    let snapshot = read_loadouts_from(
        &fixture.archive,
        &fixture.entry,
        CatalogRecordSelection::RecordIndex(1),
        None,
    )?;
    let [owner] = snapshot.rows.as_slice() else {
        return Err("the physical XML owner".into());
    };
    let [port] = owner.ports() else {
        return Err("one unnamed XML slot remains a real occurrence".into());
    };
    let name = port.name();
    assert_eq!(
        name.state(),
        SourceValueState::Present,
        "resolved spelling is retained"
    );
    assert_eq!(
        name.value().map(String::as_str),
        Some(""),
        "no invented stored name"
    );
    assert_eq!(
        name.origin().contains_encrypted_source(),
        None,
        "Archive alone proves no datum"
    );
    assert!(
        !name.origin().source_set_complete(),
        "missing original getter proof"
    );
    assert!(
        name.origin().source_uses_refs().is_empty(),
        "no fabricated name use"
    );
    Ok(())
}

fn check_typed_name(name: &SourcedValue<String>, marker: Option<bool>) {
    assert_eq!(
        name.state(),
        SourceValueState::Present,
        "original name is present"
    );
    assert_eq!(
        name.value().map(String::as_str),
        Some("shared"),
        "exact DCB spelling"
    );
    assert_eq!(
        name.origin().contains_encrypted_source(),
        marker,
        "own DCB origin"
    );
}

#[test]
fn normal_publication_keeps_the_same_canonical_name_cells() -> TestResult {
    let fixture = fixture(Input::Plain, XML, false)?;
    let normal = p4kx_export::build_catalog_from(
        &fixture.archive,
        &fixture.entry,
        crate::health_origin_fixture::options().with_full_provenance(),
    )?;
    let normal: Value = serde_json::from_slice(normal.json())?;
    let (_, diagnostic) = fixture.diagnostic()?;
    assert_eq!(
        normal["loadouts"], diagnostic["loadouts"],
        "publication policy changes no name"
    );
    check::closure(&normal)
}

fn fixture(input: Input, xml: &str, encrypted: bool) -> TestResult<Fixture> {
    let path = |id| format!("libs/foundry/records/entities/scitem/ships/names{id}.xml");
    let first = TestEntity::new(1, "Names0", path(0))
        .attach_type("WeaponGun")
        .port(TestPort::new("shared"))
        .port(TestPort::new("shared"));
    let mut builder = entity_schema(DataForgeBuilder::new().version(8)).record(first.record());
    for id in [2, 3] {
        builder = builder.record(
            TestEntity::new(id, format!("Names{id}"), path(id))
                .attach_type("WeaponGun")
                .loadout(TestLoadout::Xml("synthetic/names.xml".to_owned()))
                .record(),
        );
    }
    let entry = TestEntry::stored(PATH, xml.as_bytes());
    let entry = if encrypted { entry.encrypted() } else { entry };
    Fixture::open(&builder.build(), input, vec![entry])
}

fn check_dcb(document: &Value, owner: &Value, marker: Option<bool>) -> TestResult {
    let ports = owner["ports"]
        .as_array()
        .ok_or("original DCB port occurrences")?;
    assert_eq!(
        ports.len(),
        2,
        "both equal-name physical declarations survive"
    );
    for port in ports {
        let cell = check::present(port, "name", &json!("shared"), marker)?;
        let uses = check::uses(document, cell)?;
        assert!(
            uses.iter().any(|used| used["role"] == "payload"
                && used.pointer("/location/namespace") == Some(&json!("dataforgeValue"))),
            "name retains its actual DCB Payload instead of turning all uses into Selectors"
        );
        assert!(
            uses.iter().any(|used| used["role"] == "selector"),
            "the original getter retains its actual declaration ancestry"
        );
    }
    Ok(())
}

fn check_xml(document: &Value, owner: &Value, encrypted: bool) -> TestResult {
    let [port] = owner["ports"]
        .as_array()
        .ok_or("one original XML slot")?
        .as_slice()
    else {
        return Err("one XML name cell".into());
    };
    let cell = check::present(port, "name", &json!("shared"), Some(encrypted))?;
    let original = roxmltree::Document::parse(XML)?;
    let node = original
        .descendants()
        .find(|node| node.has_attribute("portName"))
        .ok_or("independently decoded original XML port")?;
    let attribute = node
        .attributes()
        .position(|attribute| attribute.name() == "portName")
        .ok_or("independently decoded original attribute ordinal")?;
    let expected = json!({
        "type": "subresource",
        "namespace": "xmlAttribute",
        "indices": [node.id().get().to_string(), attribute.to_string()],
        "field": "@portName"
    });
    let uses = check::uses(document, cell)?;
    assert_eq!(
        uses.len(),
        2,
        "only actual XML Payload and decoder Conversion"
    );
    for role in ["payload", "conversion"] {
        let contribution = uses
            .iter()
            .find(|used| used["role"] == role)
            .ok_or("the actual original name contribution")?;
        assert_eq!(
            contribution["location"], expected,
            "independent original XML coordinate"
        );
        assert_eq!(
            check::source(document, contribution)?["kind"],
            "archiveEntry",
            "actual entry admission"
        );
    }
    Ok(())
}
