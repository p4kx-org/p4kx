// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::time::SystemTime;

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

mod selected;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const SOURCE_TYPE: &str = "SCItemShieldGeneratorParams";
const ROWS: &str = "/configurations/shieldGenerators/rows";

#[derive(Default)]
struct Counts {
    components: usize,
    shield_fields: selected::ShieldFields,
    stun_fields: selected::StunFields,
    resistance_fields: selected::RangeFields,
    absorption_fields: selected::RangeFields,
    authored_roots: usize,
    authored_nodes: usize,
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn shield_generators_match_original_component_stun_and_range_slots() -> TestResult {
    let path = real_archive_path().map_err(|_| "configured Archive path")?;
    let key = real_archive_key().map_err(|_| "configured Archive key")?;
    let archive = Archive::open_with_key(path, Some(key)).map_err(|_| "open configured Archive")?;
    let watch = ArchiveWatch::new(&archive);
    let entry = DataForgeEntry::read(&archive).map_err(|_| "read configured DataForge")?;
    let forge = entry.open().map_err(|_| "open original DataForge")?;
    let selected = representative(&forge, &watch)?;
    let filter = Filter::new(selected.record_path());
    let options = CatalogOptions {
        filter: Some(&filter),
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..CatalogOptions::new(SystemTime::now())
    };
    let built = build_catalog_from(&archive, &entry, &options)
        .map_err(|_| "build selected Item catalog")?;
    let catalog: Value = serde_json::from_slice(built.json()).map_err(|_| "read catalog JSON")?;
    real_archive_shop_inventory_v3::verify_build(&archive, &catalog)?;
    let counts = compare(&forge, entry.origin(), &catalog, &selected)?;
    watch
        .check()
        .map_err(|_| "Archive changed during comparison")?;
    assert!(
        counts.components > 0,
        "the configured Build must exercise an original shield generator"
    );
    counts.print();
    Ok(())
}

impl Counts {
    fn print(&self) {
        real_archive_verification::print_line(&format!(
            "shield generators: {} original component slots, {} additional roots, {} additional nodes",
            self.components, self.authored_roots, self.authored_nodes,
        ));
        self.shield_fields.print("shield");
        self.stun_fields.print("stun");
        self.resistance_fields.print("resistance");
        self.absorption_fields.print("absorption");
    }
}

fn representative<'source>(
    forge: &'source DataForge<'source>,
    watch: &ArchiveWatch,
) -> TestResult<RecordTree<'source>> {
    let rule = Kind::Items.rule().ok_or("selected Item rule")?;
    let filter = Filter::new(components::FILTER);
    let probe = forge
        .pointer_probe("Components", SOURCE_TYPE)
        .ok_or("original shield declaration")?;
    let mut trees = TreeBuilder::new();
    for record in forge.records() {
        watch
            .poll()
            .map_err(|_| "Archive changed during shield selection")?;
        if !components::selected_item(&record, &rule, &filter)?
            || !components::has_component(&mut trees, &record, std::slice::from_ref(&probe))?
        {
            continue;
        }
        let tree = trees
            .build(&record)
            .map_err(|_| "read original shield owner")?;
        if has_nested_configuration(&tree) {
            return Ok(tree);
        }
    }
    Err("no original shield owner supplies stun and both range arrays".into())
}

fn has_nested_configuration(tree: &RecordTree<'_>) -> bool {
    let Some(Original::Array(components)) = tree.field("Components") else {
        return false;
    };
    components.iter().any(|raw| {
        let Original::Struct(component) = raw else {
            return false;
        };
        component.type_name() == SOURCE_TYPE
            && matches!(component.field("stunParams"), Some(Original::Struct(_)))
            && ["ShieldResistance", "ShieldAbsorption"].iter().all(|field| {
                matches!(component.field(field), Some(Original::Array(slots)) if !slots.is_empty())
            })
    })
}

fn compare(
    forge: &DataForge<'_>,
    origin: &DataForgeOrigin,
    catalog: &Value,
    tree: &RecordTree<'_>,
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
    let mut counts = Counts::default();
    let trace = Trace::of(tree, &uses, &scope, encrypted);
    compare_components(forge, tree, &trace, &mut rows, &mut counts)?;
    assert!(
        rows.keys().all(|(index, _)| *index != tree.record_index()),
        "every shield row has its own original component occurrence"
    );
    assert!(
        proof::text(catalog, "/configurations/shieldGenerators/availability")? == "available",
        "original shield declarations make the source family available"
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
        return Err("the observed shield owner has no readable Components array".into());
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
            .ok_or("one row per physical shield slot")?;
        assert!(
            proof::text(row, "/ownerGuid/value")? == tree.guid().to_string(),
            "the row retains its original owner identity"
        );
        trace.header(cell(row, "ownerGuid")?, "guid", "payload")?;
        trace.header(cell(row, "ownerRecordIndex")?, "recordIndex", "selector")?;
        trace.check(cell(row, "componentOrdinal")?, raw, "conversion")?;
        selected::compare(forge, trace, raw, component, row, counts)?;
        counts.components += 1;
    }
    Ok(())
}
