// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet};

use p4kx_assembly::{ArchiveFiles, Attachment, LoadoutResolver};
use p4kx_export::{Filter, Kind, is_kind_leftover};
use p4kx_extraction::{
    Archive, ArchiveWatch, DataForge, PointerProbe, Record, RecordTree, TreeBuilder, Value,
};

use super::TestResult;

const COMPONENTS: &str = "Components";
const CRAFTER: &str = "CrafterComponentParams";

pub(super) type Mounts = BTreeMap<u32, Vec<Vec<u32>>>;

pub(super) fn station<'source>(
    forge: &'source DataForge<'source>,
    watch: &ArchiveWatch,
    selected: Option<u32>,
) -> TestResult<RecordTree<'source>> {
    let probe = forge
        .pointer_probe(COMPONENTS, CRAFTER)
        .ok_or("no original crafter components available")?;
    let mut trees = TreeBuilder::new();
    if let Some(index) = selected {
        let record = forge
            .record(index)
            .ok_or("configured station Record is missing")?;
        if !admitted_station(&record)? || !has_component(&mut trees, &probe, &record)? {
            return Err(
                "the configured physical station does not declare a Crafter component".into(),
            );
        }
        return trees
            .build(&record)
            .map_err(|_| "read the configured original station".into());
    }
    for record in forge.records() {
        watch
            .poll()
            .map_err(|_| "Archive changed during metadata selection")?;
        if admitted_station(&record)? && has_component(&mut trees, &probe, &record)? {
            return trees
                .build(&record)
                .map_err(|_| "read the original station".into());
        }
    }
    Err("no originally authored station was found".into())
}

pub(super) fn exact_filter(
    forge: &DataForge<'_>,
    watch: &ArchiveWatch,
    filter: &Filter,
    expected: &BTreeSet<u32>,
) -> TestResult {
    let mut matched = 0;
    for record in forge.records() {
        watch
            .poll()
            .map_err(|_| "Archive changed during filter preflight")?;
        let path = record
            .record_path()
            .map_err(|_| "an unreadable owner path prevents proving the filter")?;
        if filter.matches(path) {
            if !expected.contains(&record.index()) {
                return Err("the configured filter would admit another physical Record".into());
            }
            matched += 1;
        }
    }
    if matched != expected.len() {
        return Err("the configured filter does not select all required physical Records".into());
    }
    Ok(())
}

pub(super) fn component<'tree, 'source>(
    station: &'tree RecordTree<'source>,
) -> TestResult<(u32, &'tree p4kx_extraction::StructValue<'source>)> {
    let Some(Value::Array(slots)) = station.field(COMPONENTS) else {
        return Err("the original station lacks readable Components".into());
    };
    for (index, value) in slots.iter().enumerate() {
        if let Value::Struct(component) = value
            && component.type_name() == CRAFTER
        {
            return Ok((u32::try_from(index)?, component));
        }
    }
    Err("the independently selected station has no original crafter component".into())
}

pub(super) fn resolve_mounts(
    archive: &Archive,
    forge: &DataForge<'_>,
    watch: &ArchiveWatch,
    index: u32,
) -> TestResult<Mounts> {
    let record = forge
        .record(index)
        .ok_or("the configured host Record is missing")?;
    if selected_kind(&record)? != Some(Kind::Ships) {
        return Err("the configured physical host is outside normal Ship selection".into());
    }
    let Some(probe) = forge.pointer_probe(COMPONENTS, CRAFTER) else {
        return Err("no original Crafter component instances are available".into());
    };
    let files = ArchiveFiles::new(archive);
    let mut resolver = LoadoutResolver::new(forge)
        .with_files(&files)
        .stopping_at_docked_vehicles();
    let original = resolver
        .resolve(&record)
        .map_err(|_| "resolve the originally authored host Loadout")?;
    if original.record_index() != Some(index) || !original.issues().is_empty() {
        return Err("the source-selected host has incomplete original Loadout coverage".into());
    }
    collect_mounts(forge, watch, &probe, &original)
}

fn collect_mounts<'source>(
    forge: &'source DataForge<'source>,
    watch: &ArchiveWatch,
    probe: &PointerProbe<'source>,
    original: &p4kx_assembly::EntityNode,
) -> TestResult<Mounts> {
    let mut trees = TreeBuilder::new();
    let mut mounts = BTreeMap::new();
    let mut path = Vec::new();
    let mut walk = vec![original.ports().iter().enumerate()];
    while let Some(ports) = walk.last_mut() {
        watch
            .poll()
            .map_err(|_| "Archive changed during original host verification")?;
        let Some((ordinal, port)) = ports.next() else {
            walk.pop();
            path.pop();
            continue;
        };
        path.push(u32::try_from(ordinal)?);
        let children = match port.attachment() {
            Attachment::Empty => port.nested(),
            Attachment::Entity(attachment) => {
                let target = attachment
                    .record_index()
                    .ok_or("resolved attachment lacks its original physical Record")?;
                let target_record = forge
                    .record(target)
                    .ok_or("resolved attachment Record is missing")?;
                if admitted_station(&target_record)?
                    && has_component(&mut trees, probe, &target_record)?
                {
                    mounts
                        .entry(target)
                        .or_insert_with(Vec::new)
                        .push(path.clone());
                }
                if attachment.is_expanded() {
                    attachment.ports()
                } else {
                    &[]
                }
            }
            Attachment::Unresolved(_) => {
                return Err("the source-selected host has an unresolved mount".into());
            }
        };
        if children.is_empty() {
            path.pop();
        } else {
            walk.push(children.iter().enumerate());
        }
    }
    if mounts.is_empty() {
        return Err("the exact host has no positive original crafter mounts".into());
    }
    Ok(mounts)
}

fn has_component<'source>(
    trees: &mut TreeBuilder<'source>,
    probe: &PointerProbe<'source>,
    record: &Record<'source>,
) -> TestResult<bool> {
    trees
        .holds(probe, record)
        .map_err(|_| "cannot inspect the original component slot".into())
}

fn admitted_station(record: &Record<'_>) -> TestResult<bool> {
    Ok(matches!(
        selected_kind(record)?,
        Some(
            Kind::Items | Kind::Ships | Kind::Commodities | Kind::MineableEntities | Kind::Crafters
        )
    ))
}

fn selected_kind(record: &Record<'_>) -> TestResult<Option<Kind>> {
    let path = record
        .record_path()
        .map_err(|_| "read the original owner path")?;
    let source_type = record
        .type_name()
        .map_err(|_| "read the original owner type")?;
    if source_type != p4kx_assembly::ENTITY_TYPE {
        return Ok(None);
    }
    let kind = Kind::ALL.into_iter().find(|kind| {
        kind.rule()
            .is_some_and(|rule| rule.type_name == source_type && rule.matches_path(path))
    });
    let name = record.name().map_err(|_| "read the original owner label")?;
    Ok(kind.filter(|kind| !is_kind_leftover(*kind, path, name)))
}
