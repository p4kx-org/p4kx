// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::error::Error;
use std::time::UNIX_EPOCH;

use p4kx_export::{CatalogOptions, CatalogPublicationMode, Kind, build_catalog_from};
use p4kx_extraction::{
    Archive, ArchiveWatch, DataForgeEntry, DataForgeOrigin, Filter, InputScope, Record, RecordTree,
    SourceEncryption, SourceInput, SourceUses, Value as Original,
};
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde_json::Value;

use crate::real_archive_item_components::{cell, component_rows, selected_item};
use crate::real_archive_item_durability::{self as proof, Trace};
use crate::real_archive_verification;

#[path = "weapon_actions/graph.rs"]
mod graph;
#[path = "weapon_actions/inputs.rs"]
mod inputs;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const COMPONENT: &str = "SCItemWeaponComponentParams";
const WITNESSES: &[(&str, &str)] = &[
    ("P8-SC", "behr_smg_ballistic_01"),
    ("Volt", "volt_sniper_energy_01"),
    ("Revenant", "apar_ballisticgatling_s4"),
];

#[derive(Default)]
struct Coverage {
    nodes: usize,
    inputs: usize,
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn representative_weapon_actions_match_original_slots_and_inputs() -> TestResult {
    let path = real_archive_path().map_err(|_| "configured Archive path")?;
    let key = real_archive_key().map_err(|_| "configured Archive key")?;
    let archive = Archive::open_with_key(path, Some(key)).map_err(|_| "open configured Archive")?;
    let watch = ArchiveWatch::new(&archive);
    let entry = DataForgeEntry::read(&archive).map_err(|_| "read configured DataForge")?;
    let forge = entry.open().map_err(|_| "open original DataForge")?;
    let mut coverage = BTreeMap::new();
    for (label, basename) in WITNESSES {
        let mut owners = 0;
        for record in forge.records() {
            watch
                .poll()
                .map_err(|_| "Archive changed during source selection")?;
            if !is_witness(&record, basename)? {
                continue;
            }
            compare(&archive, &entry, &record, &mut coverage)?;
            owners += 1;
        }
        real_archive_verification::print_line(&format!(
            "weapon action witness {label}: {owners} original owners"
        ));
    }
    watch
        .check()
        .map_err(|_| "Archive changed during verification")?;
    assert!(
        !coverage.is_empty(),
        "at least one representative weapon is present"
    );
    for (kind, measured) in coverage {
        real_archive_verification::print_line(&format!(
            "weapon action {kind}: {} original nodes, {} original input fields checked",
            measured.nodes, measured.inputs
        ));
    }
    Ok(())
}

fn is_witness(record: &Record<'_>, basename: &str) -> TestResult<bool> {
    let Ok(path) = record.record_path() else {
        return Ok(false);
    };
    let Some(stem) = path.rsplit(['/', '\\']).next() else {
        return Ok(false);
    };
    if !stem.eq_ignore_ascii_case(basename) {
        return Ok(false);
    }
    let rule = Kind::Items.rule().ok_or("selected Item rule")?;
    selected_item(record, &rule, &Filter::new(path))
}

fn compare(
    archive: &Archive,
    entry: &DataForgeEntry,
    record: &Record<'_>,
    coverage: &mut BTreeMap<String, Coverage>,
) -> TestResult {
    let tree = record.tree().map_err(|_| "read original weapon owner")?;
    let filter = Filter::new(tree.record_path());
    let options = CatalogOptions {
        filter: Some(&filter),
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..CatalogOptions::new(UNIX_EPOCH)
    };
    let catalog = build_catalog_from(archive, entry, &options)
        .map_err(|_| "build selected weapon owner catalog")?;
    let catalog: Value = serde_json::from_slice(catalog.json()).map_err(|_| "read catalog JSON")?;
    compare_owner(&tree, entry.origin(), &catalog, coverage)
}

fn compare_owner(
    tree: &RecordTree<'_>,
    origin: &DataForgeOrigin,
    catalog: &Value,
    coverage: &mut BTreeMap<String, Coverage>,
) -> TestResult {
    let uses = proof::indexed(catalog, "/provenance/uses", "id")?;
    let DataForgeOrigin::Archive(entry) = origin else {
        return Err("expected an observed Archive DataForge".into());
    };
    let sources = SourceUses::new();
    let scope = sources.observe(SourceInput::Archive(entry), InputScope::Payload)?;
    let encrypted = match origin.encryption() {
        SourceEncryption::Encrypted => Some(true),
        SourceEncryption::Unencrypted => Some(false),
        _ => None,
    };
    let trace = Trace::of(tree, &uses, &scope, encrypted);
    let mut rows = component_rows(catalog, "/configurations/weaponActions/rows")?;
    rows.retain(|(owner, _), _| *owner == tree.record_index());
    let Some(Original::Array(components)) = tree.field("Components") else {
        return Err("representative weapon has an original Components array".into());
    };
    let mut weapons = 0;
    for (ordinal, value) in components.iter().enumerate() {
        let Original::Struct(weapon) = value else {
            continue;
        };
        if weapon.type_name() != COMPONENT {
            continue;
        }
        let row = rows
            .remove(&(tree.record_index(), ordinal))
            .ok_or("physical weapon action row")?;
        assert!(
            proof::text(row, "/ownerGuid/value")? == tree.guid().to_string(),
            "weapon action row identifies the original owner"
        );
        trace.header(cell(row, "ownerGuid")?, "guid", "payload")?;
        trace.check(cell(row, "componentOrdinal")?, value, "conversion")?;
        trace.header(cell(row, "ownerRecordIndex")?, "recordIndex", "selector")?;
        graph::compare(&trace, weapon, row, coverage)?;
        weapons += 1;
    }
    assert!(
        weapons > 0 && rows.is_empty(),
        "every exported weapon for this owner has its physical source slot (original={weapons}, unmatched={})",
        rows.len()
    );
    Ok(())
}
