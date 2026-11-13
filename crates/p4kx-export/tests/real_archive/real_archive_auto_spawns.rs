// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::time::UNIX_EPOCH;

use p4kx_export::{CatalogOptions, CatalogPublicationMode, build_catalog_from};
use p4kx_extraction::{
    Archive, ArchiveWatch, DataForge, DataForgeEntry, Filter, RecordTree, TreeBuilder,
};
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde_json::Value;

use crate::real_archive_contract_templates::mission_graph::{self as graph, Family};

#[path = "auto_spawns/original.rs"]
mod original;
#[path = "auto_spawns/provenance.rs"]
mod provenance;
#[path = "auto_spawns/published.rs"]
mod published;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn selected_auto_spawns_match_authored_references_and_closet_search() -> TestResult {
    let path = real_archive_path().map_err(|_| "configured Archive path")?;
    let key = real_archive_key().map_err(|_| "configured Archive key")?;
    let archive = Archive::open_with_key(path, Some(key)).map_err(|_| "open configured Archive")?;
    let watch = ArchiveWatch::new(&archive);
    watch.check().map_err(|_| "Archive changed")?;
    let entry = DataForgeEntry::read(&archive).map_err(|_| "read original DataForge")?;
    let forge = entry.open().map_err(|_| "open original DataForge")?;
    let require_closet = std::env::var_os("P4KX_EXPECT_AUTO_SPAWNS_CLOSET_TAGS").is_some();
    let witnesses = find_witnesses(&forge, &watch, require_closet)?;
    for witness in &witnesses {
        compare(&archive, &entry, &watch, witness)?;
    }
    watch.check().map_err(|_| "Archive changed")?;
    Ok(())
}

struct Witness<'source> {
    family: Family,
    tree: RecordTree<'source>,
}

fn find_witnesses<'source>(
    forge: &'source DataForge<'source>,
    watch: &ArchiveWatch,
    require_closet: bool,
) -> TestResult<Vec<Witness<'source>>> {
    let mut builder = TreeBuilder::new();
    let mut template = None;
    let mut broker = None;
    for record in forge.records() {
        watch.poll().map_err(|_| "Archive changed")?;
        let (slot, family) = match record.type_name().map_err(|_| "read Record type")? {
            "ContractTemplate" if template.is_none() => (&mut template, Family::Template),
            "MissionBrokerEntry" if broker.is_none() => (&mut broker, Family::Broker),
            _ => continue,
        };
        let tree = builder.build(&record).map_err(|_| "read mission owner")?;
        let values = original::selected(tree.root(), family);
        let supported = match family {
            Family::Template if require_closet => values.iter().any(original::populated_closet),
            Family::Template => values.iter().any(original::resolved_reference),
            Family::Broker => !values.is_empty(),
        };
        if supported {
            *slot = Some(Witness { family, tree });
        }
        if template.is_some() && broker.is_some() {
            break;
        }
    }
    let expected = if require_closet {
        "Archive lacks a selected Template with a readable AutoSpawns target and authored closet tags"
    } else {
        "Archive lacks a selected Template with a readable AutoSpawns target"
    };
    let template = template.ok_or(expected)?;
    Ok(std::iter::once(template).chain(broker).collect())
}

fn compare(
    archive: &Archive,
    entry: &DataForgeEntry,
    watch: &ArchiveWatch,
    witness: &Witness<'_>,
) -> TestResult {
    let tree = &witness.tree;
    let filter = Filter::new(tree.record_path());
    let options = CatalogOptions {
        filter: Some(&filter),
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..CatalogOptions::new(UNIX_EPOCH)
    };
    watch.check().map_err(|_| "Archive changed")?;
    let built = build_catalog_from(archive, entry, &options)
        .map_err(|_| "build selected AutoSpawns owner")?;
    let catalog: Value = serde_json::from_slice(built.json()).map_err(|_| "read catalog JSON")?;
    let mission = physical_context(&catalog, witness)?;
    let origins = provenance::Origins::read(&catalog, entry.origin())?;
    origins.check_owner(mission.definition)?;
    let selected = original::selected(tree.root(), witness.family);
    let locations = original::Locations::read(tree.root());
    published::compare(
        &mission,
        &selected,
        &locations,
        &origins,
        tree.record_index(),
    )
}

fn physical_context<'catalog>(
    catalog: &'catalog Value,
    witness: &Witness<'_>,
) -> TestResult<graph::Mission<'catalog>> {
    let index = u64::from(witness.tree.record_index());
    let guid = witness.tree.guid().to_string();
    let mut matches = graph::contexts(catalog, witness.family)?
        .into_iter()
        .filter(|mission| {
            mission
                .definition
                .pointer("/recordIndex/value")
                .and_then(Value::as_u64)
                == Some(index)
        });
    let mission = matches.next().ok_or("physical AutoSpawns owner context")?;
    assert!(
        matches.next().is_none() && graph::guid(mission.definition)? == guid,
        "the original mission owner has one configuration with its exact physical identity"
    );
    Ok(mission)
}
