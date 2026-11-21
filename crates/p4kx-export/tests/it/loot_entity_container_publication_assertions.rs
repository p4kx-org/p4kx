// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::Cell;
use std::fs;

use p4kx_export::{CatalogError, CatalogFile, Compression, write_catalog_from};
use serde_json::{Value, json};

use super::{ALIAS_KEY, ALIAS_TARGET, ENTITY_DOCUMENT, ROOT, TARGET};
use crate::health_origin_assertions as proof;
use crate::health_origin_entry_source as entries;
use crate::health_origin_fixture::{self as fixture, Fixture, TestResult};

pub(super) fn verify_warning(catalog: &Value) -> TestResult {
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

pub(super) fn verify_repair(catalog: &Value, link: &Value, kind: &str) -> TestResult {
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

pub(super) fn verify_original(catalog: &Value) -> TestResult {
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

pub(super) fn verify_refusal(source: &Fixture, previous: &[u8]) -> TestResult {
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
