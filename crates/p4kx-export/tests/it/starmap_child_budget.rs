// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::Cell;
use std::fs;

use p4kx_export::{
    CatalogError, CatalogFile, CatalogOptions, CatalogPublicationMode, Compression,
    StarmapTraversalBudget, build_catalog_from, write_catalog_from,
};
use p4kx_extraction::CancelToken;
use p4kx_testkit::{TestContainer, TestElement, TestEntry};
use serde_json::{Value, json};

use crate::health_origin_assertions as proof;
use crate::health_origin_fixture::{self as fixture, Fixture, Input, TestResult};

const ROOT: &str = "Data/ObjectContainers/PU/system/synthetic/syntheticsystem.socpak";
const SHARED: &str = "Data/ObjectContainers/PU/system/synthetic/shared.socpak";

#[test]
fn selected_child_budgets_preserve_partial_health_and_last_complete_output() -> TestResult {
    let source = source()?;
    let complete = build_catalog_from(
        &source.archive,
        &source.entry,
        fixture::options().with_full_provenance(),
    )?;
    assert!(complete.summary().export_health().is_complete());
    verify_complete_positions(complete.json())?;
    for (budget, retained) in [
        (StarmapTraversalBudget::new().with_source_children(2), 3),
        (
            StarmapTraversalBudget::new().with_placed_children_per_system(3),
            3,
        ),
        (StarmapTraversalBudget::new().with_source_children(0), 1),
        (
            StarmapTraversalBudget::new().with_placed_children_per_system(0),
            1,
        ),
    ] {
        let options = CatalogOptions {
            starmap_budget: budget,
            ..fixture::options()
        };
        assert!(matches!(
            build_catalog_from(
                &source.archive,
                &source.entry,
                options.with_full_provenance()
            ),
            Err(CatalogError::IncompleteExport(_))
        ));
        let diagnostic = CatalogOptions {
            publication_mode: CatalogPublicationMode::DiagnosticPartial,
            ..options
        };
        let partial = build_catalog_from(
            &source.archive,
            &source.entry,
            diagnostic.with_full_provenance(),
        )?;
        assert!(!partial.summary().export_health().is_complete());
        let rows: Value = serde_json::from_slice(partial.json())?;
        assert_eq!(
            proof::rows(&rows, "/placements/containerInstances/rows")?.len(),
            retained
        );
        proof::closure(&rows)?;
        verify_refusal(&source, &options, complete.json())?;
    }
    Ok(())
}

fn verify_complete_positions(json: &[u8]) -> TestResult {
    let catalog: Value = serde_json::from_slice(json)?;
    let instances = proof::rows(&catalog, "/placements/containerInstances/rows")?;
    let positions = instances
        .iter()
        .map(|row| {
            row.pointer("/worldPosition/value")
                .ok_or("missing world position")
        })
        .collect::<Result<Vec<_>, _>>()?;
    assert_eq!(
        positions,
        [
            &json!([0.0, 0.0, 0.0]),
            &json!([10.0, 0.0, 0.0]),
            &json!([20.0, 0.0, 0.0]),
        ],
        "one physical tile keeps both independently authored occurrences"
    );
    proof::closure(&catalog)
}

#[test]
fn zero_child_allowances_accept_an_empty_system() -> TestResult {
    let source = Fixture::open(
        &fixture::empty_database(),
        Input::Plain,
        vec![container(
            ROOT,
            "syntheticsystem.xml",
            TestElement::new("ChildObjectContainers"),
        )],
    )?;
    let options = CatalogOptions {
        starmap_budget: StarmapTraversalBudget::new()
            .with_source_children(0)
            .with_placed_children_per_system(0),
        ..fixture::options()
    };
    let complete = build_catalog_from(
        &source.archive,
        &source.entry,
        options.with_full_provenance(),
    )?;
    assert!(
        complete.summary().export_health().is_complete(),
        "zero work can satisfy a zero allowance"
    );
    let catalog: Value = serde_json::from_slice(complete.json())?;
    assert_eq!(
        proof::rows(&catalog, "/placements/containerInstances/rows")?.len(),
        1,
        "the empty system root remains represented"
    );
    proof::closure(&catalog)
}

#[test]
fn cancellation_precedes_an_exhausted_child_allowance() -> TestResult {
    let source = source()?;
    let cancel = CancelToken::default();
    cancel.cancel();
    let options = CatalogOptions {
        starmap_budget: StarmapTraversalBudget::new()
            .with_source_children(0)
            .with_placed_children_per_system(0),
        cancel: Some(&cancel),
        ..fixture::options()
    };
    assert!(
        matches!(
            build_catalog_from(
                &source.archive,
                &source.entry,
                options.with_full_provenance()
            ),
            Err(CatalogError::Cancelled)
        ),
        "cancellation never becomes a partial quota result"
    );
    Ok(())
}

fn source() -> TestResult<Fixture> {
    let declarations = ["10,0,0", "20,0,0"].into_iter().fold(
        TestElement::new("ChildObjectContainers"),
        |children, position| {
            children.child(
                TestElement::new("Child")
                    .attribute("external", "true")
                    .attribute("name", SHARED)
                    .attribute("pos", position),
            )
        },
    );
    let shared = TestElement::new("ChildObjectContainers")
        .child(TestElement::new("Child").attribute("pos", "1,0,0"));
    Fixture::open(
        &fixture::empty_database(),
        Input::Plain,
        vec![
            container(ROOT, "syntheticsystem.xml", declarations),
            container(SHARED, "shared.xml", shared),
        ],
    )
}

fn container(path: &str, name: &str, children: TestElement) -> TestEntry {
    let xml = TestElement::new("ObjectContainer").child(children).to_xml();
    TestEntry::stored(
        path,
        TestContainer::new()
            .entry(TestEntry::stored(name, xml.into_bytes()))
            .build(),
    )
    .encrypted()
}

fn verify_refusal(source: &Fixture, options: &CatalogOptions<'_>, previous: &[u8]) -> TestResult {
    for existing in [false, true] {
        let path = source.folder.path().join(format!("budget-{existing}.json"));
        if existing {
            fs::write(&path, previous)?;
        }
        let called = Cell::new(false);
        let result = write_catalog_from(
            &source.archive,
            &source.entry,
            options.with_full_provenance(),
            Compression::None,
            || {
                called.set(true);
                CatalogFile::create(&path)
            },
        );
        let Err(CatalogError::IncompleteExport(refusal)) = result else {
            return Err("selected quota must refuse publication before creating output".into());
        };
        assert!(
            !refusal.health().is_complete(),
            "budget exhaustion remains incomplete"
        );
        assert!(!called.get(), "refusal precedes output creation");
        if existing {
            assert_eq!(
                fs::read(path)?,
                previous,
                "the last complete generation remains"
            );
        } else {
            assert!(!path.exists(), "refusal creates no new output");
        }
    }
    Ok(())
}
