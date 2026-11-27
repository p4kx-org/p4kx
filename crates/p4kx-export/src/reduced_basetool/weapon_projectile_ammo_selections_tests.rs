// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::path::Path;

use p4kx_extraction::{Archive, DataForge, Guid};
use p4kx_testkit::{ArchiveBuilder, TestEntry, test_key};
use serde_json::{Value, json};

use super::ReducedError;
use super::ammo_definitions;
use super::item_definitions;
use super::record_targets::RecordTargets;
use super::weapon_projectile_ammo_selections::{self as selections, Targets};
use crate::catalog_queries::equipment::ItemDefinitionSelection;
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, DatasetAvailability,
    ParsedCatalogQueries, ProvenanceMode,
};
use crate::catalog_rows::staged::StagedCatalogRows;

#[path = "weapon_projectile_ammo_fixture.rs"]
mod fixture;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

#[test]
fn stages_direct_linked_and_dangling_projectile_routes_with_markers() -> TestResult {
    with_queries(|parsed, forge, scratch| {
        let mut staged = StagedCatalogRows::new(scratch, None);
        let targets = Selected::read(parsed, None, None)?;
        let coordinates = append_routes(parsed, forge, &mut staged, &targets.borrow())?;
        let output = serde_json::to_string(&staged.finish(DatasetAvailability::Available)?)?;
        let table: Value = serde_json::from_str(&output)?;
        let [direct, linked, dangling] = rows(&table)? else {
            return Err("authored direct, linked and dangling routes".into());
        };
        for (row, (owner, action)) in [direct, linked, dangling].into_iter().zip(coordinates) {
            assert_eq!(row.pointer("/ownerRecordIndex/value"), Some(&json!(owner)));
            assert_eq!(row.pointer("/componentOrdinal/value"), Some(&json!(1)));
            assert_eq!(
                row.pointer("/actionSourceValueOrdinal/value"),
                Some(&json!(action))
            );
            assert_eq!(
                row.pointer("/selection/containsEncryptedSource"),
                Some(&json!(true))
            );
        }
        verify_direct(direct, record_index(forge, fixture::PRIMARY)?)?;
        verify_linked(linked, forge)?;
        assert_eq!(
            dangling.pointer("/selection/value"),
            Some(&json!({"type":"danglingMagazine"})),
            "a rejected original route keeps its discriminant and has no invented endpoints"
        );
        for excluded in ["sourceUsesRefs", "sourceSetComplete", "inputRef"] {
            assert!(!output.contains(excluded), "proof arena field {excluded}");
        }
        Ok(())
    })
}

fn append_routes(
    parsed: &ParsedCatalogQueries<'_>,
    forge: &DataForge<'_>,
    staged: &mut StagedCatalogRows<'_>,
    targets: &Targets<'_>,
) -> TestResult<Vec<(u32, String)>> {
    let mut keys = Vec::new();
    for guid in [fixture::DIRECT, fixture::LINKED, fixture::DANGLING] {
        let weapon = record_index(forge, guid)?;
        parsed.with_projectile_ammo(weapon, None, |snapshot| -> TestResult {
            let [owner] = snapshot.rows() else {
                return Err("one physical weapon candidate".into());
            };
            let [route] = owner.selections.as_slice() else {
                return Err("one original launcher action occurrence".into());
            };
            let ordinal = route
                .action_source_value_ordinal()
                .value()
                .ok_or("action ordinal")?;
            keys.push((weapon, ordinal.get().to_string()));
            selections::append(staged, &owner.selections, targets)?;
            Ok(())
        })??;
    }
    Ok(keys)
}

fn verify_direct(row: &Value, ammo: u32) -> TestResult {
    let route = row
        .pointer("/selection/value")
        .ok_or("original direct route")?;
    assert_eq!(route.get("type"), Some(&json!("selected")));
    assert_eq!(route.pointer("/role/value"), Some(&json!("primary")));
    assert_eq!(route.pointer("/ammoRecordIndex/value"), Some(&json!(ammo)));
    assert_eq!(
        route.pointer("/containerComponentOrdinal/value"),
        Some(&json!(2))
    );
    assert!(route.get("magazineRecordIndex").is_none());
    Ok(())
}

fn verify_linked(row: &Value, forge: &DataForge<'_>) -> TestResult {
    let route = row
        .pointer("/selection/value")
        .ok_or("original linked route")?;
    assert_eq!(route.get("type"), Some(&json!("selectedLinkedMagazine")));
    assert_eq!(route.pointer("/role/value"), Some(&json!("secondary")));
    assert_eq!(
        route.pointer("/magazineRecordIndex/value"),
        Some(&json!(record_index(forge, fixture::MAGAZINE)?))
    );
    assert_eq!(
        route.pointer("/ammoRecordIndex/value"),
        Some(&json!(record_index(forge, fixture::SECONDARY)?))
    );
    assert_eq!(
        route.pointer("/ammoRecordIndex/containsEncryptedSource"),
        Some(&json!(true))
    );
    assert_eq!(
        route.pointer("/containerComponentOrdinal/value"),
        Some(&json!(1))
    );
    Ok(())
}

#[test]
fn claimed_projectile_routes_require_both_independently_staged_targets() -> TestResult {
    with_queries(|parsed, forge, scratch| {
        let ammo = record_index(forge, fixture::SECONDARY)?;
        let magazine = record_index(forge, fixture::MAGAZINE)?;
        let weapon = record_index(forge, fixture::LINKED)?;
        for (omitted_ammo, omitted_item, expected_path, expected_index) in [
            (Some(ammo), None, ammo_definitions::PATH, ammo),
            (None, Some(magazine), item_definitions::PATH, magazine),
        ] {
            let targets = Selected::read(parsed, omitted_ammo, omitted_item)?;
            parsed.with_projectile_ammo(weapon, None, |snapshot| -> TestResult {
                let [owner] = snapshot.rows() else {
                    return Err("linked projectile owner".into());
                };
                let error = selections::append(
                    &mut StagedCatalogRows::new(scratch, None),
                    &owner.selections,
                    &targets.borrow(),
                );
                assert!(
                    matches!(error,
                    Err(ReducedError::UnresolvedRecordLink { path, index })
                    if path == expected_path && index == expected_index),
                    "the actual physical target must be in its selected table"
                );
                Ok(())
            })??;
        }
        Ok(())
    })
}

fn rows(table: &Value) -> TestResult<&[Value]> {
    Ok(table
        .get("rows")
        .and_then(Value::as_array)
        .ok_or("staged rows")?)
}

fn record_index(forge: &DataForge<'_>, value: u128) -> TestResult<u32> {
    let mut bytes = value.to_be_bytes();
    let (high, low) = bytes.split_at_mut(8);
    high.reverse();
    low.reverse();
    let guid = Guid::from_stored(bytes);
    Ok(forge
        .record_by_guid(guid)
        .ok_or("physical fixture Record")?
        .index())
}

#[derive(Default)]
struct Selected {
    ammo: RecordTargets,
    items: RecordTargets,
}

impl Selected {
    const fn borrow(&self) -> Targets<'_> {
        Targets {
            ammunition: &self.ammo,
            items: &self.items,
        }
    }

    fn read(
        parsed: &ParsedCatalogQueries<'_>,
        omitted_ammo: Option<u32>,
        omitted_item: Option<u32>,
    ) -> Result<Self, ReducedError> {
        let mut selected = Self::default();
        parsed.with_ammo_definitions(CatalogRecordSelection::All, None, |snapshot| {
            for ammo in snapshot.rows() {
                let header = ammo.source();
                if header.record_index().value().copied() != omitted_ammo {
                    selected.ammo.insert(ammo_definitions::PATH, header)?;
                }
            }
            Ok::<_, ReducedError>(())
        })??;
        let _ = parsed.visit_item_definitions(
            ItemDefinitionSelection::Records(CatalogRecordSelection::All),
            None,
            |index, snapshot| {
                if Some(index) != omitted_item {
                    let [item] = snapshot.rows() else {
                        return Err(ReducedError::IncompleteScope(item_definitions::PATH));
                    };
                    selected
                        .items
                        .insert(item_definitions::PATH, item.source())?;
                }
                Ok(())
            },
        )?;
        Ok(selected)
    }
}

fn with_queries(
    inspect: impl FnOnce(&ParsedCatalogQueries<'_>, &DataForge<'_>, &Path) -> TestResult,
) -> TestResult {
    let scratch = tempfile::tempdir()?;
    let input = scratch.path().join("Data.p4k");
    let bytes = fixture::database();
    let forge = DataForge::open(&bytes)?;
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", bytes.clone()).encrypted())
        .write_to(&input)?;
    let archive = Archive::open_with_key(input, Some(test_key()))?;
    let session =
        CatalogQuerySession::open(&archive, CatalogLanguageSelection::Selected(&[]), None)?
            .with_provenance_mode(ProvenanceMode::MarkersOnly);
    inspect(&session.parsed(None)?, &forge, scratch.path())
}
