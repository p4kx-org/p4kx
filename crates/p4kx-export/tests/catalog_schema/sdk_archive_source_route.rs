// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;
use std::time::UNIX_EPOCH;

use p4kx_channels::{PublicPatches, channel_name};
use p4kx_export::{CatalogOptions, build_catalog};
use p4kx_extraction::{Archive, utc_rfc3339};
use p4kx_testkit::{ArchiveBuilder, DataForgeBuilder, TestEntry, write_build_manifest};
use schemars::JsonSchema;
use serde::Serialize;
use serde_json::Value;

use super::{Route, TestResult};

const CHANNEL: &str = "/source/channel";
const SIZE: &str = "/source/archiveSize";
const MODIFIED: &str = "/source/archiveModified";
const BRANCH: &str = "/source/build/branch";
const VERSION: &str = "/source/build/version";
const CHANGE: &str = "/source/build/changeNumber";
const BUILD_ID: &str = "/source/build/buildId";
const PUBLIC_PATCH: &str = "/source/build/publicPatch";
const CHANGE_NUMBER: &str = "10012345";

pub(in super::super) const DATASETS: &[&str] = &[
    CHANNEL,
    SIZE,
    MODIFIED,
    BRANCH,
    VERSION,
    CHANGE,
    BUILD_ID,
    PUBLIC_PATCH,
];

pub(super) fn read() -> TestResult<[Route; 8]> {
    let folder = tempfile::tempdir()?;
    let channel = folder.path().join("PTU");
    std::fs::create_dir(&channel)?;
    let path = channel.join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(
            "Data/Game2.dcb",
            DataForgeBuilder::new().version(8).build(),
        ))
        .write_to(&path)?;
    check_missing_build(&path)?;
    write_build_manifest(
        &channel,
        "sc-alpha-fixture",
        "4.4.162.12345",
        CHANGE_NUMBER,
        "0123456789abcdef",
    )?;
    let archive = Archive::open(path)?;
    let build = archive.build().ok_or("the original Build manifest")?;
    let patches = read_patches(folder.path(), "matched", "(SC PTU 4.4.2-PTU.10012345)\n")?;
    let public_patch = patches.of(build);
    assert_eq!(
        public_patch,
        Some("4.4.2"),
        "use the launcher version with the exact change number"
    );
    check_mismatched_patch(&archive, folder.path())?;
    let catalog = catalog(&archive, Some(&patches))?;
    Ok([
        verify(&catalog, CHANNEL, &channel_name(archive.path()))?,
        verify(&catalog, SIZE, &archive.file_len())?,
        verify(
            &catalog,
            MODIFIED,
            &archive.file_modified().map(utc_rfc3339),
        )?,
        verify(&catalog, BRANCH, &build.branch())?,
        verify(&catalog, VERSION, &build.version())?,
        verify(&catalog, CHANGE, &build.change_number())?,
        verify(&catalog, BUILD_ID, &build.build_id())?,
        verify(&catalog, PUBLIC_PATCH, &public_patch)?,
    ])
}

fn check_missing_build(path: &Path) -> TestResult {
    let archive = Archive::open(path)?;
    assert!(archive.build().is_none(), "no manifest means no SDK Build");
    let catalog = catalog(&archive, None)?;
    assert_eq!(
        catalog.pointer("/source/build"),
        Some(&Value::Null),
        "the catalog preserves the missing Build"
    );
    Ok(())
}

fn check_mismatched_patch(archive: &Archive, folder: &Path) -> TestResult {
    let patches = read_patches(folder, "mismatched", "(SC PTU 9.9.9-PTU.10012346)\n")?;
    assert_eq!(
        patches.get("10012346"),
        Some("9.9.9"),
        "the neighboring change is a valid log record"
    );
    let build = archive
        .build()
        .ok_or("the manifest for the mismatched log")?;
    assert!(
        build.public_patch().is_none(),
        "opening the Archive leaves launcher enrichment separate"
    );
    assert!(
        patches.of(build).is_none(),
        "the different change never labels this Build"
    );
    let catalog = catalog(archive, Some(&patches))?;
    assert_eq!(
        catalog.pointer(PUBLIC_PATCH),
        Some(&Value::Null),
        "the catalog also leaves the Build's public patch unknown",
    );
    Ok(())
}

fn read_patches(folder: &Path, label: &str, log: &str) -> TestResult<PublicPatches> {
    let folder = folder.join(label);
    std::fs::create_dir(&folder)?;
    std::fs::write(folder.join("log.log"), log)?;
    Ok(PublicPatches::read_from(&folder))
}

fn catalog(archive: &Archive, patches: Option<&PublicPatches>) -> TestResult<Value> {
    let options = CatalogOptions {
        public_patches: patches,
        ..CatalogOptions::new(UNIX_EPOCH)
    };
    let result = build_catalog(archive, &options)?;
    Ok(serde_json::from_slice(result.json())?)
}

fn verify<T: JsonSchema + Serialize>(
    catalog: &Value,
    path: &'static str,
    value: &T,
) -> TestResult<Route> {
    assert_eq!(
        catalog.pointer(path),
        Some(&serde_json::to_value(value)?),
        "the Archive selector and normal producer agree at {path}",
    );
    Route::queried_rows(path, std::slice::from_ref(value), None)
}

#[test]
fn catalog_source_members_match_the_archive_and_exact_launcher_change() -> TestResult {
    read().map(|_| ())
}
