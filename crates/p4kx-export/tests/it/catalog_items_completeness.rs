// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogOptions, Filter, Kind};
use serde_json::{Value, json};

use super::{TestResult, catalog_with, class_names, options};

#[test]
fn every_kind_states_its_completeness() -> TestResult {
    let (_, catalog) = catalog_with(&options())?;
    assert_eq!(catalog["completeness"], expected_completeness(), "complete");
    let filter = Filter::new("*/spaceships/*");
    let (built, catalog) = catalog_with(&CatalogOptions {
        filter: Some(&filter),
        ..options()
    })?;
    assert_eq!(
        catalog["completeness"]["items"],
        json!({"status": "partial", "filter": "*/spaceships/*"}),
        "partial"
    );
    assert_eq!(
        catalog["completeness"]["clientProfileSettings"],
        json!({"status": "partial", "filter": "*/spaceships/*"}),
        "client profile selection respects the Archive filter"
    );
    assert_eq!(class_names(&catalog, "items"), Vec::<&str>::new(), "none");
    assert_eq!(class_names(&catalog, "ships"), ["ACAS_Ship"], "the ship");
    assert_eq!(
        catalog["completeness"]["tags"],
        json!({"status": "complete"}),
        "the global Tag taxonomy retains its full selection scope"
    );
    assert!(
        built
            .summary()
            .kinds()
            .iter()
            .all(|entry| entry.complete == matches!(entry.kind, Kind::Tags | Kind::ShopInventories)),
        "the Tag taxonomy and physical shop inventory selection are global"
    );
    Ok(())
}

fn expected_completeness() -> Value {
    Kind::ALL
        .into_iter()
        .map(Kind::name)
        .chain(["shipStoreSlugs"])
        .map(|kind| (kind.to_owned(), json!({"status": "complete"})))
        .collect::<serde_json::Map<_, _>>()
        .into()
}
