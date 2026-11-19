// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, CancelToken, DataForgeEntry, Guid, SourceInventory};
use p4kx_testkit::ArchiveBuilder;
use serde_json::{Value, json};

use super::{PALETTE, SAMPLE, TestResult, VALID, fixture, json_catalog, resource, rows};
use crate::catalog_queries::DatasetAvailability;
use crate::catalog_queries::planet_resources::{
    LooseArchivePathState, PlanetResourceSelection, read_planet_resources,
    read_planet_resources_from,
};
use crate::{CatalogError, ExportDiagnosticCode};

#[test]
fn scoped_planet_rows_match_catalog_and_close_mixed_provenance() -> TestResult {
    let (_folder, archive) = fixture(
        vec![
            resource("alias", format!(r#"{{"guid":"{VALID}","tintPalettePath":""}}"#)),
            resource(
                "encrypted",
                format!(r#"{{"guid":"{{{VALID}}}","name":"fixture","bMinable":false,"tintPalettePath":"{PALETTE}"}}"#),
            )
            .encrypted(),
            resource("unresolved", r#"{"tintPalettePath":"Fixtures/Missing.data","bMinable":null}"#),
        ],
        true,
    )?;
    let catalog = json_catalog(&archive, None)?;
    let selected = read_planet_resources(&archive, PlanetResourceSelection::All, None)?;
    let decoded = DataForgeEntry::read(&archive)?;
    let reused =
        read_planet_resources_from(&archive, &decoded, PlanetResourceSelection::All, None)?;
    let actual = serde_json::to_value(&selected.rows)?;
    check_closure(&actual, &selected.source_inventory)?;
    let [_, _, unresolved] = selected.rows.as_slice() else {
        return Err("the plain, encrypted and unresolved resource Entries must all project".into());
    };
    check_encryption(&actual);
    assert_eq!(
        unresolved.tint_palette().state(),
        Some(LooseArchivePathState::Missing),
        "a missing palette is represented on its readable resource"
    );
    assert!(
        selected.is_selection_complete(),
        "an unresolved target leaves the resource selection complete"
    );
    let scoped = without_ids(actual);
    assert_eq!(
        without_ids(Value::Array(rows(&catalog)?.to_vec())),
        scoped,
        "the selective reader emits the same fields and states"
    );
    assert_eq!(
        without_ids(serde_json::to_value(&reused.rows)?),
        scoped,
        "reusing parsed DataForge preserves the result"
    );
    Ok(())
}

#[test]
fn exact_and_guid_selectors_do_not_resolve_unrelated_palette_paths() -> TestResult {
    let (_folder, archive) = selection_fixture()?;
    check_physical_selectors(&archive)?;
    check_guid_selector(&archive)?;
    check_absent_and_cancelled(&archive)
}

fn selection_fixture() -> TestResult<(tempfile::TempDir, Archive)> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(resource(
            "same",
            format!(r#"{{"guid":"{VALID}","bMinable":true}}"#),
        ))
        .entry(resource(
            "same",
            format!(r#"{{"guid":"{VALID}","bMinable":false}}"#),
        ))
        .entry(resource("bad", "{"))
        .entry(resource(
            "unselected",
            format!(r#"{{"tintPalettePath":"{PALETTE}"}}"#),
        ))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    Ok((folder, archive))
}

fn check_physical_selectors(archive: &Archive) -> TestResult {
    let direct = read_planet_resources(
        archive,
        PlanetResourceSelection::ArchivePath(r"DATA\Libs/Planets/Resources/SAME.Resource"),
        None,
    )?;
    let [first, second] = direct.rows.as_slice() else {
        return Err("exactly two physical path aliases must be retained".into());
    };
    assert!(
        direct.is_selection_complete(),
        "unrequested bodies are untouched"
    );
    assert_eq!(
        direct.entries.selected, 2,
        "both physical aliases are selected independently"
    );
    assert_eq!(
        first.minable().value(),
        Some(&true),
        "the first alias retains its authored mining flag"
    );
    assert_eq!(
        second.minable().value(),
        Some(&false),
        "the second alias retains its different mining flag"
    );
    let index = second.identity().entry_index();
    let one = read_planet_resources(archive, PlanetResourceSelection::EntryIndex(index), None)?;
    let [one_row] = one.rows.as_slice() else {
        return Err("the physical Entry query must yield exactly one row".into());
    };
    assert_eq!(one.entries.selected, 1, "one physical Entry was read");
    assert_eq!(
        one_row.identity().entry_index(),
        index,
        "the selected ordinal identifies the second alias"
    );
    assert!(one.is_selection_complete(), "one exact Entry is readable");

    Ok(())
}

fn check_guid_selector(archive: &Archive) -> TestResult {
    let guid: Guid = VALID.parse()?;
    let selected = read_planet_resources(archive, PlanetResourceSelection::Guid(guid), None)?;
    assert_eq!(
        selected.rows.len(),
        2,
        "GUID selection preserves both original aliases"
    );
    assert_eq!(
        selected.entries.failed_selection_candidates, 1,
        "the malformed JSON could not be tested for the requested GUID"
    );
    assert!(
        !selected.is_selection_complete(),
        "a malformed candidate prevents proving there is no further alias"
    );
    assert!(
        selected.rows.iter().all(|row| row
            .issues()
            .iter()
            .any(|issue| { matches!(issue.code(), ExportDiagnosticCode::AmbiguousIdentity) })),
        "duplicates remain separate with the canonical identity diagnosis"
    );
    assert!(
        matches!(
            read_planet_resources(archive, PlanetResourceSelection::All, None),
            Err(CatalogError::Load(_))
        ),
        "a selected Record reference requires DataForge"
    );
    Ok(())
}

fn check_absent_and_cancelled(archive: &Archive) -> TestResult {
    let missing = format!("{SAMPLE}absent.resource");
    let none = read_planet_resources(
        archive,
        PlanetResourceSelection::ArchivePath(&missing),
        None,
    )?;
    assert!(
        matches!(none.availability, DatasetAvailability::Absent),
        "the directory has no Entry matching the exact path"
    );
    assert!(
        none.is_selection_complete(),
        "a healthy directory proves exact-path absence"
    );
    let cancel = CancelToken::new();
    cancel.cancel();
    assert!(
        matches!(
            read_planet_resources(archive, PlanetResourceSelection::All, Some(&cancel)),
            Err(CatalogError::Cancelled)
        ),
        "cancelled queries stop before opening Entry payloads"
    );
    Ok(())
}

fn check_encryption(actual: &Value) {
    assert_eq!(
        actual.pointer("/0/guid/containsEncryptedSource"),
        Some(&json!(false)),
        "the plain Entry GUID has a complete unencrypted source"
    );
    assert_eq!(
        actual.pointer("/1/guid/containsEncryptedSource"),
        Some(&json!(true)),
        "the encrypted Entry GUID retains its payload encryption"
    );
    assert_eq!(
        actual.pointer("/1/tintPalette/state/containsEncryptedSource"),
        Some(&json!(true)),
        "the mixed lookup includes the encrypted authored path"
    );
    assert_eq!(
        actual.pointer("/1/tintPalette/candidates/0/guid/containsEncryptedSource"),
        Some(&json!(false)),
        "the target GUID comes from plain DataForge"
    );
    assert_eq!(
        actual.pointer("/1/tintPalette/state/sourceSetComplete"),
        Some(&json!(true)),
        "both the authored selector and target are accounted for"
    );
}

fn without_ids(mut value: Value) -> Value {
    match &mut value {
        Value::Object(fields) => {
            fields.remove("inputRef");
            fields.remove("sourceUsesRefs");
            for member in fields.values_mut() {
                *member = without_ids(std::mem::take(member));
            }
        }
        Value::Array(items) => {
            for item in items {
                *item = without_ids(std::mem::take(item));
            }
        }
        _ => {}
    }
    value
}

fn check_closure(value: &Value, inventory: &SourceInventory) -> TestResult {
    match value {
        Value::Object(fields) => {
            if let Some(Value::Array(refs)) = fields.get("sourceUsesRefs") {
                for id in refs {
                    let id = id.as_str().ok_or("a string source-use reference")?;
                    let observation = inventory
                        .iter_uses()
                        .find(|used| used.id().as_str() == id)
                        .ok_or("row use in the returned source inventory")?;
                    assert!(
                        inventory
                            .iter_sources()
                            .any(|source| source.id() == observation.source()),
                        "each contributing use points to a physical source in the query inventory"
                    );
                }
            }
            for member in fields.values() {
                check_closure(member, inventory)?;
            }
        }
        Value::Array(items) => {
            for item in items {
                check_closure(item, inventory)?;
            }
        }
        _ => {}
    }
    Ok(())
}
