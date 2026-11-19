// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::Cell;
use std::fs;

use p4kx_export::{
    CatalogError, CatalogFile, Compression, build_catalog_from, catalog_schema, write_catalog_from,
};
use p4kx_testkit::{
    DataForgeBuilder, TestContainer, TestElement, TestEntry, TestRecord, TestStruct, TestType,
    TestValue,
};
use serde_json::{Value, json};

use crate::health_origin_assertions as proof;
use crate::health_origin_entry_source as entries;
use crate::health_origin_fixture::{self as fixture, Fixture, Input, TestResult};

const ROOT: &str = "Data/ObjectContainers/PU/system/synthetic/syntheticsystem.socpak";
const TARGET: &str = "Data/ObjectContainers/PU/synthetic/absent.socpak";
const PRESET_GUID: u128 = 0x5713;
const PRESET_TYPE: &str = "HarvestableProviderPreset";
const ENTRY_CONSUMERS: [&str; 3] = ["starmapObjects", "harvestableLocations", "lootContainers"];

#[test]
fn a_proved_missing_child_is_published_with_original_proof_and_warning() -> TestResult {
    let source = source(&[TARGET])?;
    let built = build_catalog_from(&source.archive, &source.entry, &fixture::options())?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    assert!(built.summary().export_health().is_complete());
    assert_eq!(catalog["exportHealth"]["publicationMode"], "normal");
    verify_original_row(&catalog)?;
    verify_absence_warning(&catalog)?;
    assert!(jsonschema::validator_for(&catalog_schema())?.is_valid(&catalog));
    proof::closure(&catalog)?;

    let (summary, bytes) = write_catalog_from(
        &source.archive,
        &source.entry,
        &fixture::options(),
        Compression::None,
        || Ok(Vec::new()),
    )?;
    assert!(summary.export_health().is_complete());
    assert_eq!(bytes, built.json());
    Ok(())
}

#[test]
fn an_unsupported_child_still_prevents_publication_beside_a_proved_absence() -> TestResult {
    const UNSUPPORTED: &str = "C:/authoring/unsearchable.socpak";
    let complete = source(&[])?;
    let previous = build_catalog_from(&complete.archive, &complete.entry, &fixture::options())?;
    assert!(previous.summary().export_health().is_complete());

    let source = source(&[TARGET, UNSUPPORTED])?;
    let (partial, catalog) = source.diagnostic()?;
    assert!(!partial.summary().export_health().is_complete());
    let rows = proof::rows(&catalog, "/placements/unresolvedContainerLinks/rows")?;
    let [proved, unsupported] = rows else {
        return Err("both original Children must survive diagnostic publication".into());
    };
    proof::present(proved, "targetStatus", &json!("notFound"), Some(true))?;
    assert_eq!(unsupported["targetStatus"]["value"], "unsupportedSelector");
    assert_eq!(unsupported["targetStatus"]["sourceSetComplete"], false);
    for kind in ENTRY_CONSUMERS {
        assert_eq!(
            entries::failure(&catalog, kind, "enumeration")?.len(),
            1,
            "{kind} retains the unsupported selector as a failed lookup"
        );
    }
    verify_absence_warning(&catalog)?;
    verify_writer_refusal(&source, previous.json())
}

fn verify_absence_warning(catalog: &Value) -> TestResult {
    let diagnostics = proof::rows(catalog, "/exportHealth/diagnostics")?;
    for kind in ENTRY_CONSUMERS {
        let findings: Vec<_> = diagnostics
            .iter()
            .filter(|diagnostic| {
                diagnostic["kind"] == kind && diagnostic["code"] == "verifiedMissingReference"
            })
            .collect();
        let [warning] = findings.as_slice() else {
            return Err(format!("one verified absence warning for {kind}").into());
        };
        assert_eq!(warning["severity"], "warning", "{kind} verified absence");
        assert_eq!(
            warning["sourceRole"], "dependency",
            "{kind} depends on the original Starmap lookup"
        );
        assert_eq!(
            warning["countUnit"], "entryOperations",
            "{kind} counts directory lookups"
        );
        assert_eq!(warning["count"], 1, "{kind} counts the lookup once");
    }
    Ok(())
}

fn source(targets: &[&str]) -> TestResult<Fixture> {
    let mut children = TestElement::new("ChildObjectContainers");
    for target in targets {
        children = children.child(
            TestElement::new("Child")
                .attribute("external", "true")
                .attribute("name", *target)
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
        &provider_database(),
        Input::Plain,
        vec![TestEntry::stored(ROOT, container).encrypted()],
    )
}

fn provider_database() -> Vec<u8> {
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("HarvestableElementGroup"))
        .structure(TestStruct::new("HarvestableAreaPreset"))
        .structure(
            TestStruct::new(PRESET_TYPE)
                .array(
                    "harvestableGroups",
                    TestType::Class("HarvestableElementGroup".to_owned()),
                )
                .array("areas", TestType::Class("HarvestableAreaPreset".to_owned())),
        )
        .record(
            TestRecord::new(
                PRESET_GUID,
                PRESET_TYPE,
                "HarvestableProviderPreset.Synthetic",
                "libs/foundry/records/harvestable/providerpresets/synthetic.xml",
            )
            .value("harvestableGroups", TestValue::Array(Vec::new()))
            .value("areas", TestValue::Array(Vec::new())),
        )
        .build()
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

    for kind in ENTRY_CONSUMERS {
        assert!(
            entries::failure(catalog, kind, "enumeration")?.is_empty(),
            "{kind} accounts for verified absence with the published row and warning"
        );
    }
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
