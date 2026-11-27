// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::io::Read as _;

use p4kx_export::catalog_queries::DatasetAvailability;
use p4kx_export::catalog_queries::shops::{
    ExportEntryCounts, ShopInventorySelection, ShopInventorySnapshot, read_shop_inventories,
};
use p4kx_extraction::{
    Archive, ArchiveWatch, ObservedEntry, PhysicalSourceKind, PhysicalSourceRef, SourceLocation,
    SourceUseRole,
};
use p4kx_testkit::{real_archive_b_path, real_archive_key, real_archive_path};
use serde_json::value::RawValue;
use serde_json::{Value, json};

use super::{
    Arena, NUMBER_FIELDS, Survey, TestResult, inventory, rows, token_index, verify_decoded,
};
use crate::real_archive_verification;

type Object<'source> = BTreeMap<&'source str, &'source RawValue>;

struct Number<'source> {
    layout: &'static str,
    line: usize,
    field: &'static str,
    authored: &'static str,
    token: &'source RawValue,
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and P4KX_DATA_P4K_B and pass --ignored"]
fn selective_shop_inventories_match_an_original_inventory_and_missing_index() -> TestResult {
    let first = real_archive_path()?;
    let second = real_archive_b_path().ok_or("second installed Archive in P4KX_DATA_P4K_B")?;
    let key = real_archive_key().map_err(|_| "configured Archive key")?;
    for (sample, path) in [(1, first), (2, second)] {
        let archive =
            Archive::open_with_key(path, Some(key.clone())).map_err(|_| "open input Archive")?;
        let watch = ArchiveWatch::new(&archive);
        watch.check()?;
        let (entry, bytes) = original(&archive)?;
        let count = check_inventory(&archive, entry, &bytes)?;
        check_missing(&archive)?;
        watch.check()?;
        real_archive_verification::print_line(&format!(
            "Shop inventory SDK source witness {sample}: one physical inventory, {count} exact authored numeric cells, one missing index"
        ));
    }
    Ok(())
}

fn original(archive: &Archive) -> TestResult<(ObservedEntry<'_>, Vec<u8>)> {
    let mut candidates: Vec<_> = archive
        .entries()
        .iter()
        .enumerate()
        .filter(|(_, entry)| entry.uncompressed_size() > 0 && inventory(entry.path()))
        .collect();
    candidates.sort_unstable_by_key(|(index, entry)| (entry.uncompressed_size(), *index));
    for (_, entry) in candidates {
        let Ok(reader) = archive.open_entry_verified(entry) else {
            continue;
        };
        let mut bytes = Vec::new();
        if reader.reject_overlong().read_to_end(&mut bytes).is_ok() && !numbers(&bytes).is_empty() {
            let original = archive
                .observed_entry(entry)
                .ok_or("physical inventory Entry")?;
            return Ok((original, bytes));
        }
    }
    Err("original Shop inventory with verified bytes and authored numeric cells".into())
}

fn numbers(bytes: &[u8]) -> Vec<Number<'_>> {
    let Ok(root) = serde_json::from_slice::<Object<'_>>(bytes) else {
        return Vec::new();
    };
    let collection = root
        .get("Collection")
        .and_then(|value| serde_json::from_str::<Object<'_>>(value.get()).ok());
    let mut result = Vec::new();
    for (layout, inventory) in [
        ("/fields/inventory/value", root.get("Inventory")),
        (
            "/fields/collection/value/inventory/value",
            collection
                .as_ref()
                .and_then(|fields| fields.get("Inventory")),
        ),
    ] {
        let Some(lines) =
            inventory.and_then(|value| serde_json::from_str::<Vec<&RawValue>>(value.get()).ok())
        else {
            continue;
        };
        for (line, value) in lines.into_iter().enumerate() {
            if let Ok(fields) = serde_json::from_str::<Object<'_>>(value.get()) {
                for (field, authored) in NUMBER_FIELDS {
                    if let Some(token) = fields.get(authored)
                        && matches!(token.get().as_bytes().first(), Some(b'-' | b'0'..=b'9'))
                    {
                        result.push(Number {
                            layout,
                            line,
                            field,
                            authored,
                            token,
                        });
                    }
                }
            }
        }
    }
    result
}

fn check_inventory(
    archive: &Archive,
    original: ObservedEntry<'_>,
    bytes: &[u8],
) -> TestResult<usize> {
    let index = original.entry_index();
    let snapshot = read_shop_inventories(archive, ShopInventorySelection::EntryIndex(index), None)
        .map_err(|_| "direct physical inventory query")?;
    check_build(archive, &snapshot)?;
    assert!(
        matches!(snapshot.availability, DatasetAvailability::Available),
        "the selected inventory is available"
    );
    assert_eq!(
        snapshot.coverage.entries,
        ExportEntryCounts {
            selected: 1,
            read_successful: 1,
            projected: 1,
            ..ExportEntryCounts::default()
        },
        "only the selected original inventory was read and projected"
    );
    let [row] = snapshot.rows.as_slice() else {
        return Err("one inventory for the requested physical index".into());
    };
    let identity = row.identity();
    assert_eq!(
        identity.entry_index(),
        index,
        "the row retains the requested physical Entry index"
    );
    let mut sources = snapshot.source_inventory.iter_sources().filter(|source| {
        matches!(
            source.kind(),
            PhysicalSourceKind::ArchiveEntry(_) | PhysicalSourceKind::ArchiveEntryProof(_)
        )
    });
    let source = sources.next().ok_or("observed physical inventory input")?;
    assert!(
        source.id() == identity.input_ref() && sources.next().is_none(),
        "the selected inventory is the only observed Entry"
    );
    let PhysicalSourceKind::ArchiveEntry(source) = source.kind() else {
        return Err("concrete stored identity of the inventory Entry".into());
    };
    assert!(
        source.entry_index() == usize::try_from(index)?
            && source.path().value().map(String::as_str) == Some(original.entry().path()),
        "the physical source refers to the original directory occurrence"
    );
    check_directory(&snapshot, identity.input_ref(), &["entryIndex", "path"])?;
    let row = serde_json::to_value(row)?;
    let wire = json!({ "provenance": snapshot.source_inventory });
    let uses = token_index(&wire)?;
    let mut survey = Survey::default();
    verify_decoded(original, &row, &uses, &mut survey, Some(bytes))?;
    check_numbers(&row, &uses, bytes, survey.number_tokens)
}

fn check_build(archive: &Archive, snapshot: &ShopInventorySnapshot) -> TestResult {
    let build = archive.build().ok_or("complete actual Build identity")?;
    assert!(
        [
            build.branch(),
            build.version(),
            build.change_number(),
            build.build_id()
        ]
        .into_iter()
        .all(|value| !value.trim().is_empty())
            && snapshot.build.as_ref() == Some(build),
        "the query retains the complete original Build identity"
    );
    Ok(())
}

fn check_numbers(
    row: &Value,
    uses: &super::TokenUses<'_>,
    bytes: &[u8],
    projected: usize,
) -> TestResult<usize> {
    let body = row.pointer("/document/value").ok_or("present inventory")?;
    let arena = Arena {
        bytes,
        id: super::text(row, "inputRef")?,
        values: rows(body, "/source/values")?,
        uses,
    };
    let expected = numbers(bytes);
    assert_eq!(
        projected,
        expected.len(),
        "every authored numeric slot contributes"
    );
    for cell in &expected {
        let field = rows(body, cell.layout)?
            .get(cell.line)
            .and_then(|line| line.get("value"))
            .and_then(|line| line.get(cell.field))
            .ok_or("original inventory line and numeric field")?;
        assert_eq!(
            field["state"], "present",
            "the observed original numeric slot is present"
        );
        let (raw, _) = arena.raw(field, cell.authored)?;
        assert!(
            std::ptr::eq(raw, cell.token.get().as_bytes()),
            "the numeric value cites the exact decoded token span at its original line position"
        );
    }
    Ok(expected.len())
}

fn check_missing(archive: &Archive) -> TestResult {
    let len = u32::try_from(archive.entries().len())?;
    let empty = read_shop_inventories(archive, ShopInventorySelection::EntryIndex(len), None)
        .map_err(|_| "direct inventory query outside the admitted directory")?;
    assert!(
        matches!(empty.availability, DatasetAvailability::Absent),
        "the admitted directory proves this index is absent"
    );
    assert!(
        empty.rows.is_empty(),
        "an out-of-range index cannot produce a row"
    );
    assert_eq!(
        empty.coverage.entries,
        ExportEntryCounts::default(),
        "there is no selected body or failed Entry read"
    );
    let mut sources = empty.source_inventory.iter_sources();
    let source = sources.next().ok_or("original admitted directory")?;
    let PhysicalSourceKind::ArchiveDirectory(directory) = source.kind() else {
        return Err("negative index cites the parsed original root directory".into());
    };
    assert_eq!(
        directory.admitted_entry_count().get(),
        u64::from(len),
        "the recorded count is the admitted directory size"
    );
    assert!(
        sources.next().is_none(),
        "the negative lookup reads no Entry"
    );
    check_directory(&empty, source.id(), &["entryCount"])
}

fn check_directory(
    snapshot: &ShopInventorySnapshot,
    physical: &PhysicalSourceRef,
    expected: &[&str],
) -> TestResult {
    let origin = &snapshot.coverage.directory;
    assert!(
        origin.source_set_complete(),
        "the directory observation must prove the lookup result"
    );
    let refs = origin.source_uses_refs();
    let mut fields = Vec::new();
    for observed in snapshot.source_inventory.iter_uses() {
        if refs.contains(observed.id()) {
            let SourceLocation::Metadata { field } = observed.location() else {
                return Err("a direct index addresses exact directory metadata".into());
            };
            assert!(
                observed.source() == physical,
                "the original physical directory source"
            );
            assert_eq!(
                observed.role(),
                SourceUseRole::Selector,
                "directory metadata supplies the selection decision"
            );
            fields.push(field.as_str());
        }
    }
    assert_eq!(refs.len(), fields.len(), "every citation resolves");
    fields.sort_unstable();
    assert_eq!(
        fields, expected,
        "only actual scope-specific metadata is cited"
    );
    Ok(())
}
