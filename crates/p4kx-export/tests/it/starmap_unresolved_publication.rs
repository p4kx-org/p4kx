// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::Cell;
use std::fs;

use p4kx_export::{
    CatalogError, CatalogFile, Compression, build_catalog_from, catalog_schema, write_catalog_from,
};
use p4kx_testkit::{TestContainer, TestElement, TestEntry};
use serde_json::{Value, json};

use crate::health_origin_assertions as proof;
use crate::health_origin_entry_source as entries;
use crate::health_origin_fixture::{self as fixture, Fixture, Input, TestResult};

const ROOT: &str = "Data/ObjectContainers/PU/system/synthetic/syntheticsystem.socpak";
const TARGET: &str = "Data/ObjectContainers/PU/synthetic/absent.socpak";

#[test]
fn an_unresolved_child_is_retained_diagnostically_and_refuses_normal_before_creation() -> TestResult
{
    let complete = source(false)?;
    let previous = build_catalog_from(&complete.archive, &complete.entry, &fixture::options())?;
    assert!(
        previous.summary().export_health().is_complete(),
        "actual complete counterpart"
    );
    let source = source(true)?;
    let (partial, catalog) = source.diagnostic()?;
    assert!(
        !partial.summary().export_health().is_complete(),
        "configured target still blocks complete export"
    );
    verify_original_row(&catalog)?;
    let validator = jsonschema::validator_for(&catalog_schema())?;
    assert!(
        validator.is_valid(&catalog),
        "runtime schema accepts the actual diagnostic row"
    );
    proof::closure(&catalog)?;
    assert!(
        matches!(
            build_catalog_from(&source.archive, &source.entry, &fixture::options()),
            Err(CatalogError::IncompleteExport(_))
        ),
        "public in-memory producer refuses the same source"
    );
    verify_writer_refusal(&source, previous.json())
}

fn source(missing: bool) -> TestResult<Fixture> {
    let mut children = TestElement::new("ChildObjectContainers");
    if missing {
        children = children.child(
            TestElement::new("Child")
                .attribute("external", "true")
                .attribute("name", TARGET)
                .attribute("pos", " 1, 2, 3 ")
                .attribute("rot", "2,0,0,0"),
        );
    }
    let xml = TestElement::new("ObjectContainer")
        .attribute("version", "14")
        .child(children)
        .to_xml();
    let container = TestContainer::new()
        .entry(TestEntry::stored("syntheticsystem.xml", xml.into_bytes()))
        .build();
    Fixture::open(
        &fixture::empty_database(),
        Input::Plain,
        vec![TestEntry::stored(ROOT, container).encrypted()],
    )
}

fn verify_original_row(catalog: &Value) -> TestResult {
    let rows = proof::rows(catalog, "/placements/unresolvedContainerLinks/rows")?;
    let [row] = rows else {
        return Err("one exact missing external Child occurrence".into());
    };
    let status = proof::present(row, "targetStatus", &json!("notFound"), Some(true))?;
    let path = proof::present(
        &row["declaration"],
        "declaredPath",
        &json!(TARGET),
        Some(true),
    )?;
    proof::present(
        &row["declaration"],
        "localPosition",
        &json!([1.0, 2.0, 3.0]),
        Some(true),
    )?;
    proof::present(
        &row["declaration"],
        "localRotation",
        &json!([2.0, 0.0, 0.0, 0.0]),
        Some(true),
    )?;
    proof::present(row, "worldPosition", &json!([1.0, 2.0, 3.0]), Some(true))?;
    let targets = proof::present(row, "targetCandidates", &json!([]), Some(true))?;
    let selector_uses = proof::uses(catalog, path)?;
    assert!(
        selector_uses.iter().any(|used| {
            used.pointer("/location/field") == Some(&json!("@name"))
                && used.get("role") == Some(&json!("payload"))
        }),
        "path keeps its original XML payload"
    );
    assert_eq!(
        status["sourceUsesRefs"], targets["sourceUsesRefs"],
        "lookup result and candidate set cite the same actual inputs"
    );
    let lookup_uses = proof::uses(catalog, status)?;
    assert!(
        lookup_uses.iter().any(|used| {
            used.pointer("/location/type") == Some(&json!("archiveDirectoryPaths"))
        }),
        "empty candidate set cites the actual root directory survey"
    );
    verify_parent(catalog, row)?;

    let failures = entries::failure(catalog, "starmapObjects", "enumeration")?;
    assert_eq!(
        failures.len(),
        1,
        "actual missing target retains its failure evidence"
    );
    let attributes = proof::rows(row, "/declaration/attributes")?;
    assert!(
        attributes.iter().any(|attribute| {
            attribute.pointer("/name/value").and_then(Value::as_str) == Some("pos")
                && attribute.pointer("/value/value").and_then(Value::as_str) == Some(" 1, 2, 3 ")
        }),
        "raw authored position survives alongside its parsed value"
    );
    Ok(())
}

fn verify_parent(catalog: &Value, row: &Value) -> TestResult {
    let resolved = proof::rows(catalog, "/placements/containerInstances/rows")?;
    let [parent] = resolved else {
        return Err("only the actual root container is resolved".into());
    };
    let id = parent
        .pointer("/instanceId/value")
        .ok_or("resolved parent instance key")?;
    let container = parent
        .pointer("/container/value")
        .ok_or("resolved parent source")?;
    assert_eq!(
        row.pointer("/parentInstanceId/value"),
        Some(id),
        "unresolved occurrence joins its exact resolved parent"
    );
    assert_eq!(
        row.pointer("/declaringContainer/value"),
        Some(container),
        "declaration belongs to that parent's physical container"
    );
    Ok(())
}

fn verify_writer_refusal(source: &Fixture, previous: &[u8]) -> TestResult {
    for existing in [false, true] {
        let path = source
            .folder
            .path()
            .join(format!("catalog-{existing}.json"));
        if existing {
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
        let Err(CatalogError::IncompleteExport(refusal)) = result else {
            return Err("public writer must retain the actual typed incomplete refusal".into());
        };
        assert!(
            !refusal.health().is_complete(),
            "normal refusal carries incomplete health"
        );
        assert!(
            !called.get(),
            "the source refusal happens before output callback"
        );
        if existing {
            assert_eq!(fs::read(&path)?, previous, "last complete Catalog survives");
        } else {
            assert!(!path.exists(), "fresh output remains absent");
        }
    }
    Ok(())
}

#[path = "starmap_unique_target_publication.rs"]
mod unique_target;
