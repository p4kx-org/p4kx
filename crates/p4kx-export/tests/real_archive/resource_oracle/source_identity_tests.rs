// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, DataForgeOrigin, InputScope, SourceInput, SourceUses};
use p4kx_testkit::{ArchiveBuilder, TestEntry};
use serde_json::{Value, json};

use super::super::{Failure, Result};
use super::dcb_source;

struct Fixture {
    _folder: tempfile::TempDir,
    origin: DataForgeOrigin,
    catalog: Value,
}

impl Fixture {
    fn new(payload: &[u8]) -> Result<Self> {
        let folder =
            tempfile::tempdir().map_err(|error| Failure::source("synthetic-folder", error))?;
        let path = folder.path().join("source.p4k");
        ArchiveBuilder::new()
            .entry(TestEntry::stored("Data/Game2.dcb", payload))
            .write_to(&path)
            .map_err(|error| Failure::source("synthetic-archive", error))?;
        let archive =
            Archive::open(path).map_err(|error| Failure::source("synthetic-open", error))?;
        let entry = archive
            .entries()
            .first()
            .ok_or_else(|| Failure::check("synthetic-entry"))?;
        let origin = archive
            .entry_origin(entry)
            .ok_or_else(|| Failure::check("synthetic-origin"))?;
        let sources = SourceUses::new();
        let _scope = sources
            .observe(SourceInput::Archive(&origin), InputScope::Payload)
            .map_err(|error| Failure::source("synthetic-observation", error))?;
        let inventory = sources
            .freeze()
            .map_err(|error| Failure::source("synthetic-inventory", error))?;
        Ok(Self {
            _folder: folder,
            origin: DataForgeOrigin::Archive(origin),
            catalog: json!({"provenance": inventory}),
        })
    }

    fn source_mut(&mut self) -> Result<&mut Value> {
        self.catalog
            .pointer_mut("/provenance/sources")
            .and_then(Value::as_array_mut)
            .and_then(|sources| sources.first_mut())
            .ok_or_else(|| Failure::check("synthetic-source"))
    }

    fn mutate(&mut self, path: &str, value: Value) -> Result<()> {
        *self
            .source_mut()?
            .pointer_mut(path)
            .ok_or_else(|| Failure::check("synthetic-source-field"))? = value;
        Ok(())
    }
}

#[test]
fn changed_stored_encryption_cannot_keep_the_same_dcb_binding() -> Result<()> {
    reject_changed_field("/storedEncryption", json!("encrypted"))
}

#[test]
fn changed_directory_coordinates_cannot_keep_the_same_dcb_binding() -> Result<()> {
    reject_changed_field("/recordRange/offset", json!("999999"))
}

#[test]
fn changed_record_format_cannot_keep_the_same_dcb_binding() -> Result<()> {
    reject_changed_field("/recordFormat", json!("p4kV2"))
}

#[test]
fn changed_zip_flags_cannot_keep_the_same_dcb_binding() -> Result<()> {
    reject_changed_field("/zipFlags/value", json!(3))
}

#[test]
fn changed_raw_encryption_declarations_cannot_keep_the_same_dcb_binding() -> Result<()> {
    reject_changed_field("/declarations/value/0/value/rawWord/value", json!(7))
}

fn reject_changed_field(path: &str, value: Value) -> Result<()> {
    let mut fixture = Fixture::new(b"synthetic-source")?;
    dcb_source(&fixture.catalog, &fixture.origin)?;
    fixture.mutate(path, value)?;
    assert!(
        dcb_source(&fixture.catalog, &fixture.origin).is_err(),
        "matching Entry index and path cannot hide altered original source evidence"
    );
    Ok(())
}

#[test]
fn a_metadata_use_cannot_keep_its_id_after_its_location_changes() -> Result<()> {
    let mut fixture = Fixture::new(b"synthetic-source")?;
    dcb_source(&fixture.catalog, &fixture.origin)?;
    let observation = fixture
        .catalog
        .pointer_mut("/provenance/uses")
        .and_then(Value::as_array_mut)
        .and_then(|uses| uses.first_mut())
        .ok_or_else(|| Failure::check("synthetic-metadata-use"))?;
    observation["location"]["field"] = json!("different-metadata-field");
    assert!(
        dcb_source(&fixture.catalog, &fixture.origin).is_err(),
        "metadata-use identity must retain the original location and role"
    );
    Ok(())
}

#[test]
fn another_archive_with_the_same_entry_name_and_index_cannot_supply_different_evidence()
-> Result<()> {
    let original = Fixture::new(b"short")?;
    let other =
        Fixture::new(b"a different synthetic payload changes the central-directory position")?;
    dcb_source(&original.catalog, &original.origin)?;
    dcb_source(&other.catalog, &other.origin)?;
    assert!(
        dcb_source(&other.catalog, &original.origin).is_err(),
        "same nominal coordinates do not replace the retained physical observation"
    );
    Ok(())
}

#[test]
fn additive_unknown_source_metadata_does_not_change_the_observed_binding() -> Result<()> {
    let mut fixture = Fixture::new(b"synthetic-source")?;
    fixture.source_mut()?["futureMetadata"] = json!({"unrelated": true});
    dcb_source(&fixture.catalog, &fixture.origin)?;
    Ok(())
}
