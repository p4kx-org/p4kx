// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogOptions, Filter, build_catalog};
use p4kx_testkit::cargo_grid_facility_database;
use serde_json::{Value, json};

use super::{INVENTORY_CONFIGURATIONS, INVENTORY_CONTAINERS, only_grid};
use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::health_origin_assertions as proof;

#[test]
fn cargo_grid_filtered_container_keeps_identity_without_projecting_capacity() -> TestResult {
    let (_directory, archive) = common::open(&cargo_grid_facility_database(), DcbOrigin::Plain)?;
    let filter = Filter::new("entities/scitem/fixture/cargo_grid");
    let options = CatalogOptions {
        filter: Some(&filter),
        ..common::options()
    };
    let catalog: Value =
        serde_json::from_slice(build_catalog(&archive, options.with_full_provenance())?.json())?;
    let capacity = proof::at(only_grid(&catalog)?, "/capacity")?;
    assert_eq!(
        proof::at(capacity, "/status")?,
        "containerOutsideSelection",
        "the physically resolved definition has an explicit filter disposition"
    );
    proof::present(capacity, "containerRecordIndex", &json!(2), Some(false))?;
    let scu = proof::cell(capacity, "scu", "unreadable")?;
    assert_eq!(
        proof::at(scu, "/sourceSetComplete")?,
        false,
        "the filtered operands are absent from the actual contributor set"
    );
    assert!(
        proof::rows(&catalog, INVENTORY_CONTAINERS)?.is_empty(),
        "the capacity projection respects the selected inventory table"
    );
    let [storage] = proof::rows(&catalog, INVENTORY_CONFIGURATIONS)? else {
        return Err("one selected storage component".into());
    };
    let record_path = proof::at(storage, "/containerParams/value/recordPath")?;
    let path_uses = proof::rows(record_path, "/sourceUsesRefs")?;
    let capacity_uses = proof::rows(scu, "/sourceUsesRefs")?;
    assert!(
        !path_uses.is_empty() && path_uses.iter().all(|used| capacity_uses.contains(used)),
        "filter disposition retains the exact original target path evidence"
    );
    proof::closure(&catalog)
}
