// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;

use p4kx_export::{
    CatalogError, CatalogFile, Compression, build_catalog_from, catalog_schema, write_catalog_from,
};

use p4kx_testkit::{TestContainer, TestEntry};
use serde_json::{Value, json};

use crate::health_origin_assertions as proof;
use crate::health_origin_entry_source as entries;
use crate::health_origin_fixture::{self as fixture, Fixture, TestResult};

#[path = "loot_entity_container_publication_assertions.rs"]
mod assertions;
#[path = "loot_entity_container_sdk.rs"]
mod sdk;

const ROOT: &str = "Data/ObjectContainers/PU/system/synthetic/syntheticsystem.socpak";
const TARGET: &str = "Data/ObjectContainers/PU/synthetic/missing.socpak";
const ENTITY_DOCUMENT: &str = "entdata\\source.entxml";
const UNSUPPORTED: &str = "C:/authoring/unsupported.socpak";

#[test]
fn proved_configless_container_absence_publishes_with_original_evidence_and_warning() -> TestResult
{
    let source = source(Some(TARGET))?;
    let path = source.folder.path().join("catalog.json");
    let (summary, file) = write_catalog_from(
        &source.archive,
        &source.entry,
        &fixture::options(),
        Compression::None,
        || CatalogFile::create(&path),
    )?;
    file.commit()?;
    assert!(summary.export_health().is_complete());
    let catalog: Value = serde_json::from_reader(fs::File::open(path)?)?;
    assertions::verify_original(&catalog)?;
    assertions::verify_warning(&catalog)?;
    assert!(jsonschema::validator_for(&catalog_schema())?.is_valid(&catalog));
    proof::closure(&catalog)
}

#[test]
fn absent_literal_with_brackets_requires_no_repair() -> TestResult {
    let source = source(Some("[authoring]/missing.socpak"))?;
    let published = build_catalog_from(&source.archive, &source.entry, &fixture::options())?;
    let catalog: Value = serde_json::from_slice(published.json())?;
    let link = proof::at(
        &catalog,
        "/declarations/lootEntityContainerDocuments/rows/0/links/value/0",
    )?;
    proof::present(link, "targetStatus", &json!("notFound"), Some(true))?;
    assert_eq!(
        link.get("targetLookupRepair"),
        Some(&Value::Null),
        "a valid literal bracketed path uses only the ordinary lookup"
    );
    assertions::verify_warning(&catalog)
}

#[test]
fn unsupported_container_selector_refuses_normal_before_output_creation() -> TestResult {
    let complete = source(None)?;
    let previous = build_catalog_from(&complete.archive, &complete.entry, &fixture::options())?;
    assert!(previous.summary().export_health().is_complete());
    let source = source(Some(UNSUPPORTED))?;
    let (partial, catalog) = source.diagnostic()?;
    assert!(!partial.summary().export_health().is_complete());
    assert_eq!(
        catalog.pointer(
            "/declarations/lootEntityContainerDocuments/rows/0/links/value/0/targetStatus/value"
        ),
        Some(&json!("unsupportedSelector"))
    );
    assert_eq!(
        catalog.pointer("/declarations/lootEntityContainerDocuments/rows/0/links/value/0/targetStatus/sourceSetComplete"),
        Some(&json!(false))
    );
    assertions::verify_refusal(&source, previous.json())
}

#[test]
fn conflicting_proofs_for_the_same_absent_target_retain_both_authored_origins() -> TestResult {
    const COMPETING_DOCUMENT: &str = "entdata\\competing.entxml";
    let clean = format!(r#"<Entity objectContainer="{TARGET}"/>"#);
    let competing = format!(
        r#"<Entity><a objectContainer="{TARGET}"/><b objectContainer="{UNSUPPORTED}"/></Entity>"#
    );
    let source = source_documents([
        TestEntry::stored(ENTITY_DOCUMENT, clean),
        TestEntry::stored(COMPETING_DOCUMENT, competing),
    ])?;
    let (partial, catalog) = source.diagnostic()?;
    assert!(!partial.summary().export_health().is_complete());
    let failures = entries::failure(&catalog, "lootContainers", "enumeration")?;
    let [failure] = failures.as_slice() else {
        return Err("one deduplicated failed enumeration".into());
    };
    let path = proof::present_text(&catalog, failure, "path", &json!(TARGET), Some(true))?;
    let mut owners = Vec::new();
    for used in proof::uses(&catalog, path)? {
        if used.pointer("/location/field") == Some(&json!("@objectContainer")) {
            owners.push(proof::at(proof::source(&catalog, used)?, "/path/value")?);
        }
    }
    assert!(owners.contains(&&json!(ENTITY_DOCUMENT)));
    assert!(owners.contains(&&json!(COMPETING_DOCUMENT)));
    assert!(matches!(
        build_catalog_from(&source.archive, &source.entry, &fixture::options()),
        Err(CatalogError::IncompleteExport(_))
    ));
    proof::closure(&catalog)
}

const ALIAS_TARGET: &str = "Data/ObjectContainers/PU/synthetic/alias-target.socpak";
const ALIAS_KEY: &str = "data\\objectcontainers\\pu\\synthetic\\alias-target.socpak";

#[test]
fn repaired_authoring_paths_publish_with_original_literal_and_reach_the_exact_target() -> TestResult
{
    let cases = [
        (
            "R:/authoring/Data/ObjectContainers/PU/synthetic/alias-target.socpak",
            "driveQualified",
        ),
        (
            "../../authoring/Data/ObjectContainers/PU/synthetic/alias-target.socpak",
            "parentRelative",
        ),
        (
            "[8:15 PM] saved container path Data/ObjectContainers/PU/synthetic/alias-target.socpak",
            "timestampedAnnotation",
        ),
    ];
    let target = TestContainer::new()
        .entry(TestEntry::stored(
            "entdata/nested.entxml",
            format!(r#"<Entity objectContainer="{ROOT}"/>"#),
        ))
        .build();
    let originals = cases.iter().map(|(selector, _)| {
        TestEntry::stored(
            ENTITY_DOCUMENT,
            format!(r#"<Entity objectContainer="{selector}"/>"#),
        )
    });
    let source =
        source_documents_and_targets(originals, [TestEntry::stored(ALIAS_TARGET, target)])?;
    let published = build_catalog_from(&source.archive, &source.entry, &fixture::options())?;
    assert!(published.summary().export_health().is_complete());
    let catalog: Value = serde_json::from_slice(published.json())?;
    let documents = proof::rows(&catalog, "/declarations/lootEntityContainerDocuments/rows")?;
    assert_eq!(
        documents.len(),
        4,
        "the traversal reads the exactly admitted target once"
    );
    for (wanted, kind) in cases {
        let link = documents
            .iter()
            .filter_map(|document| document.pointer("/links/value/0"))
            .find(|link| link.pointer("/declaredPath/value") == Some(&json!(wanted)))
            .ok_or("original unreplaced authoring literal")?;
        assertions::verify_repair(&catalog, link, kind)?;
    }
    assert!(jsonschema::validator_for(&catalog_schema())?.is_valid(&catalog));
    proof::closure(&catalog)
}

#[test]
fn missing_or_malformed_authoring_paths_remain_unsupported_and_block_publication() -> TestResult {
    for selector in [
        "R:/authoring/Data/ObjectContainers/PU/synthetic/missing/alias-target.socpak",
        "R:/Data/ObjectContainers/Data/ObjectContainers/PU/synthetic/alias-target.socpak",
        "[8:99 PM] saved container path Data/ObjectContainers/PU/synthetic/alias-target.socpak",
    ] {
        let source = source_documents_and_targets(
            [TestEntry::stored(
                ENTITY_DOCUMENT,
                format!(r#"<Entity objectContainer="{selector}"/>"#),
            )],
            [TestEntry::stored(
                ALIAS_TARGET,
                TestContainer::new().build(),
            )],
        )?;
        let (partial, catalog) = source.diagnostic()?;
        assert!(!partial.summary().export_health().is_complete());
        let link = proof::at(
            &catalog,
            "/declarations/lootEntityContainerDocuments/rows/0/links/value/0",
        )?;
        assert_eq!(
            link.pointer("/targetStatus/value"),
            Some(&json!("unsupportedSelector"))
        );
        assert_eq!(link.get("targetLookupRepair"), Some(&Value::Null));
        assert!(matches!(
            build_catalog_from(&source.archive, &source.entry, &fixture::options()),
            Err(CatalogError::IncompleteExport(_))
        ));
    }
    Ok(())
}

#[test]
fn repaired_lookup_with_duplicate_physical_targets_denies_traversal_and_publication() -> TestResult
{
    let selector = "R:/authoring/Data/ObjectContainers/PU/synthetic/alias-target.socpak";
    let target = || TestEntry::stored(ALIAS_TARGET, TestContainer::new().build());
    let source = source_documents_and_targets(
        [TestEntry::stored(
            ENTITY_DOCUMENT,
            format!(r#"<Entity objectContainer="{selector}"/>"#),
        )],
        [target(), target()],
    )?;
    let (partial, catalog) = source.diagnostic()?;
    assert!(!partial.summary().export_health().is_complete());
    let link = proof::at(
        &catalog,
        "/declarations/lootEntityContainerDocuments/rows/0/links/value/0",
    )?;
    assert_eq!(
        link.pointer("/targetStatus/value"),
        Some(&json!("ambiguous"))
    );
    assert_eq!(link.get("targetLookupRepair"), Some(&Value::Null));
    let [first, second] = proof::rows(link, "/targetCandidates/value")? else {
        return Err("both physical candidates must be retained".into());
    };
    assert_ne!(
        first.pointer("/value/entryIndex"),
        second.pointer("/value/entryIndex")
    );
    assert!(matches!(
        build_catalog_from(&source.archive, &source.entry, &fixture::options()),
        Err(CatalogError::IncompleteExport(_))
    ));
    proof::closure(&catalog)
}

fn source(selector: Option<&str>) -> TestResult<Fixture> {
    let document = selector.map(|selector| {
        let xml = format!(
            r#"<Root><Entity CryGUID="00000000-0000-0000-0000-000000000001" pos=" 1, 2, 3 " rot="2,0,0,0"><ObjectContainerComponent objectContainer="{selector}"/></Entity></Root>"#
        );
        TestEntry::stored(ENTITY_DOCUMENT, xml)
    });
    source_documents(document)
}

fn source_documents(documents: impl IntoIterator<Item = TestEntry>) -> TestResult<Fixture> {
    source_documents_and_targets(documents, [])
}

fn source_documents_and_targets(
    documents: impl IntoIterator<Item = TestEntry>,
    targets: impl IntoIterator<Item = TestEntry>,
) -> TestResult<Fixture> {
    let mut container = TestContainer::new().entry(TestEntry::stored(
        "syntheticsystem.xml",
        r#"<ObjectContainer version="14"/>"#,
    ));
    for document in documents {
        container = container.entry(document);
    }
    let mut members = vec![
        TestEntry::stored(ROOT, container.build()).encrypted(),
        TestEntry::stored(fixture::REGISTRY, b"<LayerBackups/>"),
    ];
    members.extend(targets);
    entries::with_starmap(members)
}
