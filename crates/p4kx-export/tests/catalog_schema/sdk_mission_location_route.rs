// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::UNIX_EPOCH;

use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogRecordSelection, with_mission_location_templates_from,
};
use p4kx_export::{CatalogOptions, CatalogPublicationMode, build_catalog_from};
use p4kx_extraction::{Archive, DataForgeEntry, InputScope};
use p4kx_testkit::{ArchiveBuilder, TestEntry};
use serde_json::Value;

use super::super::MISSION_LOCATION_TEMPLATES;
use super::{Route, TestResult};

#[path = "../it/mission_location_sdk_fixture.rs"]
mod fixture;

pub(super) fn read() -> TestResult<Route> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("synthetic-mission-location.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", fixture::entity_cluster().build()).encrypted())
        .write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?;
    let entry = DataForgeEntry::read(&archive)?;
    let options = CatalogOptions {
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..CatalogOptions::new(UNIX_EPOCH)
    };
    let catalog: Value =
        serde_json::from_slice(build_catalog_from(&archive, &entry, &options)?.json())?;
    with_mission_location_templates_from(
        &archive,
        &entry,
        CatalogRecordSelection::All,
        CatalogLanguageSelection::AllAvailable,
        None,
        |snapshot| {
            let [row] = snapshot.rows.as_slice() else {
                return Err("exactly one authored location template".into());
            };
            let expected = catalog
                .pointer(MISSION_LOCATION_TEMPLATES)
                .ok_or("canonical mission location template array")?;
            assert_eq!(
                &serde_json::to_value(&snapshot.rows)?,
                expected,
                "typed location template rows match the exact Catalog array"
            );
            let cluster = row.entity_cluster_member();
            let origin = cluster.origin();
            assert_eq!(
                origin.contains_encrypted_source(),
                Some(true),
                "the cluster link came from the encrypted DataForge Entry"
            );
            assert!(
                cluster.value().is_some()
                    && origin.source_set_complete()
                    && snapshot.source_inventory.iter_uses().any(|used| {
                        used.scope() == InputScope::Payload
                            && origin.source_uses_refs().contains(used.id())
                    }),
                "the authored cluster reference keeps its original payload evidence"
            );
            Route::queried_rows(
                MISSION_LOCATION_TEMPLATES,
                &snapshot.rows,
                "encrypted synthetic producer parity available; installed producer parity open",
            )
        },
    )?
}
