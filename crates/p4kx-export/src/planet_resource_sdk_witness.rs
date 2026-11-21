// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    Archive, DecimalU64, Entry, InputScope, SourceLocation, SourceValueState, SourcedValue,
};
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde_json::Value;

use super::super::in_scope;
use super::TestResult;
use crate::catalog_queries::DatasetAvailability;
use crate::catalog_queries::planet_resources::{
    ExportEntryCounts, PlanetResource, PlanetResourceSelection, PlanetResourceSnapshot,
    read_planet_resources,
};
use crate::catalog_read::original::{EntryRead, EntryReceipt, with_entry_capture};
use crate::{CatalogError, ExportAccountingError, starmap_index};

struct InstalledResource {
    archive: Archive,
    path: String,
    change_number: String,
}

impl InstalledResource {
    fn open() -> TestResult<Self> {
        let archive = Archive::open_with_key(real_archive_path()?, real_archive_key().ok())?;
        let build = archive.build().ok_or("installed Build identity")?;
        let change_number = std::env::var("P4KX_PLANET_RESOURCE_BUILD")?;
        assert_eq!(
            build.change_number(),
            change_number,
            "selected Build identity"
        );
        let path = starmap_index::key(&std::env::var("P4KX_PLANET_RESOURCE_PATH")?);
        assert!(in_scope(&path), "a qualifying typed resource selector");
        Ok(Self {
            archive,
            path,
            change_number,
        })
    }

    fn matching_directory_entries(&self) -> impl Iterator<Item = &Entry> {
        self.archive
            .entries()
            .iter()
            .filter(|entry| starmap_index::key(entry.path()) == self.path)
    }

    #[expect(
        clippy::print_stdout,
        reason = "installed witness shows only build and count"
    )]
    fn report(&self, original_resources: usize) {
        let change = &self.change_number;
        println!("change={change} originalResources={original_resources}");
    }
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn installed_planet_resource_sdk_matches_one_physical_definition() -> TestResult {
    let installed = InstalledResource::open()?;
    let archive = &installed.archive;
    let entries: Vec<_> = installed.matching_directory_entries().collect();
    let [entry] = entries.as_slice() else {
        return Err("one exact original resource Entry must qualify".into());
    };
    let original = original_json(archive, entry)?;
    let snapshot = read_planet_resources(
        archive,
        PlanetResourceSelection::ArchivePath(&installed.path),
        None,
    )?;
    let [row] = snapshot.rows.as_slice() else {
        return Err("the selected resource Entry must project exactly once".into());
    };
    check_original_fields(&snapshot, row, &original, entry)?;
    assert!(
        snapshot.is_selection_complete(),
        "the verified original Entry has a complete selected projection"
    );
    installed.report(1);
    Ok(())
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn installed_planet_resource_sdk_proves_exact_path_absent() -> TestResult {
    let installed = InstalledResource::open()?;
    let archive = &installed.archive;
    assert!(
        installed.matching_directory_entries().next().is_none(),
        "no original Entry has the selected path"
    );
    let snapshot = read_planet_resources(
        archive,
        PlanetResourceSelection::ArchivePath(&installed.path),
        None,
    )?;
    assert_eq!(
        snapshot.build.as_ref(),
        archive.build(),
        "the absence proof belongs to the selected Build"
    );
    assert!(
        matches!(snapshot.availability, DatasetAvailability::Absent),
        "the SDK identifies exact-path absence"
    );
    assert!(
        snapshot.is_selection_complete(),
        "a healthy directory proves exact-path absence"
    );
    assert!(snapshot.rows.is_empty(), "no resource row is emitted");
    assert_eq!(
        snapshot.entries,
        ExportEntryCounts::default(),
        "an absent path selects, reads and projects no Entry"
    );
    check_absence_provenance(&snapshot, archive)?;
    installed.report(0);
    Ok(())
}

fn check_absence_provenance(snapshot: &PlanetResourceSnapshot, archive: &Archive) -> TestResult {
    let [reference] = snapshot.directory.source_uses_refs() else {
        return Err("the absence proof must cite one directory survey".into());
    };
    let used = snapshot
        .source_inventory
        .iter_uses()
        .find(|used| used.id() == reference)
        .ok_or("the exact-path directory proof is in the query inventory")?;
    let end_entry_index = u64::try_from(archive.entries().len())?;
    assert_eq!(
        used.location(),
        &SourceLocation::ArchiveDirectoryPaths {
            start_entry_index: DecimalU64::new(0),
            end_entry_index: DecimalU64::new(end_entry_index),
        },
        "the absence proof covers the complete admitted directory"
    );
    assert!(
        snapshot
            .source_inventory
            .iter_uses()
            .all(|used| used.scope() == InputScope::ArchiveDirectoryMetadata),
        "absence needs directory evidence alone"
    );
    Ok(())
}

fn check_original_fields(
    snapshot: &PlanetResourceSnapshot,
    row: &PlanetResource,
    original: &Value,
    entry: &Entry,
) -> TestResult {
    let physical_index = u32::try_from(entry.index())?;
    assert_eq!(
        row.identity().entry_index(),
        physical_index,
        "the SDK row retains the original directory ordinal"
    );
    assert_eq!(
        row.identity().source_path().value().map(String::as_str),
        Some(entry.path()),
        "the SDK row retains the original Archive path spelling"
    );
    compare(
        original,
        "guid",
        row.guid_raw(),
        Value::as_str,
        String::as_str,
    );
    compare(original, "name", row.name(), Value::as_str, String::as_str);
    compare(
        original,
        "bMinable",
        row.minable(),
        Value::as_bool,
        |value| *value,
    );
    compare(
        original,
        "tintPalettePath",
        row.tint_palette_path(),
        Value::as_str,
        String::as_str,
    );
    for origin in [
        row.guid_raw().origin(),
        row.name().origin(),
        row.minable().origin(),
        row.tint_palette_path().origin(),
    ] {
        assert!(
            origin.source_set_complete(),
            "the source Entry accounts for the entire field contribution"
        );
        for reference in origin.source_uses_refs() {
            let used = snapshot
                .source_inventory
                .iter_uses()
                .find(|used| used.id() == reference)
                .ok_or("the original field contributor is in the query inventory")?;
            assert_eq!(
                used.source(),
                row.identity().input_ref(),
                "the field addresses the exact source Entry"
            );
        }
    }
    Ok(())
}

fn original_json(archive: &Archive, entry: &Entry) -> TestResult<Value> {
    let bytes = with_entry_capture(archive, |capture| {
        let receipt = EntryReceipt::outer(capture, archive, entry)?;
        let Ok(read) = EntryRead::read_verified(receipt, archive, entry, None)? else {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        };
        Ok::<_, CatalogError>(read.bytes().to_vec())
    })?;
    Ok(serde_json::from_slice(&bytes)?)
}

fn compare<'a, T, R: std::fmt::Debug + PartialEq>(
    source: &'a Value,
    key: &str,
    actual: &'a SourcedValue<T>,
    authored: impl FnOnce(&'a Value) -> Option<R>,
    projected: impl FnOnce(&'a T) -> R,
) {
    let value = source.get(key);
    let expected = match value {
        None => SourceValueState::Absent,
        Some(Value::Null) => SourceValueState::Null,
        Some(_) => SourceValueState::Present,
    };
    assert_eq!(actual.state(), expected, "original field state");
    assert_eq!(
        actual.value().map(projected),
        value.and_then(authored),
        "original field value"
    );
}
