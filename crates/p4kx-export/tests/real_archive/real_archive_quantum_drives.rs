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
    verify_archive(Selection::AllItems)
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn source_selected_quantum_modes_and_heat_match_exact_original_fields() -> TestResult {
    verify_archive(Selection::Representative)
}

#[derive(Clone, Copy)]
enum Selection {
    AllItems,
    Representative,
}

fn verify_archive(selection: Selection) -> TestResult {
    let path = real_archive_path().map_err(|_| "configured Archive path")?;
    let key = real_archive_key().map_err(|_| "configured Archive key")?;
    let archive = Archive::open_with_key(path, Some(key)).map_err(|_| "open configured Archive")?;
    let watch = ArchiveWatch::new(&archive);
    let entry = DataForgeEntry::read(&archive).map_err(|_| "read configured DataForge")?;
    let forge = entry.open().map_err(|_| "open original DataForge")?;
    let mut counts = Counts::default();
    match selection {
        Selection::AllItems => {
            let filter = Filter::new(components::FILTER);
            verify_catalog(
                &archive,
                &entry,
                &forge,
                &filter,
                &watch,
                &mut counts,
                selection,
            )?;
        }
        Selection::Representative => {
            let witnesses = witnesses(&forge, &watch)?;
            for path in witnesses.paths() {
                let filter = Filter::new(path);
                verify_catalog(
                    &archive,
                    &entry,
                    &forge,
                    &filter,
                    &watch,
                    &mut counts,
                    selection,
                )?;
            }
            verify_witness_coverage(&witnesses, &counts);
        }
    }
    watch
        .check()
        .map_err(|_| "Archive changed during comparison")?;
    report(&archive, counts)
}

fn verify_witness_coverage(witnesses: &Witnesses<'_>, counts: &Counts) {
    assert!(
        ["standard", "spline", "heat"]
            .into_iter()
            .all(|block| counts
                .blocks
                .contains_key(&(block, observation::State::Present))),
        "all three required source blocks are present and compared"
    );
    if witnesses.boost_path.is_some() {
        assert!(
            counts
                .blocks
                .contains_key(&("boost", observation::State::Present)),
            "observed boost has a typed export"
        );
    }
    real_archive_verification::print_line(&format!(
        "quantum source selection: {} owner filters, boost declaration: {}, boost observed: {}",
        witnesses.paths().count(),
        witnesses.boost_declared,
        witnesses.boost_path.is_some()
    ));
}

fn report(archive: &Archive, counts: Counts) -> TestResult {
    assert!(
        counts.components > 0,
        "the configured Build must exercise an original quantum drive"
    );
    let identity = archive.build().ok_or("complete actual Build identity")?;
    real_archive_verification::print_line(&format!(
        "quantum drives in {}: {} component slots, {} original numeric cells",
        identity.branch(),
        counts.components,
        counts.numbers,
    ));
    for ((block, state), count) in counts.blocks {
        real_archive_verification::print_line(&format!("{block} {}: {count}", state.label()));
    }
    Ok(())
}

fn verify_catalog(
    archive: &Archive,
    entry: &DataForgeEntry,
    forge: &DataForge<'_>,
    filter: &Filter,
    watch: &ArchiveWatch,
    counts: &mut Counts,
    selection: Selection,
) -> TestResult {
    let options = CatalogOptions {
        filter: Some(filter),
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..CatalogOptions::new(UNIX_EPOCH)
    };
    let built =
        build_catalog_from(archive, entry, &options).map_err(|_| "build selected Item catalog")?;
    if matches!(selection, Selection::Representative) {
        let health = built.summary().export_health();
        let items = health.kind(Kind::Items).ok_or("selected Item health")?;
        assert!(
            items.source_inventory_complete
                && items.field_inventory_complete
                && items.failed_records().is_empty()
                && items.failed_entries().is_empty(),
            "the selected Item scope has a complete source inventory with no failed reads"
        );
    }
    let catalog: Value = serde_json::from_slice(built.json()).map_err(|_| "read catalog JSON")?;
    real_archive_shop_inventory_v3::verify_build(archive, &catalog)?;
    compare(forge, entry.origin(), &catalog, filter, watch, counts)
}

#[derive(Default)]
struct Witnesses<'source> {
    modes_and_heat: Option<&'source str>,
    boost_path: Option<&'source str>,
    boost_declared: bool,
}

impl<'source> Witnesses<'source> {
    fn paths(&self) -> impl Iterator<Item = &'source str> {
        self.modes_and_heat.into_iter().chain(
            self.boost_path
                .filter(|path| Some(*path) != self.modes_and_heat),
        )
    }
}

fn witnesses<'source>(
    forge: &'source DataForge<'source>,
    watch: &ArchiveWatch,
) -> TestResult<Witnesses<'source>> {
    let rule = Kind::Items.rule().ok_or("selected Item rule")?;
    let filter = Filter::new(components::FILTER);
    let probe = forge
        .pointer_probe("Components", SOURCE_TYPE)
        .ok_or("original quantum declaration")?;
    let mut trees = TreeBuilder::new();
    let mut selection = Witnesses::default();
    for record in forge.records() {
        watch
            .poll()
            .map_err(|_| "Archive changed during source selection")?;
        if !components::selected_item(&record, &rule, &filter)?
            || !components::has_component(&mut trees, &record, std::slice::from_ref(&probe))?
        {
            continue;
        }
        let path = record
            .record_path()
            .map_err(|_| "read original Quantum Record path")?;
        let tree = trees
            .build(&record)
            .map_err(|_| "read original quantum owner")?;
        observe_witnesses(&mut selection, &tree, path)?;
        if selection.modes_and_heat.is_some() && selection.boost_path.is_some() {
            break;
        }
    }
    assert!(
        selection.modes_and_heat.is_some(),
        "an original owner must have distinct standard and spline drive speeds and all five thermal values"
    );
    Ok(selection)
}

fn observe_witnesses<'source>(
    selection: &mut Witnesses<'source>,
    tree: &RecordTree<'source>,
    path: &'source str,
) -> TestResult {
    let Some(Original::Array(components)) = tree.field("Components") else {
        return Err("original Quantum Components array".into());
    };
    for value in components {
        let Original::Struct(component) = value else {
            continue;
        };
        if component.type_name() != SOURCE_TYPE {
            continue;
        }
        if selection.modes_and_heat.is_none() && observation::witnesses_modes_and_heat(component)? {
            selection.modes_and_heat = Some(path);
        }
        if let Some(boost) = component.field("quantumBoostParams") {
            selection.boost_declared = true;
            if matches!(boost, Original::Struct(_)) {
                selection.boost_path.get_or_insert(path);
            }
        }
    }
    Ok(())
}

fn compare(
    forge: &DataForge<'_>,
    origin: &DataForgeOrigin,
    catalog: &Value,
    filter: &Filter,
    watch: &ArchiveWatch,
    counts: &mut Counts,
) -> TestResult {
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
        compare_components(forge, &tree, &trace, &mut rows, counts)?;
    }
    assert!(
        rows.is_empty(),
        "every quantum row has its own original physical occurrence"
    );
    assert!(
        proof::text(catalog, "/configurations/quantumDrives/availability")? == "available",
        "the original quantum source family is available"
    );
    Ok(())
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
