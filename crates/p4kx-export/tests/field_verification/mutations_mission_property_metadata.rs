// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, ArchiveWatch, DataForge, DataForgeEntry};
use p4kx_testkit::{ArchiveBuilder, DataForgeBuilder, TestEntry, TestStruct, TestType};
use serde_json::Value;
use std::path::Path;

use super::resource_declarations::Scope;
use super::{
    mission_property_metadata_fixture, resource_declarations, resource_metadata,
    resource_metadata_output,
};

fn verify_private_boundaries(bytes: &[u8]) -> super::Result<Value> {
    let text = std::str::from_utf8(bytes)?;
    for forbidden in [
        "SENTINEL",
        "9876.125",
        "doNotFollowReference",
        "doNotFollowWeak",
        "doNotFollowSibling",
    ] {
        assert!(
            !text.contains(forbidden),
            "only declaration metadata may reach the artifact"
        );
    }
    Ok(serde_json::from_slice(bytes)?)
}

#[test]
fn mission_scope_keeps_schema_only_roots_and_only_parent_owned_declarations() -> super::Result<()> {
    let bytes = mission_property_metadata_fixture::database();
    let forge = DataForge::open(&bytes)?;
    let declarations =
        resource_declarations::collect_scope(&forge, Scope::MissionProperties, &|| false)?;
    let value = verify_private_boundaries(&serde_json::to_vec(&declarations)?)?;
    let roots = value["roots"].as_array().ok_or("roots absent")?;
    assert!(
        roots
            .iter()
            .any(|name| name == "MissionPropertyValue_SchemaOnly"),
        "uninstantiated matching declarations remain eligible"
    );
    let owners = value["types"].as_array().ok_or("declarations absent")?;
    for name in [
        "MissionPropertyValue_Alpha",
        "MissionPropertyValue_SchemaOnly",
        "MetadataAncestor",
        "OwnedMissionBase",
        "OwnedMissionConcrete",
        "OwnedLeaf",
    ] {
        assert!(
            owners.iter().any(|owner| owner["name"] == name),
            "the requested parent/owned schema closure is complete"
        );
    }
    for forbidden in ["ExternalReference", "ExternalWeak", "UnrelatedSibling"] {
        assert!(
            !owners.iter().any(|owner| owner["name"] == forbidden),
            "unowned schema targets are named without following their fields"
        );
    }
    Ok(())
}

fn assert_rejected(builder: &DataForgeBuilder) -> super::Result<()> {
    let bytes = builder.build();
    let forge = DataForge::open(&bytes)?;
    assert!(
        resource_declarations::collect_scope(&forge, Scope::MissionProperties, &|| false).is_err(),
        "invalid or unbounded mission declaration scopes cannot claim completeness"
    );
    Ok(())
}

fn invalid_scopes() -> impl Iterator<Item = DataForgeBuilder> {
    const OVERSIZED_NAME_BYTES: usize = 4096;
    [
        DataForgeBuilder::new().structure(TestStruct::new("Unrelated")),
        DataForgeBuilder::new()
            .structure(TestStruct::new("MissionPropertyValue_A"))
            .structure(TestStruct::new("MissionPropertyValue_A")),
        DataForgeBuilder::new().structure(
            TestStruct::new("MissionPropertyValue_A")
                .field("owned", TestType::Class("MissingOwnedType".to_owned())),
        ),
        mission_property_metadata_fixture::excessive_closure(),
        DataForgeBuilder::new().structure(TestStruct::new(format!(
            "MissionPropertyValue_{}",
            "x".repeat(OVERSIZED_NAME_BYTES)
        ))),
    ]
    .into_iter()
}

#[test]
fn mission_scope_preserves_cancellation_identity_target_and_closure_limits() -> super::Result<()> {
    let bytes = mission_property_metadata_fixture::database();
    let forge = DataForge::open(&bytes)?;
    assert!(
        resource_declarations::collect_scope(&forge, Scope::MissionProperties, &|| true).is_err(),
        "metadata collection remains cancellable"
    );
    for invalid in invalid_scopes() {
        assert_rejected(&invalid)?;
    }
    Ok(())
}

#[test]
fn mission_metadata_publication_reuses_new_file_and_source_watch_guards() -> super::Result<()> {
    let directory = tempfile::tempdir()?;
    let root = std::fs::canonicalize(directory.path())?;
    let path = root.join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(
            "Data/Game2.dcb",
            mission_property_metadata_fixture::database(),
        ))
        .write_to(&path)?;
    let archive = Archive::open(&path)?;
    let watch = ArchiveWatch::new(&archive);
    let loaded = DataForgeEntry::read(&archive)?;
    let forge = loaded.open()?;
    let output = root.join("mission-schema.json");
    let destination = resource_metadata_output::Destination::new(&root, &output)?;
    resource_metadata::publish(
        &archive,
        &forge,
        &watch,
        &destination,
        Scope::MissionProperties,
    )?;
    let before = std::fs::read(&output)?;
    verify_private_boundaries(&before)?;
    assert!(
        resource_metadata::publish(
            &archive,
            &forge,
            &watch,
            &destination,
            Scope::MissionProperties
        )
        .is_err(),
        "an explicit existing artifact is never overwritten"
    );
    assert_eq!(
        std::fs::read(&output)?,
        before,
        "the existing declaration artifact remains intact"
    );
    append_source_change(&path)?;
    let fresh = root.join("changed-mission-schema.json");
    let destination = resource_metadata_output::Destination::new(&root, &fresh)?;
    assert!(
        resource_metadata::publish(
            &archive,
            &forge,
            &watch,
            &destination,
            Scope::MissionProperties
        )
        .is_err(),
        "source changes refuse metadata publication"
    );
    assert!(
        !fresh.exists(),
        "changed sources leave no successful artifact"
    );
    Ok(())
}

fn append_source_change(path: &Path) -> super::Result<()> {
    use std::io::Write as _;

    let mut changed = std::fs::OpenOptions::new().append(true).open(path)?;
    changed.write_all(b"changed")?;
    changed.sync_all()?;
    Ok(())
}
