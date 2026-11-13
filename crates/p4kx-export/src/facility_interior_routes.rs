// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use p4kx_assembly::ChildContainerPath;
use p4kx_extraction::{Archive, CancelToken, Entry, PathIndex, PathKeys, SourceUse, SourceValue};

use super::types::InteriorFindingKind;
use crate::catalog_read::original::{
    AuthoredEntryPath, EntryCapture, EntryRead, EntryReadError, EntryReceipt, EntryXml,
    OriginalDatum, PhysicalEntryRef,
};
use crate::starmap_xml::Node;
use crate::{CatalogError, ExportAccountingError};

pub(super) struct OpenedRoom {
    pub(super) archive: Archive,
    pub(super) container: OriginalDatum<PhysicalEntryRef>,
    keys: PathKeys,
    owner: Option<usize>,
    entry_index: usize,
}

#[derive(Clone, Copy)]
pub(super) struct ContainerOwner<'archive> {
    archive: &'archive Archive,
    entry: &'archive Entry,
}

pub(super) fn open_room(
    archive: &Archive,
    index: &PathIndex<'_>,
    capture: &EntryCapture<'_>,
    name: &str,
    cancel: Option<&CancelToken>,
) -> Result<Result<OpenedRoom, InteriorFindingKind>, CatalogError> {
    let path = name.trim().trim_start_matches(['/', '\\']);
    let data = format!("Data\\{path}");
    let selected = select(index, [data.as_str(), path], cancel)?;
    open_selected(archive, selected, capture, None, &BTreeSet::new())
}

pub(super) fn open_child<'room>(
    archive: &Archive,
    index: &PathIndex<'_>,
    capture: &EntryCapture<'_>,
    ancestors: impl DoubleEndedIterator<Item = &'room OpenedRoom> + ExactSizeIterator,
    active: &BTreeSet<PhysicalEntryRef>,
    name: &str,
    cancel: Option<&CancelToken>,
) -> Result<Result<OpenedRoom, InteriorFindingKind>, CatalogError> {
    let Some(path) = ChildContainerPath::new(name) else {
        return Ok(Err(InteriorFindingKind::InvalidChildPath));
    };
    for (owner, room) in ancestors.enumerate().rev() {
        check_cancel(cancel)?;
        let index = room.index();
        let selection = select(&index, [path.relative()], cancel)?;
        if !matches!(selection, Selection::Missing) {
            return open_selected(&room.archive, selection, capture, Some(owner), active);
        }
    }
    let selection = select(index, path.root_candidates(), cancel)?;
    open_selected(archive, selection, capture, None, active)
}

fn open_selected(
    archive: &Archive,
    selected: Selection<'_>,
    capture: &EntryCapture<'_>,
    owner: Option<usize>,
    active: &BTreeSet<PhysicalEntryRef>,
) -> Result<Result<OpenedRoom, InteriorFindingKind>, CatalogError> {
    let entry = match selected {
        Selection::One(entry) => entry,
        Selection::Missing => return Ok(Err(InteriorFindingKind::MissingContainer)),
        Selection::Ambiguous => return Ok(Err(InteriorFindingKind::AmbiguousContainer)),
    };
    let receipt = EntryReceipt::outer(capture, archive, entry)?;
    let container = receipt.physical_reference(capture)?;
    if active.contains(physical_ref(&container)?) {
        return Ok(Err(InteriorFindingKind::ChildCycle));
    }
    receipt.attempted_payload()?;
    let Ok(opened) = archive.open_container(entry) else {
        return Ok(Err(InteriorFindingKind::UnreadableContainer));
    };
    Ok(Ok(OpenedRoom {
        keys: PathKeys::new(&opened),
        archive: opened,
        container,
        owner,
        entry_index: entry.index(),
    }))
}

pub(super) fn physical_ref(
    datum: &OriginalDatum<PhysicalEntryRef>,
) -> Result<&PhysicalEntryRef, CatalogError> {
    match datum.value() {
        SourceValue::Present(entry) => Ok(entry),
        _ => Err(ExportAccountingError::ConflictingSourceDatum.into()),
    }
}

impl OpenedRoom {
    pub(super) fn index(&self) -> PathIndex<'_> {
        self.keys.index(&self.archive)
    }

    pub(super) fn owner<'room>(
        &self,
        archive: &'room Archive,
        mut ancestors: impl Iterator<Item = &'room Self>,
    ) -> Result<ContainerOwner<'room>, CatalogError> {
        let owner = match self.owner {
            None => archive,
            Some(layer) => {
                &ancestors
                    .nth(layer)
                    .ok_or(ExportAccountingError::ConflictingSourceDatum)?
                    .archive
            }
        };
        let entry = owner
            .entries()
            .get(self.entry_index)
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        Ok(ContainerOwner {
            archive: owner,
            entry,
        })
    }

    pub(super) fn descriptor(
        &self,
        owner: ContainerOwner<'_>,
        index: &PathIndex<'_>,
        capture: &EntryCapture<'_>,
        cancel: Option<&CancelToken>,
    ) -> Result<Result<EntryRead, InteriorFindingKind>, CatalogError> {
        let leaf = owner
            .entry
            .path()
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or_default();
        let stem = leaf.rsplit_once('.').map_or(leaf, |(stem, _)| stem);
        let direct = format!("{stem}.xml");
        let nested = format!("{stem}\\{stem}.xml");
        let entry = match select(index, [direct, nested], cancel)? {
            Selection::One(entry) => entry,
            Selection::Missing => return Ok(Err(InteriorFindingKind::MissingDescriptor)),
            Selection::Ambiguous => return Ok(Err(InteriorFindingKind::AmbiguousDescriptor)),
        };
        Ok(self
            .read(owner, capture, entry, cancel)?
            .map_err(|_| InteriorFindingKind::UnreadableDescriptor))
    }

    pub(super) fn stream(
        &self,
        owner: ContainerOwner<'_>,
        index: &PathIndex<'_>,
        capture: &EntryCapture<'_>,
        name: &str,
        cancel: Option<&CancelToken>,
    ) -> Result<Result<EntryRead, InteriorFindingKind>, CatalogError> {
        let entry = match select(index, [name], cancel)? {
            Selection::One(entry) => entry,
            Selection::Missing => return Ok(Err(InteriorFindingKind::MissingStream)),
            Selection::Ambiguous => return Ok(Err(InteriorFindingKind::AmbiguousStream)),
        };
        Ok(self
            .read(owner, capture, entry, cancel)?
            .map_err(|_| InteriorFindingKind::UnreadableStream))
    }

    fn read(
        &self,
        owner: ContainerOwner<'_>,
        capture: &EntryCapture<'_>,
        entry: &Entry,
        cancel: Option<&CancelToken>,
    ) -> Result<Result<EntryRead, EntryReadError>, CatalogError> {
        let receipt =
            EntryReceipt::inner(capture, owner.archive, owner.entry, &self.archive, entry)?;
        EntryRead::read_all(receipt, &self.archive, entry, cancel)
    }
}

#[derive(Clone, Copy)]
enum Selection<'archive> {
    One(&'archive Entry),
    Missing,
    Ambiguous,
}

fn select<'archive>(
    index: &PathIndex<'archive>,
    paths: impl IntoIterator<Item = impl AsRef<str>>,
    cancel: Option<&CancelToken>,
) -> Result<Selection<'archive>, CatalogError> {
    for path in paths {
        let mut candidates = index
            .matching_entries_with(path.as_ref(), |_| check_cancel(cancel))
            .map_err(path_error)?;
        let Some(first) = candidates.next().transpose().map_err(path_error)? else {
            continue;
        };
        return Ok(
            if candidates.next().transpose().map_err(path_error)?.is_some() {
                Selection::Ambiguous
            } else {
                Selection::One(first)
            },
        );
    }
    Ok(Selection::Missing)
}

fn path_error(error: p4kx_extraction::PathIndexBuildError<CatalogError>) -> CatalogError {
    match error {
        p4kx_extraction::PathIndexBuildError::Interrupted(error) => error,
        _ => ExportAccountingError::ConflictingSourceDatum.into(),
    }
}

pub(super) fn check_cancel(cancel: Option<&CancelToken>) -> Result<(), CatalogError> {
    if cancel.is_some_and(CancelToken::is_cancelled) {
        return Err(CatalogError::Cancelled);
    }
    Ok(())
}

pub(super) struct StreamDeclaration {
    pub(super) name: AuthoredEntryPath,
    pub(super) node_index: u32,
    pub(super) selection: SourceUse,
}

pub(super) fn declared_streams(
    xml: &EntryXml<'_, '_, '_>,
    cancel: Option<&CancelToken>,
) -> Result<(Vec<StreamDeclaration>, Vec<InteriorFindingKind>), CatalogError> {
    let mut declarations = Vec::new();
    let mut findings = Vec::new();
    for section in xml.root().children() {
        if !section.name().eq_ignore_ascii_case("StreamingOCs") {
            continue;
        }
        let mut pending: Vec<_> = section.children().collect();
        pending.reverse();
        while let Some(node) = pending.pop() {
            check_cancel(cancel)?;
            if !node.name().eq_ignore_ascii_case("OC") {
                continue;
            }
            match stream_declaration(xml, node)? {
                Ok(Some(declaration)) => declarations.push(declaration),
                Ok(None) => {}
                Err(finding) => findings.push(finding),
            }
            let start = pending.len();
            for nested in node.children() {
                if nested.name().eq_ignore_ascii_case("StreamingOCs") {
                    pending.extend(nested.children());
                }
            }
            pending
                .get_mut(start..)
                .ok_or(ExportAccountingError::ConflictingSourceDatum)?
                .reverse();
        }
    }
    Ok((declarations, findings))
}

fn stream_declaration<'node>(
    xml: &EntryXml<'_, '_, '_>,
    node: impl Node<'node>,
) -> Result<Result<Option<StreamDeclaration>, InteriorFindingKind>, CatalogError> {
    let mut names = node
        .attributes()
        .enumerate()
        .filter(|(_, (name, _))| name.eq_ignore_ascii_case("name"));
    let Some((ordinal, (_, value))) = names.next() else {
        return Ok(Ok(None));
    };
    if names.next().is_some() {
        return Ok(Err(InteriorFindingKind::AmbiguousStreamName));
    }
    if value.is_empty() {
        return Ok(Ok(None));
    }
    let name = xml
        .attribute_at(node, ordinal)?
        .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
    let source = xml.node_source(node)?;
    Ok(Ok(Some(StreamDeclaration {
        name,
        node_index: source.index(),
        selection: source.selected()?,
    })))
}
