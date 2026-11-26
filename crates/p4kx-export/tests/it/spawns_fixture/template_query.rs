// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::time::UNIX_EPOCH;

use p4kx_export::catalog_queries::contract_template_context::ContractTemplateContextSnapshot;
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, DatasetAvailability,
    ProvenanceMode,
};
use p4kx_export::{CatalogOptions, build_catalog};
use p4kx_extraction::{Archive, Filter};
use p4kx_testkit::{ArchiveBuilder, DataForgeBuilder, TestEntry, TestRecord};
use serde_json::Value;

use super::generator_query::{ENTITIES, LOCATIONS, NPCS, SHIPS};
use super::pool_query_checks::{check_game_text, registered_pool_rows};
use super::{
    ELSEWHERE_TEMPLATE, TEMPLATE, array, english, location, npc, ships, structures, tag_records,
    template_parts,
};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

#[test]
fn template_context_retains_only_selected_canonical_spawn_pools() -> TestResult {
    let Fixture {
        _directory,
        archive,
        selected,
    } = fixture()?;
    let session =
        CatalogQuerySession::open(&archive, CatalogLanguageSelection::AllAvailable, None)?
            .with_provenance_mode(ProvenanceMode::Full);
    let parsed = session.parsed(None)?;
    let all = catalog(&archive, None)?;
    let filter = Filter::new("scoped_anchor");
    let scoped = catalog(&archive, Some(&filter))?;
    assert_eq!(all["exportHealth"]["publicationMode"], "normal");
    assert_eq!(scoped["exportHealth"]["publicationMode"], "normal");

    parsed.with_contract_template_context(CatalogRecordSelection::All, None, |snapshot| {
        check_pools(&snapshot, &all, None)
    })??;
    parsed.with_contract_template_context(
        CatalogRecordSelection::RecordIndex(selected),
        None,
        |snapshot| -> TestResult {
            let [owner] = snapshot.rows.as_slice() else {
                return Err("one selected Template".into());
            };
            assert_eq!(owner.template().record_index(), selected);
            check_pools(&snapshot, &scoped, Some(selected))?;
            check_npc_name(&snapshot, &scoped)
        },
    )??;
    parsed.with_contract_template_context(
        CatalogRecordSelection::RecordIndex(u32::MAX),
        None,
        |snapshot| check_empty(&snapshot),
    )?;
    Ok(())
}

struct Fixture {
    _directory: tempfile::TempDir,
    archive: Archive,
    selected: u32,
}

fn fixture() -> TestResult<Fixture> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("Data.p4k");
    let tags = tag_records();
    let selected = u32::try_from(tags.len())?;
    let database = tags
        .into_iter()
        .chain([
            template(TEMPLATE, "scoped_anchor", 2),
            template(ELSEWHERE_TEMPLATE, "other_owner", 4),
            template_parts::owner(),
        ])
        .fold(
            structures::structures(DataForgeBuilder::new().version(8)),
            DataForgeBuilder::record,
        )
        .build();
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", database).encrypted())
        .entry(english())
        .write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?;
    Ok(Fixture {
        _directory: directory,
        archive,
        selected,
    })
}

fn catalog(archive: &Archive, filter: Option<&Filter>) -> TestResult<Value> {
    let mut options = CatalogOptions::new(UNIX_EPOCH);
    options.filter = filter;
    let built = build_catalog(archive, options.with_full_provenance())?;
    Ok(serde_json::from_slice(built.json())?)
}

fn check_pools(
    snapshot: &ContractTemplateContextSnapshot<'_>,
    expected: &Value,
    owner: Option<u32>,
) -> TestResult {
    let count = if owner.is_some() { 1 } else { 2 };
    assert!(matches!(
        snapshot.availability,
        DatasetAvailability::Available
    ));
    assert_eq!(snapshot.rows.len(), count);
    assert_eq!(snapshot.owners.len(), count);
    assert_eq!(snapshot.coverage.incomplete_fields, [] as [u32; 0]);
    for (path, kind, bytes) in [
        (SHIPS, "ships", serde_json::to_vec(&snapshot.ship_spawns)?),
        (NPCS, "npcs", serde_json::to_vec(&snapshot.npc_spawns)?),
        (
            ENTITIES,
            "entities",
            serde_json::to_vec(&snapshot.entity_spawns)?,
        ),
        (
            LOCATIONS,
            "location",
            serde_json::to_vec(&snapshot.location_queries)?,
        ),
    ] {
        let rows: Vec<Value> = serde_json::from_slice(&bytes)?;
        assert_eq!(rows.len(), count, "{path}");
        assert_eq!(
            rows.iter().collect::<Vec<_>>(),
            producer_rows(expected, path, kind, owner)?,
            "ordered producer rows at {path}"
        );
    }
    Ok(())
}

fn producer_rows<'catalog>(
    catalog: &'catalog Value,
    path: &str,
    kind: &str,
    owner: Option<u32>,
) -> TestResult<Vec<&'catalog Value>> {
    let rows = catalog
        .pointer(path)
        .and_then(Value::as_array)
        .ok_or("producer pool rows")?;
    let Some(owner) = owner else {
        return Ok(rows.iter().collect());
    };
    let projection = catalog
        .pointer("/configurations/contractTemplates/rows")
        .and_then(Value::as_array)
        .and_then(|rows| {
            rows.iter().find(|row| {
                row.pointer("/template/recordIndex").and_then(Value::as_u64)
                    == Some(u64::from(owner))
            })
        })
        .ok_or("selected producer Template projection")?;
    registered_pool_rows(catalog, std::slice::from_ref(projection), path, kind)
}

fn check_npc_name(snapshot: &ContractTemplateContextSnapshot<'_>, catalog: &Value) -> TestResult {
    let name = snapshot.npc_spawns[0].groups[0].options[0]
        .embedded_name
        .as_ref()
        .ok_or("authored NPC name")?;
    let reference = name.given_name.value().ok_or("protected name reference")?;
    let result_text = snapshot
        .game_texts
        .iter()
        .find(|text| text.id().value() == Some(reference))
        .ok_or("NPC pool name resolves in the same result")?;
    check_game_text(catalog, result_text)?;
    let origin = name.given_name.origin();
    assert_eq!(origin.contains_encrypted_source(), Some(true));
    let uses = origin.source_uses_refs();
    assert!(
        !uses.is_empty(),
        "the name retains its authored field origin"
    );
    assert!(uses.iter().all(|reference| {
        snapshot
            .source_inventory
            .iter_uses()
            .any(|source| source.id() == reference)
    }));
    Ok(())
}

fn check_empty(snapshot: &ContractTemplateContextSnapshot<'_>) {
    assert!(matches!(
        snapshot.availability,
        DatasetAvailability::Available
    ));
    assert!(snapshot.owners.is_empty());
    assert!(snapshot.rows.is_empty());
    assert!(snapshot.ship_spawns.is_empty());
    assert!(snapshot.npc_spawns.is_empty());
    assert!(snapshot.entity_spawns.is_empty());
    assert!(snapshot.location_queries.is_empty());
    assert!(snapshot.game_texts.is_empty());
}

fn template(guid: u128, name: &str, weight: i32) -> TestRecord {
    TestRecord::new(
        guid,
        "ContractTemplate",
        format!("ContractTemplate.{name}"),
        format!("libs/foundry/records/contracts/contracttemplates/{name}.xml"),
    )
    .value(
        "contractProperties",
        array(vec![
            ships("Ship", "", Some(weight)),
            npc::npcs("Npc", "", Some((name, npc::NpcKind::Full))),
            npc::entities("Entity", "", Some(name)),
            location("Location", "", Some((1, weight))),
        ]),
    )
}
