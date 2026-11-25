// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogError, build_catalog_from, catalog_schema};
use p4kx_testkit::{TestContainer, TestElement, TestEntry};
use serde_json::{Value, json};

use crate::health_origin_assertions as proof;
use crate::health_origin_entry_source as entries;
use crate::health_origin_fixture::{self as fixture, Fixture, TestResult};

const ROOT: &str = "Data/ObjectContainers/PU/system/synthetic/syntheticsystem.socpak";
const TARGET: &str = "Data/ObjectContainers/PU/synthetic/destination.socpak";

#[derive(Clone, Copy)]
enum Target {
    Unreadable,
    Unparsed,
    Empty,
}

#[test]
fn unique_unavailable_targets_keep_original_children_and_refuse_normal_publication() -> TestResult {
    let complete = source(Target::Empty)?;
    let previous = build_catalog_from(
        &complete.archive,
        &complete.entry,
        fixture::options().with_full_provenance(),
    )?;
    for target in [Target::Unreadable, Target::Unparsed] {
        let source = source(target)?;
        let (partial, catalog) = source.diagnostic()?;
        assert!(
            !partial.summary().export_health().is_complete(),
            "real required failure remains"
        );
        let rows = proof::rows(&catalog, "/placements/unresolvedContainerLinks/rows")?;
        let [first, second] = rows else {
            return Err(
                "both reached original Child declarations survive the target failure".into(),
            );
        };
        verify(&catalog, first, target)?;
        verify(&catalog, second, target)?;
        assert_ne!(
            first["instanceId"], second["instanceId"],
            "distinct physical Child occurrences"
        );
        assert_ne!(
            first["declaration"]["childNodeIndex"], second["declaration"]["childNodeIndex"],
            "each original node is distinct"
        );
        assert_eq!(
            unique_target(first)?,
            unique_target(second)?,
            "same original physical target"
        );
        assert!(
            jsonschema::validator_for(&catalog_schema())?.is_valid(&catalog),
            "actual diagnostic wire validates"
        );
        proof::closure(&catalog)?;
        assert!(
            matches!(
                build_catalog_from(
                    &source.archive,
                    &source.entry,
                    fixture::options().with_full_provenance()
                ),
                Err(CatalogError::IncompleteExport(_))
            ),
            "original target failure is still required"
        );
        super::verify_writer_refusal(&source, previous.json())?;
    }
    Ok(())
}

#[test]
fn a_successfully_parsed_empty_target_stays_resolved_without_orphan_declaration_uses() -> TestResult
{
    let source = source(Target::Empty)?;
    let built = build_catalog_from(
        &source.archive,
        &source.entry,
        fixture::options().with_full_provenance(),
    )?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    assert!(
        built.summary().export_health().is_complete(),
        "empty readable body is a success"
    );
    assert!(
        proof::rows(&catalog, "/placements/unresolvedContainerLinks/rows")?.is_empty(),
        "an empty parsed target is not unavailable"
    );
    assert_eq!(
        proof::rows(&catalog, "/placements/containerInstances/rows")?.len(),
        3,
        "root and both distinct occurrences of the readable empty target remain"
    );
    assert!(
        proof::rows(&catalog, "/provenance/uses")?
            .iter()
            .all(|used| {
                used.pointer("/location/field").and_then(Value::as_str) != Some("@custom")
            }),
        "discarded deferred packets register no raw-attribute source uses"
    );
    proof::closure(&catalog)
}

fn unique_target(row: &Value) -> TestResult<&Value> {
    let [candidate] = proof::rows(row, "/targetCandidates/value")? else {
        return Err("one physical target".into());
    };
    Ok(&candidate["value"])
}

fn source(target: Target) -> TestResult<Fixture> {
    let mut children = TestElement::new("ChildObjectContainers");
    for pos in ["1,2,3", "4,5,6"] {
        children = children.child(
            TestElement::new("Child")
                .attribute("name", TARGET)
                .attribute("external", "true")
                .attribute("pos", pos)
                .attribute("rot", "2,0,0,0")
                .attribute("custom", "retained"),
        );
    }
    let root = TestElement::new("ObjectContainer").child(children).to_xml();
    let container = TestContainer::new()
        .entry(TestEntry::stored("syntheticsystem.xml", root))
        .build();
    let bytes = match target {
        Target::Unreadable => b"not a ZIP container".to_vec(),
        Target::Unparsed => TestContainer::new()
            .entry(TestEntry::stored("destination.xml", b"<broken>".as_slice()))
            .build(),
        Target::Empty => TestContainer::new()
            .entry(TestEntry::stored(
                "destination.xml",
                b"<ObjectContainer/>".as_slice(),
            ))
            .build(),
    };
    entries::with_starmap(vec![
        TestEntry::stored(ROOT, container).encrypted(),
        TestEntry::stored(TARGET, bytes),
        TestEntry::stored(fixture::REGISTRY, b"<LayerBackups/>"),
    ])
}

fn verify(catalog: &Value, row: &Value, target: Target) -> TestResult {
    let status = proof::present(row, "targetStatus", &json!("resolved"), Some(true))?;
    let read = match target {
        Target::Unreadable => "unreadable",
        Target::Unparsed => "unparsed",
        Target::Empty => return Err("readable controls must have no unavailable row".into()),
    };
    let read_status = proof::present(row, "targetReadStatus", &json!(read), Some(true))?;
    let candidates = proof::rows(row, "/targetCandidates/value")?;
    assert_eq!(candidates.len(), 1, "directory resolution really is unique");
    assert!(
        proof::uses(catalog, read_status)?
            .iter()
            .all(|used| used["role"] == "selector"),
        "the read outcome retains selectors rather than claiming failed target payload"
    );
    assert!(
        status["sourceUsesRefs"]
            .as_array()
            .ok_or("directory selector uses")?
            .iter()
            .all(|used| read_status["sourceUsesRefs"]
                .as_array()
                .is_some_and(|refs| refs.contains(used))),
        "actual directory lookup remains a contributor to the availability result"
    );
    let declaration = &row["declaration"];
    proof::present(declaration, "declaredPath", &json!(TARGET), Some(true))?;
    let attributes = declaration["attributes"]
        .as_array()
        .ok_or("original authored attributes")?;
    let custom = attributes
        .iter()
        .find(|attribute| {
            attribute.pointer("/name/value").and_then(Value::as_str) == Some("custom")
        })
        .ok_or("original extra authored value")?;
    proof::present(custom, "value", &json!("retained"), Some(true))?;
    assert_eq!(
        proof::at(declaration, "/localRotation/value")?,
        &json!([2.0, 0.0, 0.0, 0.0]),
        "stored local rotation survives"
    );
    Ok(())
}
