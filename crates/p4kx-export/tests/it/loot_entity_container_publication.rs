// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::Cell;
use std::fs;

use p4kx_export::{
    CatalogError, CatalogFile, Compression, build_catalog_from, catalog_schema, write_catalog_from,
};
use p4kx_testkit::{TestContainer, TestEntry};
use serde_json::{Value, json};

use crate::health_origin_assertions as proof;
use crate::health_origin_entry_source as entries;
use crate::health_origin_fixture::{self as fixture, Fixture, TestResult};

const ROOT: &str = "Data/ObjectContainers/PU/system/synthetic/syntheticsystem.socpak";
const TARGET: &str = "Data/ObjectContainers/PU/synthetic/missing.socpak";
const ENTITY_DOCUMENT: &str = "entdata\\source.entxml";

#[test]
fn configless_container_links_remain_lossless_but_refuse_normal_before_output_creation()
-> TestResult {
    let complete = source(false)?;
    let previous = build_catalog_from(&complete.archive, &complete.entry, &fixture::options())?;
    assert!(
        previous.summary().export_health().is_complete(),
        "complete original counterpart"
    );
    let previous_catalog: Value = serde_json::from_slice(previous.json())?;
    verify_root(&previous_catalog)?;
    assert!(
        proof::rows(
            &previous_catalog,
            "/declarations/lootEntityContainerDocuments/rows"
        )?
        .is_empty(),
        "the reached complete counterpart has no original link document"
    );
    let source = source(true)?;
    let (partial, catalog) = source.diagnostic()?;
    verify_original(&catalog)?;
    assert!(
        !partial.summary().export_health().is_complete(),
        "retained original is not a completed derived projection"
    );
    assert!(
        jsonschema::validator_for(&catalog_schema())?.is_valid(&catalog),
        "the actual additive document carrier validates"
    );
    proof::closure(&catalog)?;
    assert!(matches!(
        build_catalog_from(&source.archive, &source.entry, &fixture::options()),
        Err(CatalogError::IncompleteExport(_))
    ));
    verify_refusal(&source, previous.json())
}

fn source(missing: bool) -> TestResult<Fixture> {
    let mut container = TestContainer::new().entry(TestEntry::stored(
        "syntheticsystem.xml",
        r#"<ObjectContainer version="14"/>"#,
    ));
    if missing {
        let xml = format!(
            r#"<Root><Entity CryGUID="00000000-0000-0000-0000-000000000001" pos=" 1, 2, 3 " rot="2,0,0,0"><ObjectContainerComponent objectContainer="{TARGET}"/></Entity></Root>"#
        );
        container = container.entry(TestEntry::stored(ENTITY_DOCUMENT, xml));
    }
    entries::with_starmap(vec![
        TestEntry::stored(ROOT, container.build()).encrypted(),
        TestEntry::stored(fixture::REGISTRY, b"<LayerBackups/>"),
    ])
}

fn verify_root(catalog: &Value) -> TestResult {
    assert_eq!(
        proof::rows(catalog, "/definitions/starmapObjects/rows")?.len(),
        1,
        "the actual selected Record activates the Loot placement scope"
    );
    let roots = proof::rows(catalog, "/placements/containerInstances/rows")?;
    let [root] = roots else {
        return Err("one actual reached root container instance".into());
    };
    verify_physical(catalog, proof::cell(root, "container", "present")?, ROOT)
}

fn verify_physical(catalog: &Value, carrier: &Value, path: &str) -> TestResult {
    let reference = proof::at(carrier, "/value")?;
    let source = proof::rows(catalog, "/provenance/sources")?
        .iter()
        .find(|source| source.get("id") == reference.get("inputRef"))
        .ok_or("the reference identifies its actual physical source")?;
    assert_eq!(
        source.get("kind"),
        Some(&json!("archiveEntry")),
        "physical Entry kind"
    );
    assert_eq!(
        source.pointer("/path/value"),
        Some(&json!(path)),
        "original Entry path"
    );
    assert_eq!(
        source.get("entryIndex"),
        reference.get("entryIndex"),
        "original Entry ordinal"
    );
    Ok(())
}

fn verify_original(catalog: &Value) -> TestResult {
    verify_root(catalog)?;
    let documents = proof::rows(catalog, "/declarations/lootEntityContainerDocuments/rows")?;
    let [document] = documents else {
        return Err("one physical reached EntXML document".into());
    };
    let roots = proof::rows(catalog, "/placements/containerInstances/rows")?;
    let [root] = roots else {
        return Err("one original containing root".into());
    };
    assert_eq!(
        proof::at(document, "/containingContainer/value")?,
        proof::at(root, "/container/value")?,
        "the original document belongs to the actually reached physical root"
    );
    verify_physical(
        catalog,
        proof::cell(document, "document", "present")?,
        ENTITY_DOCUMENT,
    )?;
    proof::present(document, "configReferences", &json!([]), Some(true))?;
    let links = proof::rows(document, "/links/value")?;
    let [link] = links else {
        return Err("one original configless link attribute".into());
    };
    verify_link(catalog, link)?;
    verify_nodes(document)?;
    assert!(
        proof::rows(catalog, "/lootContainers")?.is_empty(),
        "raw configless source is not invented as a multi-config placement"
    );
    Ok(())
}

fn verify_link(catalog: &Value, link: &Value) -> TestResult {
    let raw = proof::present(link, "declaredPath", &json!(TARGET), Some(true))?;
    let status = proof::present(link, "targetStatus", &json!("notFound"), Some(true))?;
    let candidates = proof::present(link, "targetCandidates", &json!([]), Some(true))?;
    assert_eq!(
        status["sourceUsesRefs"], candidates["sourceUsesRefs"],
        "negative status and candidate set cite the same original lookup"
    );
    assert!(
        proof::uses(catalog, status)?
            .iter()
            .any(|used| used.pointer("/location/type") == Some(&json!("archiveDirectoryPaths"))),
        "absence has original directory evidence"
    );
    assert!(
        proof::uses(catalog, raw)?
            .iter()
            .any(
                |used| used.pointer("/location/field") == Some(&json!("@objectContainer"))
                    && used.get("role") == Some(&json!("payload"))
                    && used
                        .pointer("/location/indices")
                        .and_then(Value::as_array)
                        .is_some_and(|indices| indices.len() == 2)
            ),
        "exact attribute position and raw Payload provenance survive"
    );
    Ok(())
}

fn verify_nodes(document: &Value) -> TestResult {
    let nodes = proof::rows(document, "/nodes/value")?;
    let entity = nodes
        .iter()
        .find(|node| node.pointer("/name/value") == Some(&json!("Entity")))
        .ok_or("actual Entity owner node")?;
    assert!(
        proof::rows(entity, "/attributes")?
            .iter()
            .any(
                |attribute| attribute.pointer("/name/value") == Some(&json!("pos"))
                    && attribute.pointer("/value/value") == Some(&json!(" 1, 2, 3 "))
            ),
        "original local position text is retained without a fabricated world placement"
    );
    Ok(())
}

fn verify_refusal(source: &Fixture, previous: &[u8]) -> TestResult {
    for exists in [false, true] {
        let path = source.folder.path().join(format!("catalog-{exists}.json"));
        if exists {
            fs::write(&path, previous)?;
        }
        let called = Cell::new(false);
        let result = write_catalog_from(
            &source.archive,
            &source.entry,
            &fixture::options(),
            Compression::None,
            || {
                called.set(true);
                CatalogFile::create(&path)
            },
        );
        assert!(
            matches!(result, Err(CatalogError::IncompleteExport(_))),
            "normal source health refusal remains"
        );
        assert!(!called.get(), "refusal precedes output creation callback");
        if exists {
            assert_eq!(
                fs::read(&path)?,
                previous,
                "last complete generation remains byte-identical"
            );
        } else {
            assert!(!path.exists(), "no new partial normal output exists");
        }
    }
    Ok(())
}
