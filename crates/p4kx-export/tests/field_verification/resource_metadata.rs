// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::Read as _;
use std::path::Path;

use p4kx_extraction::{Archive, ArchiveWatch, DataForge, DataForgeEntry};
use serde::Serialize;

use super::inventory::BuildIdentity;
use super::resource_declarations::Scope;
use super::{resource_declarations, resource_metadata_output};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BuildVersion<'a> {
    branch: &'a str,
    version: &'a str,
    change_number: &'a str,
}

#[derive(Serialize)]
#[serde(untagged)]
enum SourceBuild<'a> {
    Version(BuildVersion<'a>),
    Identity(BuildIdentity),
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Metadata<'a> {
    schema_version: u32,
    data_forge_format_version: u32,
    source_build: Option<SourceBuild<'a>>,
    declarations: resource_declarations::Declarations,
}

fn source_build(archive: &Archive, scope: Scope) -> super::Result<Option<SourceBuild<'_>>> {
    if matches!(scope, Scope::QuantumDrives) {
        let build = BuildIdentity::of_archive(archive)
            .ok_or("quantum declaration evidence needs a complete Build and known Channel")?;
        return Ok(Some(SourceBuild::Identity(build)));
    }
    Ok(archive.build().map(|build| {
        SourceBuild::Version(BuildVersion {
            branch: build.branch(),
            version: build.version(),
            change_number: build.change_number(),
        })
    }))
}

pub(crate) fn publish(
    archive: &Archive,
    forge: &DataForge<'_>,
    watch: &ArchiveWatch,
    destination: &resource_metadata_output::Destination,
    scope: Scope,
) -> super::Result<()> {
    publish_document(
        forge,
        source_build(archive, scope)?,
        watch,
        destination,
        scope,
    )
}

fn publish_document(
    forge: &DataForge<'_>,
    source_build: Option<SourceBuild<'_>>,
    watch: &ArchiveWatch,
    destination: &resource_metadata_output::Destination,
    scope: Scope,
) -> super::Result<()> {
    watch.check()?;
    let metadata = Metadata {
        schema_version: 1,
        data_forge_format_version: forge.version(),
        source_build,
        declarations: resource_declarations::collect_scope(forge, scope, &|| {
            watch.poll().is_err()
        })?,
    };
    let bytes = serde_json::to_vec_pretty(&metadata)?;
    watch.check()?;
    destination.write(&bytes)
}

pub(crate) fn read_and_publish(
    archive: &Archive,
    destination: &resource_metadata_output::Destination,
    scope: Scope,
) -> super::Result<()> {
    let watch = ArchiveWatch::new(archive);
    watch.check()?;
    let build = source_build(archive, scope)?;
    let original = DataForgeEntry::locate(archive).ok_or("original DataForge Entry absent")?;
    let size = original.uncompressed_size();
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(usize::try_from(size)?)?;
    archive
        .open_entry_verified(original)?
        .reject_overlong()
        .read_to_end(&mut bytes)?;
    let forge = DataForge::open(&bytes)?;
    publish_document(&forge, build, &watch, destination, scope)
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn resource_owned_type_declarations_only() -> super::Result<()> {
    run(Scope::Resources)
}

pub(crate) fn run(scope: Scope) -> super::Result<()> {
    let root =
        std::env::var_os("P4KX_METADATA_TASK_ROOT").ok_or("explicit metadata task root absent")?;
    let output_variable = match scope {
        Scope::Resources => "P4KX_RESOURCE_SCHEMA_OUT",
        Scope::MissionProperties => "P4KX_MISSION_PROPERTY_SCHEMA_OUT",
        Scope::QuantumDrives => "P4KX_QUANTUM_SCHEMA_OUT",
    };
    let output = std::env::var_os(output_variable).ok_or("explicit metadata output absent")?;
    let destination =
        resource_metadata_output::Destination::new(Path::new(&root), Path::new(&output))?;
    let archive = Archive::open_with_key(
        p4kx_testkit::real_archive_path()?,
        Some(p4kx_testkit::real_archive_key()?),
    )?;
    read_and_publish(&archive, &destination, scope)?;
    super::report(match scope {
        Scope::Resources => {
            "resource schema declaration artifact written; no Record values or identities surveyed"
        }
        Scope::MissionProperties => {
            "mission-property schema declaration artifact written; no Record values or identities surveyed"
        }
        Scope::QuantumDrives => {
            "quantum-drive schema declaration artifact written; no Record values or identities surveyed"
        }
    });
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
        publish(&archive, &forge, &watch, &destination, Scope::Resources).is_err(),
        "source changes fail before publishing declaration evidence"
    );
    assert!(
        !output.exists(),
        "a changed source creates no successful metadata artifact"
    );
    Ok(())
}
