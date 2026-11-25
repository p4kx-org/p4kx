// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::planet_resources::{
    PlanetResourceSelection, read_planet_resources,
};
use p4kx_extraction::Archive;
use p4kx_testkit::{ArchiveBuilder, TestEntry};

use super::{Route, TestResult};

pub(super) fn read() -> TestResult<Route> {
    const RESOURCE: &str = "Data/Libs/Planets/resources/sample.resource";
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("planet_resource_fixture.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(
            RESOURCE,
            r#"{"guid":"aaaaaaaa-0000-0000-0000-000000000001","name":"fixture","bMinable":true}"#,
        ))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    let snapshot = read_planet_resources(
        &archive,
        PlanetResourceSelection::ArchivePath(RESOURCE),
        None,
    )?;
    Route::queried_rows("/definitions/planetResources", &snapshot.rows, None)
}
