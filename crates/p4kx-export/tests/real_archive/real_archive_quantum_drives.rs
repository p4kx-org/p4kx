// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::error::Error;
use std::time::UNIX_EPOCH;

use p4kx_export::{CatalogOptions, CatalogPublicationMode, Kind, build_catalog_from};
use p4kx_extraction::{
    Archive, ArchiveWatch, DataForge, DataForgeEntry, DataForgeOrigin, Filter, InputScope,
    RecordTree, SourceEncryption, SourceInput, SourceUses, TreeBuilder, Value as Original,
};
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde_json::Value;

use crate::real_archive_item_components::{self as components, Rows, cell};
use crate::real_archive_item_durability::{self as proof, Trace};
use crate::{real_archive_shop_inventory_v3, real_archive_verification};

#[path = "quantum_drive_observation.rs"]
mod observation;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const SOURCE_TYPE: &str = "SCItemQuantumDriveParams";
const ROWS: &str = "/configurations/quantumDrives/rows";

#[derive(Default)]
struct Counts {
    components: usize,
    numbers: usize,
    blocks: BTreeMap<(&'static str, observation::State), usize>,
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn quantum_drives_match_both_original_modes_heat_and_observed_boost() -> TestResult {
    let path = real_archive_path().map_err(|_| "configured Archive path")?;
    let key = real_archive_key().map_err(|_| "configured Archive key")?;
    let archive = Archive::open_with_key(path, Some(key)).map_err(|_| "open configured Archive")?;
    let watch = ArchiveWatch::new(&archive);
    let entry = DataForgeEntry::read(&archive).map_err(|_| "read configured DataForge")?;
    let filter = Filter::new(components::FILTER);
    let options = CatalogOptions {
        filter: Some(&filter),
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..CatalogOptions::new(UNIX_EPOCH)
    };
    let built = build_catalog_from(&archive, &entry, &options)
        .map_err(|_| "build selected Item catalog")?;
    let catalog: Value = serde_json::from_slice(built.json()).map_err(|_| "read catalog JSON")?;
    real_archive_shop_inventory_v3::verify_build(&archive, &catalog)?;
    let forge = entry.open().map_err(|_| "open original DataForge")?;
    let counts = compare(&forge, entry.origin(), &catalog, &filter, &watch)?;
    watch
        .check()
        .map_err(|_| "Archive changed during comparison")?;
    assert!(
        counts.components > 0,
        "the configured Build must exercise an original quantum drive"
    );
    let identity = archive.build().ok_or("complete actual Build identity")?;
    real_archive_verification::print_line(&format!(
        "quantum drives in {} {} {} {}: {} component slots, {} original numeric cells",
        identity.branch(),
        identity.version(),
        identity.change_number(),
        identity.build_id(),
        counts.components,
        counts.numbers,
    ));
    for ((block, state), count) in counts.blocks {
        real_archive_verification::print_line(&format!("{block} {}: {count}", state.label()));
    }
    Ok(())
}

fn compare(
    forge: &DataForge<'_>,
    origin: &DataForgeOrigin,
    catalog: &Value,
    filter: &Filter,
    watch: &ArchiveWatch,
) -> TestResult<Counts> {
    let mut rows = components::component_rows(catalog, ROWS)?;
    let uses = proof::indexed(catalog, "/provenance/uses", "id")?;
    let DataForgeOrigin::Archive(entry) = origin else {
        return Err("expected an observed Archive DataForge".into());
    };
    let sources = SourceUses::new();
    let scope = sources
        .observe(SourceInput::Archive(entry), InputScope::Payload)
        .map_err(|_| "observe original DataForge origin")?;
    let encrypted = match origin.encryption() {
        SourceEncryption::Encrypted => Some(true),
        SourceEncryption::Unencrypted => Some(false),
        _ => None,
    };
    let rule = Kind::Items.rule().ok_or("selected Item rule")?;
    let probe = forge
        .pointer_probe("Components", SOURCE_TYPE)
        .ok_or("original quantum declaration")?;
    let mut trees = TreeBuilder::new();
    let mut counts = Counts::default();
    for record in forge.records() {
        watch
            .poll()
            .map_err(|_| "Archive changed during source selection")?;
        if !components::selected_item(&record, &rule, filter)?
            || !components::has_component(&mut trees, &record, std::slice::from_ref(&probe))?
        {
            continue;
        }
        let tree = trees
            .build(&record)
            .map_err(|_| "read original quantum owner")?;
        let trace = Trace::of(&tree, &uses, &scope, encrypted);
        compare_components(forge, &tree, &trace, &mut rows, &mut counts)?;
    }
    assert!(
        rows.is_empty(),
        "every quantum row has its own original physical occurrence"
    );
    assert!(
        proof::text(catalog, "/configurations/quantumDrives/availability")? == "available",
        "the original quantum source family is available"
    );
    Ok(counts)
}

fn compare_components<'source>(
    forge: &DataForge<'_>,
    tree: &RecordTree<'source>,
    trace: &Trace<'_, 'source>,
    rows: &mut Rows<'_>,
    counts: &mut Counts,
) -> TestResult {
    let Some(Original::Array(components)) = tree.field("Components") else {
        return Err("the observed quantum owner has no readable Components array".into());
    };
    for (slot, raw) in components.iter().enumerate() {
        let Original::Struct(component) = raw else {
            continue;
        };
        if component.type_name() != SOURCE_TYPE {
            continue;
        }
        let row = rows
            .remove(&(tree.record_index(), slot))
            .ok_or("one row per physical quantum slot")?;
        assert!(
            proof::text(row, "/ownerGuid/value")? == tree.guid().to_string(),
            "original owner identity"
        );
        trace.header(cell(row, "ownerGuid")?, "guid", "payload")?;
        trace.header(cell(row, "ownerRecordIndex")?, "recordIndex", "selector")?;
        trace.check(cell(row, "componentOrdinal")?, raw, "conversion")?;
        observation::compare(forge, trace, raw, component, row, counts)?;
        counts.components += 1;
    }
    Ok(())
}
