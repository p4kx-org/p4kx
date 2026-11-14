// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet};
use std::time::UNIX_EPOCH;

use p4kx_export::{CatalogOptions, CatalogPublicationMode, Filter, Kind, build_catalog_from};
use p4kx_extraction::{
    Archive, ArchiveWatch, DataForge, DataForgeEntry, TreeBuilder, Value as Original,
};
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde_json::Value;

use crate::catalog_v3_consumer as consumer;
use crate::real_archive_item_durability::number;
use crate::real_archive_verification::print_line;

#[path = "real_archive_module_definition_witness.rs"]
mod definitions;
#[path = "real_archive_module_objective_witness.rs"]
mod objectives;
#[path = "real_archive_module_source_receipt.rs"]
mod receipts;

type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
const SOURCE: &str = "ModuleDeclaration";
const MODULE_KIND: Kind = Kind::MissionModuleDeclarations;
const TEMPLATE_KIND: Kind = Kind::ContractTemplates;

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn original_module_declarations_and_objective_references_match_catalog() -> Result {
    let archive = Archive::open_with_key(real_archive_path()?, Some(real_archive_key()?))
        .map_err(|_| "open installed Archive")?;
    let watch = ArchiveWatch::new(&archive);
    let entry = DataForgeEntry::read(&archive).map_err(|_| "read original DataForge")?;
    let forge = entry.open().map_err(|_| "open original DataForge")?;
    let sample = select_definitions(&forge)?;
    let callers = objectives::select(&forge)?;
    if sample.is_empty() && callers.is_empty() {
        print_line(
            "ModuleDeclaration roots and Local/PlayerAttached objective module links are absent in the examined source families",
        );
        return watch
            .check()
            .map_err(|_| "Archive changed during comparison".into());
    }

    let (mut builds, mut checked_targets) = verify_definitions(&forge, &archive, &entry, &sample)?;
    for caller in &callers {
        let catalog = build(&archive, &entry, caller.path)?;
        let source = receipts::Receipt::new(&catalog, entry.origin())?;
        let targets = objectives::verify(&forge, &source, &catalog, caller)?;
        for target in targets {
            if checked_targets.insert(target) {
                verify_target(&forge, &archive, &entry, target)?;
                builds += 1;
            }
        }
        builds += 1;
    }
    report(&sample, &callers, builds);
    watch
        .check()
        .map_err(|_| "Archive changed during comparison".into())
}

fn verify_definitions(
    forge: &DataForge<'_>,
    archive: &Archive,
    entry: &DataForgeEntry,
    sample: &BTreeMap<u32, &str>,
) -> Result<(usize, BTreeSet<u32>)> {
    for (index, path) in sample {
        let catalog = build(archive, entry, path)?;
        let receipt = receipts::Receipt::new(&catalog, entry.origin())?;
        verify_definition(forge, &catalog, &receipt, *index)?;
    }
    Ok((sample.len(), sample.keys().copied().collect()))
}

fn verify_definition(
    forge: &DataForge<'_>,
    catalog: &Value,
    receipt: &receipts::Receipt<'_>,
    index: u32,
) -> Result {
    let row = definition(catalog, index)?;
    let original = forge
        .record(index)
        .ok_or("selected original module Record")?;
    let tree = original
        .tree()
        .map_err(|_| "read selected original module")?;
    definitions::verify(forge, receipt, &tree, row)
}

fn report(sample: &BTreeMap<u32, &str>, callers: &[objectives::Caller<'_>], builds: usize) {
    print_line(&format!(
        "ModuleDeclaration: {} selected physical definitions, {builds} Record-path diagnostic catalog runs",
        sample.len()
    ));
    for source in [objectives::LOCAL, objectives::PLAYER] {
        let count = callers.iter().filter(|caller| caller.has(source)).count();
        let label = if count == 0 {
            "no resolved source Reference observed"
        } else {
            "positive original Reference compared"
        };
        print_line(&format!("{source}: {label}"));
    }
}

fn select_definitions<'source>(
    forge: &'source DataForge<'source>,
) -> Result<BTreeMap<u32, &'source str>> {
    let mut builder = TreeBuilder::new();
    let mut sample = BTreeMap::new();
    let mut coverage = BTreeSet::new();
    for record in forge
        .records()
        .filter(|record| record.type_name().ok() == Some(SOURCE))
    {
        let tree = builder
            .build(&record)
            .map_err(|_| "read scoped original module Record")?;
        let module_type = match tree.field("moduleType") {
            Some(Original::Struct(data)) => data.type_name(),
            Some(Original::Null) => "null",
            None => "absent",
            _ => "unreadable",
        };
        let has_npc = matches!(
            tree.field("spawnedEntities"),
            Some(Original::Array(slots)) if slots.iter().any(|slot| {
                matches!(slot, Original::Struct(spawn) if spawn.type_name() == "ModuleSpawnedEntities_NPC")
            })
        );
        if coverage.insert((module_type, has_npc)) {
            sample.insert(record.index(), record.record_path()?);
        }
    }
    print_line(if coverage.iter().any(|(_, has_npc)| *has_npc) {
        "ModuleSpawnedEntities_NPC: positive original slot selected"
    } else {
        "ModuleSpawnedEntities_NPC: no original populated slot observed"
    });
    Ok(sample)
}

fn build(archive: &Archive, entry: &DataForgeEntry, path: &str) -> Result<Value> {
    let filter = Filter::parse(path).map_err(|_| "select original Record path")?;
    let options = CatalogOptions {
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        filter: Some(&filter),
        ..CatalogOptions::new(UNIX_EPOCH)
    };
    let built = build_catalog_from(archive, entry, &options)
        .map_err(|_| "build scoped diagnostic catalog")?;
    let catalog: Value =
        serde_json::from_slice(built.json()).map_err(|_| "read scoped catalog JSON")?;
    crate::real_archive_shop_inventory_v3::verify_build(archive, &catalog)?;
    assert_eq!(
        catalog.get("schemaVersion").and_then(Value::as_u64),
        Some(3),
        "the production catalog schema"
    );
    Ok(catalog)
}

fn definition(catalog: &Value, index: u32) -> Result<&Value> {
    let rows = consumer::rows(catalog, MODULE_KIND)?;
    by_index(rows, index, "the physical module definition appears once")
}

fn by_index<'a>(rows: &'a [Value], index: u32, missing: &'static str) -> Result<&'a Value> {
    let mut matches = rows
        .iter()
        .filter(|row| number(row, "/recordIndex/value").ok() == Some(u64::from(index)));
    let row = matches.next().ok_or(missing)?;
    assert!(matches.next().is_none(), "{missing}");
    Ok(row)
}

fn verify_target(
    forge: &DataForge<'_>,
    archive: &Archive,
    entry: &DataForgeEntry,
    index: u32,
) -> Result {
    let record = forge
        .record(index)
        .ok_or("actual referenced module Record")?;
    let catalog = build(archive, entry, record.record_path()?)?;
    let row = definition(&catalog, index)?;
    let source = receipts::Receipt::new(&catalog, entry.origin())?;
    let tree = record
        .tree()
        .map_err(|_| "read actual referenced module Record")?;
    definitions::header(&source, &tree, row)
}
