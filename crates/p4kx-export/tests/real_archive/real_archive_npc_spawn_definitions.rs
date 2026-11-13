// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;
use std::error::Error;
use std::time::SystemTime;

use p4kx_export::{CatalogOptions, CatalogPublicationMode, Filter, Kind, build_catalog_from};
use p4kx_extraction::{
    Archive, ArchiveWatch, DataForge, DataForgeEntry, RecordTree, TreeBuilder, Value as Original,
};
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde_json::{Value, json};

use crate::authored_definition_witness::{field, verify_array, verify_link, verify_state};
use crate::catalog_v3_consumer as consumer;
use crate::real_archive_verification::print_line;

#[path = "npc_spawn_definition_witness.rs"]
mod spawn;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

struct Family {
    kind: Kind,
    source_type: &'static str,
    field: &'static str,
    verify_entry: fn(&DataForge<'_>, Option<&Original<'_>>, &Value) -> TestResult,
}

const FAMILIES: [Family; 5] = [
    Family {
        kind: Kind::NpcClassPools,
        source_type: "NPCClassPool",
        field: "npcClasses",
        verify_entry: verify_pool_entry,
    },
    Family {
        kind: Kind::NpcArchetypes,
        source_type: "NPCArchetype",
        field: "identity",
        verify_entry: verify_identity_branch,
    },
    Family {
        kind: Kind::AutoSpawns,
        source_type: "AutoSpawn",
        field: "autoSpawnData",
        verify_entry: spawn::verify_spawn,
    },
    Family {
        kind: Kind::AutoSpawnSets,
        source_type: "AutoSpawns",
        field: "autoSpawnData",
        verify_entry: spawn::verify_set_entry,
    },
    Family {
        kind: Kind::EncounterIndices,
        source_type: "EncounterIndex",
        field: "encounters",
        verify_entry: spawn::verify_encounter,
    },
];

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn npc_spawn_definitions_match_original_entries() -> TestResult {
    let path = real_archive_path().map_err(|_| "configured Archive path")?;
    let key = real_archive_key().map_err(|_| "configured Archive key")?;
    let archive = Archive::open_with_key(path, Some(key)).map_err(|_| "open configured Archive")?;
    let watch = ArchiveWatch::new(&archive);
    let entry = DataForgeEntry::read(&archive).map_err(|_| "read original DataForge")?;
    let forge = entry.open().map_err(|_| "open original DataForge")?;
    let filter = std::env::var("P4KX_NPC_DEFINITION_FILTER")
        .ok()
        .map(|pattern| Filter::new(&pattern));
    let options = CatalogOptions {
        filter: filter.as_ref(),
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..CatalogOptions::new(SystemTime::now())
    };
    let built = build_catalog_from(&archive, &entry, &options)
        .map_err(|_| "build NPC spawn definitions catalog")?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    crate::real_archive_shop_inventory_v3::verify_build(&archive, &catalog)?;
    let mut selected = 0;
    for family in &FAMILIES {
        let count = verify_family(&forge, &catalog, family, filter.as_ref())?;
        selected += count;
        print_line(&format!(
            "{}: {count} original definitions matched",
            family.kind
        ));
    }
    assert!(
        selected > 0,
        "at least one independent NPC definition was selected"
    );
    watch
        .check()
        .map_err(|_| "Archive changed during verification")?;
    Ok(())
}

fn verify_family(
    forge: &DataForge<'_>,
    catalog: &Value,
    family: &Family,
    filter: Option<&Filter>,
) -> TestResult<usize> {
    let mut expected = BTreeSet::new();
    for record in forge.records() {
        if record.type_name()? == family.source_type
            && filter
                .is_none_or(|filter| record.record_path().is_ok_and(|path| filter.matches(path)))
        {
            expected.insert((record.guid().to_string(), record.index()));
        }
    }
    let rows = consumer::rows(catalog, family.kind)?;
    let mut builder = TreeBuilder::new();
    for row in rows {
        let (guid, index) = expected.pop_first().ok_or("unexpected definition row")?;
        let record = forge.record(index).ok_or("original definition Record")?;
        let tree = builder
            .build(&record)
            .map_err(|_| "read original definition")?;
        verify_record(forge, family, row, &tree, (&guid, index))?;
    }
    assert!(expected.is_empty(), "every selected definition has a row");
    Ok(rows.len())
}

fn verify_record(
    forge: &DataForge<'_>,
    family: &Family,
    row: &Value,
    tree: &RecordTree<'_>,
    (guid, index): (&str, u32),
) -> TestResult {
    assert_eq!(
        consumer::identity(row, family.kind)?,
        guid,
        "physical GUID and row order"
    );
    assert_eq!(
        consumer::field(row, family.kind, "recordIndex")?,
        &json!(index),
        "physical Record index and row order"
    );
    assert_eq!(
        consumer::field(row, family.kind, "recordPath")?.as_str(),
        Some(tree.record_path()),
        "catalog row uses the original physical Record path"
    );
    if family.kind == Kind::AutoSpawns {
        return (family.verify_entry)(forge, tree.field(family.field), field(row, family.field)?);
    }
    verify_array(
        tree.field(family.field),
        field(row, family.field)?,
        |slot, row| (family.verify_entry)(forge, slot, row),
    )
}

fn verify_pool_entry(
    forge: &DataForge<'_>,
    source: Option<&Original<'_>>,
    row: &Value,
) -> TestResult {
    let Some(Original::Struct(source)) = source else {
        return verify_state(source, row);
    };
    let row = consumer::present(row)?;
    spawn::verify_single(source.field("weight"), field(row, "weight")?)?;
    verify_link(
        forge,
        "EntityClassDefinition",
        source.field("npcClass"),
        field(row, "npcClass")?,
    )
}

fn verify_identity_branch(
    forge: &DataForge<'_>,
    source: Option<&Original<'_>>,
    row: &Value,
) -> TestResult {
    let Some(Original::Struct(source)) = source else {
        return verify_state(source, row);
    };
    let row = consumer::present(row)?;
    verify_link(
        forge,
        "FactionIdentity",
        source.field("identity"),
        field(row, "identity")?,
    )?;
    verify_array(
        source.field("npcPools"),
        field(row, "npcPools")?,
        |slot, row| verify_link(forge, "NPCClassPool", slot, row),
    )
}
