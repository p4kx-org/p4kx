// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::btree_map::Entry as EntrySlot;
use std::collections::{BTreeMap, BTreeSet};

use p4kx_assembly::{EntityRig, RigPlacement};
use p4kx_extraction::cryxml::{SourceDocument, SourceElement, SourceReadOptions};
use p4kx_extraction::{
    Archive, ArchiveWatch, DataForge, Entry, Struct, StructValue, Value as DataValue,
    archive_animation_path,
};
use p4kx_media::{Identity, MediaRecord};
use serde_json::Value;

use super::adb_rows::Graph;
use super::loadout::{Site, folded};
use super::sources::{Document, Family};
use super::{TestResult, require};

pub(super) const BINDING: &str = "animation-definition-entity-database-binding";

const LOCATORS: [(&str, &str); 2] = [
    ("SAnimatedCharacterComponentParams", "animDatabase3P"),
    ("SAnimationControllerParams", "AnimationDatabase"),
];

pub(super) struct Binding {
    record_index: u32,
    component_index: usize,
    field: &'static str,
    authored: String,
    archive_path: Option<String>,
    port_path: Option<String>,
    resolution: Resolution,
    pub(super) sites: BTreeSet<Site>,
}

enum Resolution {
    Invalid,
    Missing,
    Ambiguous(Vec<usize>),
    Resolved(usize),
}

struct Locator<'source> {
    component: usize,
    field: &'static str,
    authored: &'source str,
}

pub(super) fn bindings<'source>(
    archive: &Archive,
    forge: &'source DataForge<'source>,
    rigs: &[EntityRig],
    forge_entry: usize,
) -> TestResult<Vec<Binding>> {
    let mut owners = BTreeMap::<u32, Vec<Locator<'source>>>::new();
    let mut result = Vec::new();
    for rig in rigs {
        let port = match &rig.placement {
            RigPlacement::Root => None,
            RigPlacement::Attachment { port_path } => Some(port_path.as_str()),
            _ => continue,
        };
        let record_index = rig.record_index.ok_or("original Entity Record index")?;
        let locators: &[Locator<'source>] = match owners.entry(record_index) {
            EntrySlot::Occupied(entry) => entry.into_mut(),
            EntrySlot::Vacant(entry) => entry.insert(locators(forge, record_index)?),
        };
        for locator in locators {
            let archive_path = adb_path(locator.authored.trim());
            let resolution = resolve(archive, archive_path.as_deref())?;
            result.push(Binding {
                record_index,
                component_index: locator.component,
                field: locator.field,
                authored: locator.authored.to_owned(),
                archive_path,
                port_path: port.map(str::to_owned),
                resolution,
                sites: BTreeSet::from([Site {
                    entry: forge_entry,
                    namespace: "dataforge-record",
                    indices: vec![u64::from(record_index)],
                    field: format!("Components[{}].{}", locator.component, locator.field),
                }]),
            });
        }
    }
    Ok(result)
}

fn locators<'source>(
    forge: &'source DataForge<'source>,
    record_index: u32,
) -> TestResult<Vec<Locator<'source>>> {
    let record = forge.record(record_index).ok_or("original Entity Record")?;
    let tree = record.tree()?;
    let components = match tree.root().field("Components") {
        Some(DataValue::Array(components)) => components,
        None => return Ok(Vec::new()),
        _ => return Err("unreadable original Entity Components".into()),
    };
    let mut result = Vec::new();
    for (component, value) in components.iter().enumerate() {
        let value = match value {
            DataValue::Struct(value) => value,
            DataValue::Null => continue,
            _ => return Err("unreadable original Entity component slot".into()),
        };
        for (owner, field) in LOCATORS {
            if !inherits(forge, value, owner)? {
                continue;
            }
            match value.field(field) {
                Some(DataValue::String(authored)) => result.push(Locator {
                    component,
                    field,
                    authored,
                }),
                Some(DataValue::Null) | None => {}
                _ => return Err("unreadable original Entity database locator".into()),
            }
        }
    }
    Ok(result)
}

fn inherits(forge: &DataForge<'_>, component: &StructValue<'_>, owner: &str) -> TestResult<bool> {
    let original = forge
        .struct_at(component.struct_index())
        .ok_or("original component type")?;
    for current in std::iter::successors(Some(original), Struct::parent) {
        if current.name()? == owner {
            return Ok(true);
        }
    }
    Ok(false)
}

fn adb_path(path: &str) -> Option<String> {
    let (_, extension) = path.rsplit_once('.')?;
    if extension.eq_ignore_ascii_case("adb") {
        archive_animation_path(path)
    } else {
        None
    }
}

fn resolve(archive: &Archive, path: Option<&str>) -> TestResult<Resolution> {
    let Some(path) = path else {
        return Ok(Resolution::Invalid);
    };
    let matches = archive
        .entries()
        .iter()
        .filter(|entry| folded(entry.path()).eq(folded(path)))
        .map(Entry::index)
        .collect::<Vec<_>>();
    Ok(match matches.as_slice() {
        [] => Resolution::Missing,
        [index] => {
            require(
                archive.raw_path(*index).is_none(),
                "readable original ADB path",
            )?;
            Resolution::Resolved(*index)
        }
        _ => Resolution::Ambiguous(matches),
    })
}

impl Binding {
    pub(super) fn verify(&self, row: &MediaRecord, ordinal: usize) -> TestResult {
        require(
            matches!(row.source.identity, Identity::Root),
            "root Entity binding identity",
        )?;
        require(
            row.subresource.indices == [ordinal as u64],
            "original locator occurrence",
        )?;
        let payload = row.data.get("payload").ok_or("database binding payload")?;
        require(
            payload.get("owner_record_index").and_then(Value::as_u64)
                == Some(u64::from(self.record_index))
                && payload.get("component_index").and_then(Value::as_u64)
                    == Some(self.component_index as u64)
                && payload.get("field").and_then(Value::as_str) == Some(self.field)
                && payload.get("authored_path").and_then(Value::as_str)
                    == Some(self.authored.as_str())
                && payload.get("archive_path").and_then(Value::as_str)
                    == self.archive_path.as_deref()
                && payload.get("port_path").and_then(Value::as_str) == self.port_path.as_deref(),
            "database binding matches original Record, component, selector and placement",
        )?;
        self.resolution.verify(
            payload
                .get("reference")
                .ok_or("database binding reference")?,
        )
    }
}

impl Resolution {
    const fn target(&self) -> Option<usize> {
        match self {
            Self::Resolved(index) => Some(*index),
            _ => None,
        }
    }

    fn verify(&self, reference: &Value) -> TestResult {
        let resolution = reference
            .get("resolution")
            .ok_or("database target resolution")?;
        let status = match self {
            Self::Invalid => "unavailable",
            Self::Missing => "missing",
            Self::Ambiguous(_) => "ambiguous",
            Self::Resolved(_) => "resolved",
        };
        require(
            resolution.get("status").and_then(Value::as_str) == Some(status),
            "original database target status",
        )?;
        match self {
            Self::Invalid => require(
                resolution.pointer("/reason/type").and_then(Value::as_str)
                    == Some("invalid-identifier"),
                "invalid authored target diagnosis",
            ),
            Self::Resolved(index) => require(
                resolution
                    .pointer("/target/source/entry/entry_index")
                    .and_then(Value::as_u64)
                    == Some(*index as u64),
                "exact physical database target",
            ),
            Self::Ambiguous(indices) => {
                let candidates = resolution
                    .get("candidates")
                    .and_then(Value::as_array)
                    .ok_or("physical database candidates")?;
                let observed = candidates.iter().map(|candidate| {
                    candidate
                        .pointer("/target/source/entry/entry_index")
                        .and_then(Value::as_u64)
                });
                require(
                    observed.eq(indices.iter().map(|index| Some(*index as u64))),
                    "original ordered database candidates",
                )
            }
            Self::Missing => Ok(()),
        }
    }
}

pub(super) fn read<'archive>(
    archive: &'archive Archive,
    watch: &ArchiveWatch,
    bindings: &[Binding],
    allowance: &mut u64,
    documents: &mut BTreeMap<usize, Document<'archive>>,
) -> TestResult<Graph> {
    let mut graph = Graph::default();
    let mut pending = bindings
        .iter()
        .filter_map(|binding| binding.resolution.target())
        .collect::<Vec<_>>();
    while let Some(index) = pending.pop() {
        if documents.contains_key(&index) {
            continue;
        }
        let entry = archive
            .entries()
            .get(index)
            .ok_or("original physical database")?;
        let document = Document::read(archive, entry, watch, allowance, Family::Adb)?;
        let syntax = SourceDocument::parse(&document.bytes, SourceReadOptions::default())?;
        let root = syntax.root();
        if exact(root, "AnimDB") {
            graph.document(index);
            for file in original_targets(root) {
                if let Some(target) = resolve(archive, adb_path(file).as_deref())?.target() {
                    graph.branch(index, target);
                    pending.push(target);
                }
            }
        }
        documents.insert(index, document);
    }
    graph.prepare();
    Ok(graph)
}

fn original_targets(root: SourceElement<'_>) -> impl Iterator<Item = &str> {
    root.elements()
        .filter(|list| exact(*list, "SubADBs"))
        .flat_map(SourceElement::elements)
        .filter(|branch| exact(*branch, "SubADB"))
        .filter_map(|branch| {
            let mut files = branch.attributes().filter(|attribute| {
                attribute.namespace().is_none() && attribute.name().eq_ignore_ascii_case("File")
            });
            let file = files.next()?;
            files.next().is_none().then(|| file.value())
        })
}

fn exact(element: SourceElement<'_>, name: &str) -> bool {
    element.namespace().is_none() && element.name() == name
}
