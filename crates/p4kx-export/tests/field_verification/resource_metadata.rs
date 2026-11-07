// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::{Archive, ArchiveWatch, DataForge, DataForgeEntry};
use serde::Serialize;

use super::{resource_declarations, resource_metadata_output};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SourceBuild<'a> {
    branch: &'a str,
    version: &'a str,
    change_number: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Metadata<'a> {
    schema_version: u32,
    data_forge_format_version: u32,
    source_build: Option<SourceBuild<'a>>,
    declarations: resource_declarations::Declarations,
}

fn document<'a>(
    archive: &'a Archive,
    forge: &DataForge<'_>,
    cancelled: &impl Fn() -> bool,
) -> super::Result<Metadata<'a>> {
    Ok(Metadata {
        schema_version: 1,
        data_forge_format_version: forge.version(),
        source_build: archive.build().map(|build| SourceBuild {
            branch: build.branch(),
            version: build.version(),
            change_number: build.change_number(),
        }),
        declarations: resource_declarations::collect(forge, cancelled)?,
    })
}

fn publish(
    archive: &Archive,
    forge: &DataForge<'_>,
    watch: &ArchiveWatch,
    destination: &resource_metadata_output::Destination,
) -> super::Result<()> {
    watch.check()?;
    let metadata = document(archive, forge, &|| watch.poll().is_err())?;
    let bytes = serde_json::to_vec_pretty(&metadata)?;
    watch.check()?;
    destination.write(&bytes)
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn resource_owned_type_declarations_only() -> super::Result<()> {
    let root =
        std::env::var_os("P4KX_METADATA_TASK_ROOT").ok_or("explicit metadata task root absent")?;
    let output =
        std::env::var_os("P4KX_RESOURCE_SCHEMA_OUT").ok_or("explicit metadata output absent")?;
    let destination =
        resource_metadata_output::Destination::new(Path::new(&root), Path::new(&output))?;
    let archive = Archive::open_with_key(
        p4kx_testkit::real_archive_path()?,
        Some(p4kx_testkit::real_archive_key()?),
    )?;
    let watch = ArchiveWatch::new(&archive);
    watch.check()?;
    let loaded = DataForgeEntry::read(&archive)?;
    watch.check()?;
    let forge = loaded.open()?;
    publish(&archive, &forge, &watch, &destination)?;
    super::report(
        "resource schema declaration artifact written; no Record values or identities surveyed",
    );
    Ok(())
}

#[test]
fn a_changed_archive_never_publishes_a_metadata_artifact() -> super::Result<()> {
    use std::io::Write as _;

    use p4kx_testkit::{ArchiveBuilder, TestEntry};

    let directory = tempfile::tempdir()?;
    let root = std::fs::canonicalize(directory.path())?;
    let path = root.join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(
            "Data/Game2.dcb",
            super::resource_metadata_fixture::database(),
        ))
        .write_to(&path)?;
    let archive = Archive::open(&path)?;
    let watch = ArchiveWatch::new(&archive);
    let loaded = DataForgeEntry::read(&archive)?;
    let forge = loaded.open()?;
    let output = root.join("resource-schema.json");
    let destination = resource_metadata_output::Destination::new(&root, &output)?;
    let mut changed = std::fs::OpenOptions::new().append(true).open(&path)?;
    changed.write_all(b"changed")?;
    changed.sync_all()?;
    drop(changed);
    assert!(
        publish(&archive, &forge, &watch, &destination).is_err(),
        "source changes fail before publishing declaration evidence"
    );
    assert!(
        !output.exists(),
        "a changed source creates no successful metadata artifact"
    );
    Ok(())
}
