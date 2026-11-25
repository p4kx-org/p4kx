// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::build_catalog;
use p4kx_export::catalog_queries::CatalogRecordSelection;
use p4kx_export::catalog_queries::equipment::{AlignmentDependencyState, AlignmentRole};
use serde_json::{Value, json};

use super::super::{Layout, OWNER, database};
use super::check_default;
use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::loadouts_support;
use crate::sdk_full_proof as full_proof;

#[test]
fn direct_loadout_aligns_exact_reached_roles_to_the_unfiltered_producer() -> TestResult {
    let (_folder, archive) = common::open(&database(Layout::Complete), DcbOrigin::Encrypted)?;
    let producer: Value = serde_json::from_slice(
        build_catalog(&archive, common::options().with_full_provenance())?.json(),
    )?;
    let snapshot = full_proof::open(&archive, full_proof::NO_LANGUAGES, None)?
        .read_loadouts(CatalogRecordSelection::RecordIndex(0), None)?;
    let [owner] = snapshot.rows.as_slice() else {
        return Err("one selected Loadout root".into());
    };
    assert_eq!(
        owner.record_index(),
        0,
        "position belongs to the selected original Record"
    );
    let rows = serde_json::to_value(&snapshot.rows)?;
    assert_eq!(rows[0], *loadouts_support::loadout(&producer, OWNER)?);
    let alignment = &snapshot.dependencies.alignment;
    assert_eq!(
        alignment.len(),
        3,
        "only root Port and reached child Port and Attachment"
    );
    assert!(
        alignment
            .iter()
            .all(|dependency| matches!(dependency.state, AlignmentDependencyState::Complete))
    );
    assert!(
        alignment
            .iter()
            .any(|dependency| dependency.record_index == 1
                && matches!(dependency.role, AlignmentRole::Attachment))
    );
    let projected = json!({
        "loadouts": rows,
        "provenance": snapshot.source_inventory,
    });
    check_default(&projected, "projected")
}
