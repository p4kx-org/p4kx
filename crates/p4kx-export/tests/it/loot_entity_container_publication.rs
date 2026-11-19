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
    verify_original(&catalog)?;
    verify_warning(&catalog)?;
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
    verify_warning(&catalog)
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
    verify_refusal(&source, previous.json())
}

fn verify_warning(catalog: &Value) -> TestResult {
    let diagnostics = proof::rows(catalog, "/exportHealth/diagnostics")?;
    assert_eq!(
        diagnostics
            .iter()
            .filter(|item| item["kind"] == "lootContainers"
                && item["code"] == "verifiedMissingReference"
                && item["severity"] == "warning"
                && item["count"] == 1)
            .count(),
        1,
        "the verified absent target emits a nonblocking warning"
    );
    assert_eq!(
        entries::failure(catalog, "lootContainers", "enumeration")?,
        [] as [&Value; 0],
        "the proved absence is not a failed enumeration"
    );
    Ok(())
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
        verify_repair(&catalog, link, kind)?;
    }
    assert!(jsonschema::validator_for(&catalog_schema())?.is_valid(&catalog));
    proof::closure(&catalog)
}

fn verify_repair(catalog: &Value, link: &Value, kind: &str) -> TestResult {
    let status = proof::present(link, "targetStatus", &json!("resolved"), Some(true))?;
    let repair = proof::at(link, "/targetLookupRepair")?;
    let label = proof::present(repair, "kind", &json!(kind), Some(true))?;
    let key = proof::present(repair, "normalizedPath", &json!(ALIAS_KEY), Some(true))?;
    assert_eq!(
        label["sourceUsesRefs"], key["sourceUsesRefs"],
        "the repair kind and virtual key cite the same source evidence"
    );
    assert_eq!(
        status["sourceUsesRefs"], key["sourceUsesRefs"],
        "resolution and its normalized key cite the same original lookup"
    );
    let original_uses = proof::uses(catalog, key)?;
    assert!(
        original_uses.iter().any(|used| {
            used.pointer("/location/field") == Some(&json!("@objectContainer"))
                && used["role"] == "conversion"
        }),
        "the repaired key cites conversion of the original attribute"
    );
    assert!(
        original_uses.iter().any(|used| {
            used.pointer("/location/type") == Some(&json!("archiveDirectoryPaths"))
        }),
        "the repaired key cites the physical archive directory"
    );
    let candidate = proof::at(link, "/targetCandidates/value/0")?;
    verify_physical(catalog, candidate, ALIAS_TARGET)
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
        status["sourceSetComplete"], true,
        "the status cites a complete lookup"
    );
    assert_eq!(
        candidates["sourceSetComplete"], true,
        "the empty candidate set has complete source evidence"
    );
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
