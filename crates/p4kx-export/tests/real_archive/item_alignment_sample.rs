// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::UNIX_EPOCH;

use p4kx_export::{
    CatalogOptions, CatalogPublicationMode, Filter, Kind, build_catalog_from, is_kind_leftover,
};
use p4kx_extraction::{
    Archive, ArchiveWatch, DataForge, DataForgeEntry, RecordTree, Reference, StructValue,
    TreeBuilder, Value as Original,
};
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde_json::Value;

use super::{
    ATTACHABLE, CONTAINER, TestResult, at, check_items, check_ports, original_components, position,
    rows, structures,
};
use crate::real_archive_auto_spawns::{original::Locations, provenance::Origins};

const DEFAULT_LOADOUT: &str = "SEntityComponentDefaultLoadoutParams";

struct Sample<'source> {
    owner: RecordTree<'source>,
    child: RecordTree<'source>,
    component: usize,
    port: usize,
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn item_port_alignment_matches_original_physical_declarations() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, Some(real_archive_key()?))
        .map_err(|_| "open configured Archive")?;
    let watch = ArchiveWatch::new(&archive);
    let entry = DataForgeEntry::read(&archive).map_err(|_| "read original DataForge")?;
    let forge = entry.open().map_err(|_| "open original DataForge")?;
    let sample = select(&forge, &watch)?;
    let owner_catalog = build(&archive, &entry, &sample.owner)?;
    let origins = Origins::read(&owner_catalog, entry.origin())?;
    let count = check_ports(&forge, &sample.owner, &owner_catalog, &origins)?;
    check_join(&owner_catalog, &origins, &sample)?;
    let child_catalog = build(&archive, &entry, &sample.child)?;
    let origins = Origins::read(&child_catalog, entry.origin())?;
    check_items(&forge, &sample.child, &child_catalog, &origins)?;
    watch
        .check()
        .map_err(|_| "Archive changed during alignment trace")?;
    crate::real_archive_verification::print_line(&format!(
        "item-port alignment: {count} original port slots, an original default attachment and its independent item definition verified"
    ));
    Ok(())
}

fn select<'source>(
    forge: &'source DataForge<'source>,
    watch: &ArchiveWatch,
) -> TestResult<Sample<'source>> {
    let probe = forge
        .pointer_probe("Components", CONTAINER)
        .ok_or("original port component declaration")?;
    let mut trees = TreeBuilder::new();
    for record in forge.records() {
        watch
            .poll()
            .map_err(|_| "Archive changed during alignment selection")?;
        let path = record.record_path()?;
        let kind = [Kind::Items, Kind::Ships]
            .into_iter()
            .find(|kind| kind.rule().is_some_and(|rule| rule.matches_path(path)));
        let Some(kind) = kind else { continue };
        if record.type_name()? != "EntityClassDefinition"
            || is_kind_leftover(kind, path, record.name()?)
            || !trees.holds(&probe, &record)?
        {
            continue;
        }
        let owner = trees.build(&record)?;
        if original_components(&owner).any(|(_, slot)| slot.type_name() == DEFAULT_LOADOUT) {
            continue;
        }
        if let Some((child, component, port)) = sample_default(forge, &mut trees, &owner)? {
            return Ok(Sample {
                owner,
                child,
                component,
                port,
            });
        }
    }
    Err("no original projected port with an independently stored default attachment found".into())
}

fn sample_default<'source>(
    forge: &'source DataForge<'source>,
    trees: &mut TreeBuilder<'source>,
    owner: &RecordTree<'source>,
) -> TestResult<Option<(RecordTree<'source>, usize, usize)>> {
    let filter = Filter::new(owner.record_path());
    let item = Kind::Items.rule().ok_or("selected Item rule")?;
    let item_filter = Filter::new(crate::real_archive_item_components::FILTER);
    for (component, container) in
        original_components(owner).filter(|(_, slot)| slot.type_name() == CONTAINER)
    {
        for (port, source) in structures(container, "Ports") {
            let Some((_, link)) = authored_default(source) else {
                continue;
            };
            if link.location().is_some() || filter.matches(link.record_path()) {
                continue;
            }
            let target = forge
                .record(link.record_index())
                .ok_or("physical default item target")?;
            if !crate::real_archive_item_components::selected_item(&target, &item, &item_filter)? {
                continue;
            }
            let child = trees.build(&target)?;
            if has_attachment(&child) {
                return Ok(Some((child, component, port)));
            }
        }
    }
    Ok(None)
}

fn has_attachment(tree: &RecordTree<'_>) -> bool {
    original_components(tree)
        .find(|(_, slot)| slot.type_name() == ATTACHABLE)
        .and_then(|(_, slot)| slot.field("AttachDef"))
        .is_some_and(|slot| {
            matches!(slot, Original::Struct(source)
                if source.type_name() == "SItemDefinition"
                    && matches!(source.field("ignoredAttachAxis"), Some(Original::Enum(_)))
                    && matches!(source.field("attachCheckCenterOfMass"), Some(Original::Bool(_))))
        })
}

fn authored_default<'read, 'source>(
    port: &'read StructValue<'source>,
) -> Option<(&'read Original<'source>, &'read Reference<'source>)> {
    if !matches!(port.field("attachAxesToAlign"), Some(Original::Enum(_))) {
        return None;
    }
    let Original::Struct(default) = port.field("defaultItem")? else {
        return None;
    };
    let original @ Original::Reference(link) = default.field("entityClass")? else {
        return None;
    };
    Some((original, link))
}

fn build(archive: &Archive, entry: &DataForgeEntry, tree: &RecordTree<'_>) -> TestResult<Value> {
    let filter = Filter::new(tree.record_path());
    let options = CatalogOptions {
        filter: Some(&filter),
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..CatalogOptions::new(UNIX_EPOCH)
    };
    let built = build_catalog_from(archive, entry, &options)
        .map_err(|_| "build source-selected alignment catalog")?;
    let catalog = serde_json::from_slice(built.json())?;
    crate::real_archive_shop_inventory_v3::verify_build(archive, &catalog)?;
    Ok(catalog)
}

fn check_join(catalog: &Value, origins: &Origins<'_>, sample: &Sample<'_>) -> TestResult {
    let owner = sample.owner.record_index();
    let guid = sample.owner.guid().to_string();
    let loadout = rows(catalog, "/loadouts")?
        .iter()
        .find(|entry| entry.get("entityGuid").and_then(Value::as_str) == Some(guid.as_str()))
        .ok_or("source-selected owner Loadout")?;
    let mut matching = rows(loadout, "/ports")?.iter().filter(|port| {
        let Some(link) = port.pointer("/sourceConstraints/portDefinition") else {
            return false;
        };
        position(link, "/ownerRecordIndex/value").ok() == usize::try_from(owner).ok()
            && position(link, "/componentOrdinal/value").ok() == Some(sample.component)
            && position(link, "/portOrdinal/value").ok() == Some(sample.port)
    });
    let port = matching
        .next()
        .ok_or("Loadout does not link the original port slot")?;
    if matching.next().is_some()
        || at(port, "/sourceConstraints/portDefinition/state")? != "projected"
        || at(port, "/source")? != "port_default"
        || !at(port, "/unresolved")?.is_null()
    {
        return Err("original default port selection differs".into());
    }
    let attached = at(port, "/sourceConstraints/attachedItem")?;
    check_attached(attached, origins, sample)
}

fn check_attached(attached: &Value, origins: &Origins<'_>, sample: &Sample<'_>) -> TestResult {
    let child = sample.child.record_index();
    let guid = sample.child.guid().to_string();
    let container = original_components(&sample.owner)
        .find(|(slot, _)| *slot == sample.component)
        .ok_or("original port container")?
        .1;
    let port = structures(container, "Ports")
        .find(|(slot, _)| *slot == sample.port)
        .ok_or("original port slot")?
        .1;
    let (raw, _) = authored_default(port).ok_or("original default item reference")?;
    let locations = Locations::read(sample.owner.root());
    let ordinal = locations.ordinal(raw)?;
    let target_guid = at(attached, "/targetGuid")?;
    let target_index = at(attached, "/targetRecordIndex")?;
    if at(target_guid, "/value")?.as_str() != Some(guid.as_str())
        || position(target_index, "/value")? != usize::try_from(child)?
        || at(attached, "/definition/state")? != "notProjected"
    {
        return Err("default reference or filtered item definition differs".into());
    }
    let owner = sample.owner.record_index();
    origins.reference_field(target_guid, owner, ordinal, child, None)?;
    origins.reference_field(target_index, owner, ordinal, child, Some("recordIndex"))
}
