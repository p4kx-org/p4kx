// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet};
use std::time::UNIX_EPOCH;

use p4kx_export::{CatalogOptions, CatalogPublicationMode, Kind, build_catalog_from};
use p4kx_extraction::{
    Archive, ArchiveWatch, DataForge, DataForgeEntry, DataForgeOrigin, Filter, InputScope,
    RecordTree, SourceEncryption, SourceInput, SourceUses, StructValue, TreeBuilder,
    Value as Original,
};
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde_json::Value;

use crate::original_item_arena as arena;
use crate::real_archive_item_components::{self as components, cell};
use crate::real_archive_item_durability::{self as proof, Trace};
use crate::real_archive_shop_inventory_v3;
use crate::real_archive_verification::print_line;

#[path = "missile_configuration_observation.rs"]
mod observation;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

const MISSILE: &str = "SCItemMissileParams";
const ROWS: &str = "/configurations/missileConfigurations/rows";

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn source_selected_missile_signal_families_match_original_guidance_and_targeting() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, Some(real_archive_key()?))
        .map_err(|_| "open configured Archive")?;
    let watch = ArchiveWatch::new(&archive);
    let entry = DataForgeEntry::read(&archive).map_err(|_| "read original DataForge")?;
    let forge = entry.open().map_err(|_| "open original DataForge")?;
    let selected = select(&forge, &watch)?;
    assert!(
        selected.len() >= 2,
        "at least two original missile signal families must be compared"
    );
    let mut coverage = observation::Coverage::default();
    for tree in selected.values() {
        let catalog = build(&archive, &entry, tree)?;
        compare(&catalog, entry.origin(), tree, &mut coverage)?;
    }
    watch
        .check()
        .map_err(|_| "Archive changed during verification")?;
    print_line(&format!(
        "Missile source selection: {} original signal families",
        selected.len()
    ));
    coverage.report();
    Ok(())
}

fn select<'source>(
    forge: &'source DataForge<'source>,
    watch: &ArchiveWatch,
) -> TestResult<BTreeMap<&'source str, RecordTree<'source>>> {
    let rule = Kind::Items.rule().ok_or("selected Item rule")?;
    let filter = Filter::new(components::FILTER);
    let probe = forge
        .pointer_probe("Components", MISSILE)
        .ok_or("original missile declaration")?;
    let mut trees = TreeBuilder::new();
    let mut selected = BTreeMap::new();
    let mut selected_owners = BTreeSet::new();
    for record in forge.records() {
        watch
            .poll()
            .map_err(|_| "Archive changed during source selection")?;
        if !components::selected_item(&record, &rule, &filter)?
            || !components::has_component(&mut trees, &record, std::slice::from_ref(&probe))?
        {
            continue;
        }
        let tree = trees
            .build(&record)
            .map_err(|_| "read original missile owner")?;
        let candidates = slots(&tree)?.iter().filter_map(signal);
        for name in candidates {
            if !selected.contains_key(name) && selected_owners.insert(tree.record_index()) {
                selected.insert(name, tree);
                break;
            }
        }
    }
    Ok(selected)
}

fn slots<'tree, 'source>(
    tree: &'tree RecordTree<'source>,
) -> TestResult<&'tree [Original<'source>]> {
    let Some(Original::Array(slots)) = tree.field("Components") else {
        return Err("original missile owner must contain Components".into());
    };
    Ok(slots)
}

fn signal<'source>(raw: &Original<'source>) -> Option<&'source str> {
    let Original::Struct(source) = raw else {
        return None;
    };
    if source.type_name() != MISSILE {
        return None;
    }
    let Original::Struct(targeting) = source.field("targetingParams")? else {
        return None;
    };
    let Original::Enum(label) = targeting.field("trackingSignalType")? else {
        return None;
    };
    Some(label)
}

fn build(archive: &Archive, entry: &DataForgeEntry, tree: &RecordTree<'_>) -> TestResult<Value> {
    let filter = Filter::new(tree.record_path());
    let options = CatalogOptions {
        filter: Some(&filter),
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..CatalogOptions::new(UNIX_EPOCH)
    };
    let built = build_catalog_from(archive, entry, &options)
        .map_err(|_| "build selected missile catalog")?;
    let items = built
        .summary()
        .export_health()
        .kind(Kind::Items)
        .ok_or("selected Item health")?;
    assert!(
        items.source_inventory_complete
            && items.field_inventory_complete
            && items.failed_records().is_empty()
            && items.failed_entries().is_empty(),
        "the selected Item scope has a complete source inventory with no failed reads"
    );
    let catalog = serde_json::from_slice(built.json()).map_err(|_| "read scoped catalog JSON")?;
    real_archive_shop_inventory_v3::verify_build(archive, &catalog)?;
    Ok(catalog)
}

fn compare(
    catalog: &Value,
    origin: &DataForgeOrigin,
    tree: &RecordTree<'_>,
    coverage: &mut observation::Coverage,
) -> TestResult {
    let DataForgeOrigin::Archive(entry) = origin else {
        return Err("observed Archive DataForge".into());
    };
    let uses = proof::indexed(catalog, "/provenance/uses", "id")?;
    let sources = SourceUses::new();
    let scope = sources.observe(SourceInput::Archive(entry), InputScope::Payload)?;
    let encrypted = match origin.encryption() {
        SourceEncryption::Encrypted => Some(true),
        SourceEncryption::Unencrypted => Some(false),
        _ => None,
    };
    let trace = Trace::of(tree, &uses, &scope, encrypted);
    let mut rows = components::component_rows(catalog, ROWS)?;
    let ordnance = components::component_rows(catalog, "/configurations/explosives/rows")?;
    let mut compared = 0;
    for (slot, raw) in slots(tree)?.iter().enumerate() {
        let Original::Struct(source) = raw else {
            continue;
        };
        if source.type_name() != MISSILE {
            continue;
        }
        let identity = (tree.record_index(), slot);
        let row = rows
            .remove(&identity)
            .ok_or("one row per physical missile slot")?;
        let shared = ordnance
            .get(&identity)
            .ok_or("the same physical slot owns shared ordnance")?;
        verify_identity(tree, &trace, raw, source, row, shared)?;
        observation::compare(&trace, source, row, coverage)?;
        compared += 1;
    }
    assert!(
        compared > 0 && !rows.keys().any(|(owner, _)| *owner == tree.record_index()),
        "scoped missile rows follow the selected original slots"
    );
    Ok(())
}

fn verify_identity<'source>(
    tree: &RecordTree<'source>,
    trace: &Trace<'_, 'source>,
    raw: &Original<'source>,
    source: &StructValue<'source>,
    row: &Value,
    ordnance: &Value,
) -> TestResult {
    let guid = tree.guid().to_string();
    assert_eq!(
        proof::text(row, "/ownerGuid/value")?,
        guid,
        "the actual owner GUID matches"
    );
    for field in ["ownerGuid", "ownerRecordIndex", "componentOrdinal"] {
        assert_eq!(
            cell(row, field)?,
            cell(ordnance, field)?,
            "missile and ordnance rows share physical owner and slot"
        );
    }
    trace.header(cell(row, "ownerGuid")?, "guid", "payload")?;
    trace.header(cell(row, "ownerRecordIndex")?, "recordIndex", "selector")?;
    trace.check(cell(row, "componentOrdinal")?, raw, "conversion")?;
    arena::source_type(trace, raw, source, row)
}
