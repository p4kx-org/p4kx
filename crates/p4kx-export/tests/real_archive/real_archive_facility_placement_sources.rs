// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::io::Read as _;

use p4kx_extraction::cryxml::{
    SourceDocument, SourceElement, SourceElementPosition, SourceReadOptions,
};
use p4kx_extraction::model::{SocEntityFile, Transform};
use p4kx_extraction::{Archive, Entry, EntryOrigin, Guid, PathIndex};

use super::Result;
use super::inventory::Facility;

const CHILD_DEPTH_SAMPLE: usize = 5;
const CONTAINER_SAMPLE: usize = 30;

#[derive(Clone)]
pub(super) struct EntryEvidence {
    pub(super) origin: EntryOrigin,
    pub(super) ordinal: usize,
}

#[derive(Clone)]
pub(super) struct Witness {
    pub(super) container: EntryEvidence,
    pub(super) descriptor: EntryEvidence,
    pub(super) descriptor_index: Option<u32>,
    pub(super) stream: EntryEvidence,
    pub(super) stream_name: String,
    pub(super) chunk: usize,
    pub(super) entity: usize,
    pub(super) class: Guid,
    pub(super) configuration: Facility,
    pub(super) local: Transform,
    pub(super) nested: Option<Transform>,
}

struct Walker<'a> {
    archive: &'a Archive,
    root_index: &'a PathIndex<'a>,
    facilities: &'a HashMap<Guid, Facility>,
    opened: usize,
    nested_opened: usize,
}

struct Room<'a> {
    archive: Archive,
    evidence: EntryEvidence,
    entry: &'a Entry,
}

pub(super) fn find_witness(
    archive: &Archive,
    index: &PathIndex<'_>,
    path: &str,
    facilities: &HashMap<Guid, Facility>,
    need_nested: bool,
) -> Result<(Option<Witness>, usize)> {
    let path = path.trim_start_matches(['/', '\\']);
    let data = format!("Data\\{path}");
    let Some(entry) = pick(index, [data.as_str(), path])? else {
        return Ok((None, 0));
    };
    let mut walker = Walker {
        archive,
        root_index: index,
        facilities,
        opened: 0,
        nested_opened: 0,
    };
    let witness = walker.open(archive, entry, &[], Some(Transform::IDENTITY), need_nested)?;
    Ok((witness, walker.nested_opened))
}

impl<'archive> Walker<'archive> {
    fn open<'room>(
        &mut self,
        owner: &'room Archive,
        entry: &'room Entry,
        ancestors: &[&'room Archive],
        transform: Option<Transform>,
        need_nested: bool,
    ) -> Result<Option<Witness>> {
        if ancestors.len() >= CHILD_DEPTH_SAMPLE || self.opened == CONTAINER_SAMPLE {
            return Ok(None);
        }
        let room = Room {
            archive: owner.open_container(entry)?,
            evidence: evidence(owner, entry)?,
            entry,
        };
        self.opened += 1;
        self.nested_opened += usize::from(!ancestors.is_empty());
        self.inspect(&room, ancestors, transform, need_nested)
    }

    fn inspect(
        &mut self,
        room: &Room<'_>,
        ancestors: &[&Archive],
        transform: Option<Transform>,
        need_nested: bool,
    ) -> Result<Option<Witness>> {
        let index = PathIndex::new(&room.archive);
        let Some(entry) = descriptor(&index, room.entry)? else {
            return Ok(None);
        };
        let bytes = read(&room.archive, entry)?;
        let document = SourceDocument::parse(&bytes, SourceReadOptions::default())?;
        let descriptor_root = document.root();
        if document.is_lossy()
            || !descriptor_root
                .name()
                .eq_ignore_ascii_case("ObjectContainer")
        {
            return Ok(None);
        }
        if need_nested != ancestors.is_empty()
            && let Some(witness) = self.streams(
                room,
                entry,
                descriptor_root,
                &index,
                transform.filter(|_| !ancestors.is_empty()),
            )?
        {
            return Ok(Some(witness));
        }
        if need_nested {
            return self.children(room, descriptor_root, ancestors, transform);
        }
        Ok(None)
    }

    fn streams(
        &self,
        room: &Room<'_>,
        descriptor: &Entry,
        descriptor_root: SourceElement<'_>,
        index: &PathIndex<'_>,
        nested: Option<Transform>,
    ) -> Result<Option<Witness>> {
        for stream in streams(descriptor_root) {
            let Some(name) = attribute(stream, "name") else {
                continue;
            };
            let Some(entry) = pick(index, [name])? else {
                continue;
            };
            if let Some(witness) = self.stream(room, descriptor, stream, entry, name, nested)? {
                return Ok(Some(witness));
            }
        }
        Ok(None)
    }

    fn children(
        &mut self,
        room: &Room<'_>,
        descriptor_root: SourceElement<'_>,
        ancestors: &[&Archive],
        transform: Option<Transform>,
    ) -> Result<Option<Witness>> {
        let Some(parent) = transform else {
            return Ok(None);
        };
        let mut next = ancestors.to_vec();
        next.push(&room.archive);
        let lists = descriptor_root
            .elements()
            .filter(|node| node.name().eq_ignore_ascii_case("ChildObjectContainers"));
        for child in lists
            .flat_map(SourceElement::elements)
            .filter(|node| node.name().eq_ignore_ascii_case("Child"))
        {
            let Some(path) =
                attribute(child, "name").and_then(p4kx_assembly::ChildContainerPath::new)
            else {
                continue;
            };
            let Some(local) = child_transform(child).and_then(|local| parent.then(&local).finite())
            else {
                continue;
            };
            if let Some((owner, entry)) = self.child_target(&next, &path)?
                && let Some(witness) = self.open(owner, entry, &next, Some(local), true)?
            {
                return Ok(Some(witness));
            }
        }
        Ok(None)
    }

    fn child_target<'a>(
        &self,
        ancestors: &[&'a Archive],
        path: &p4kx_assembly::ChildContainerPath<'_>,
    ) -> Result<Option<(&'a Archive, &'a Entry)>>
    where
        'archive: 'a,
    {
        for &owner in ancestors.iter().rev() {
            if let Some(entry) = pick(&PathIndex::new(owner), [path.relative()])? {
                return Ok(Some((owner, entry)));
            }
        }
        Ok(pick(self.root_index, path.root_candidates())?.map(|entry| (self.archive, entry)))
    }

    fn stream(
        &self,
        room: &Room<'_>,
        descriptor: &Entry,
        declaration: SourceElement<'_>,
        stream: &Entry,
        name: &str,
        nested: Option<Transform>,
    ) -> Result<Option<Witness>> {
        let bytes = read(&room.archive, stream)?;
        let soc = SocEntityFile::parse(&bytes)?;
        for chunk in soc.entity_chunks() {
            let chunk = chunk?;
            if chunk.document().is_lossy() {
                continue;
            }
            for entity in chunk.entities() {
                let Some(class) = attribute(entity.element(), "EntityClassGUID")
                    .and_then(|value| value.trim().parse::<Guid>().ok())
                else {
                    continue;
                };
                let Some(configuration) = self.facilities.get(&class) else {
                    continue;
                };
                let Some(local) = entity.local_transform() else {
                    continue;
                };
                let descriptor_index = match declaration.position() {
                    SourceElementPosition::CryXml(index) => Some(index),
                    _ => None,
                };
                return Ok(Some(Witness {
                    container: room.evidence.clone(),
                    descriptor: evidence(&room.archive, descriptor)?,
                    descriptor_index,
                    stream: evidence(&room.archive, stream)?,
                    stream_name: name.to_owned(),
                    chunk: chunk.chunk_index(),
                    entity: entity.ordinal(),
                    class,
                    configuration: configuration.clone(),
                    local,
                    nested: nested.and_then(|parent| parent.then(&local).finite()),
                }));
            }
        }
        Ok(None)
    }
}

fn descriptor<'archive>(
    index: &PathIndex<'archive>,
    container: &Entry,
) -> Result<Option<&'archive Entry>> {
    let filename = container
        .path()
        .rsplit(['/', '\\'])
        .next()
        .ok_or("room archive name")?;
    let (stem, _) = filename.rsplit_once('.').ok_or("room archive suffix")?;
    let leaf = format!("{stem}.xml");
    let subfolder = format!("{stem}\\{stem}.xml");
    pick(index, [leaf, subfolder])
}

fn evidence(archive: &Archive, entry: &Entry) -> Result<EntryEvidence> {
    Ok(EntryEvidence {
        origin: archive
            .entry_origin(entry)
            .ok_or("original physical Entry")?,
        ordinal: entry.index(),
    })
}

fn pick<'archive>(
    index: &PathIndex<'archive>,
    paths: impl IntoIterator<Item = impl AsRef<str>>,
) -> Result<Option<&'archive Entry>> {
    for path in paths {
        let mut matches = index
            .matching_entries_with(path.as_ref(), |_| Ok::<(), std::convert::Infallible>(()))?;
        if let Some(entry) = matches.next().transpose()? {
            if matches.next().transpose()?.is_some() {
                return Err("ambiguous physical archive path".into());
            }
            return Ok(Some(entry));
        }
    }
    Ok(None)
}

fn read(archive: &Archive, entry: &Entry) -> Result<Vec<u8>> {
    let mut buffer = Vec::new();
    archive.open_entry(entry)?.read_to_end(&mut buffer)?;
    Ok(buffer)
}

fn attribute<'a>(node: SourceElement<'a>, name: &str) -> Option<&'a str> {
    let mut values = node
        .attributes()
        .filter(|value| value.namespace().is_none() && value.name().eq_ignore_ascii_case(name));
    let first = values.next()?.value();
    values.next().is_none().then_some(first)
}

fn streams(descriptor_root: SourceElement<'_>) -> Vec<SourceElement<'_>> {
    let mut pending: Vec<_> = descriptor_root
        .elements()
        .filter(|node| node.name().eq_ignore_ascii_case("StreamingOCs"))
        .flat_map(SourceElement::elements)
        .collect();
    pending.reverse();
    let mut result = Vec::new();
    while let Some(node) = pending.pop() {
        if !node.name().eq_ignore_ascii_case("OC") {
            continue;
        }
        result.push(node);
        let start = pending.len();
        pending.extend(
            node.elements()
                .filter(|node| node.name().eq_ignore_ascii_case("StreamingOCs"))
                .flat_map(SourceElement::elements),
        );
        if let Some(next) = pending.get_mut(start..) {
            next.reverse();
        }
    }
    result
}

fn child_transform(node: SourceElement<'_>) -> Option<Transform> {
    let position = vector::<3>(attribute(node, "pos"), [0.0; 3])?;
    let rotation = vector::<4>(attribute(node, "rot"), [1.0, 0.0, 0.0, 0.0])?;
    Transform::from_quaternion(rotation, position)
}

fn vector<const N: usize>(raw: Option<&str>, fallback: [f64; N]) -> Option<[f64; N]> {
    let Some(raw) = raw else {
        return Some(fallback);
    };
    let mut values = raw.split(',');
    let mut parts = [0.0; N];
    for component in &mut parts {
        *component = values.next()?.trim().parse().ok()?;
    }
    values.next().is_none().then_some(parts)
}
