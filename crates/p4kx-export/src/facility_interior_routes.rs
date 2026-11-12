// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, CancelToken, Entry, PathIndex, SourceUse};

use super::types::InteriorFindingKind;
use crate::catalog_read::original::{
    AuthoredEntryPath, EntryCapture, EntryRead, EntryReadError, EntryReceipt, EntryXml,
    OriginalDatum, PhysicalEntryRef,
};
use crate::starmap_xml::Node;
use crate::{CatalogError, ExportAccountingError};

pub(super) struct OpenedRoom<'archive> {
    pub(super) archive: Archive,
    entry: &'archive Entry,
    pub(super) container: OriginalDatum<PhysicalEntryRef>,
}

pub(super) fn open_room<'archive>(
    archive: &'archive Archive,
    index: &PathIndex<'archive>,
    capture: &EntryCapture<'_>,
    name: &str,
    cancel: Option<&CancelToken>,
) -> Result<Result<OpenedRoom<'archive>, InteriorFindingKind>, CatalogError> {
    let path = name.trim().trim_start_matches(['/', '\\']);
    let data = format!("Data\\{path}");
    let entry = match select(index, &[&data, path], cancel)? {
        Selection::One(entry) => entry,
        Selection::Missing => return Ok(Err(InteriorFindingKind::MissingContainer)),
        Selection::Ambiguous => return Ok(Err(InteriorFindingKind::AmbiguousContainer)),
    };
    let receipt = EntryReceipt::outer(capture, archive, entry)?;
    receipt.attempted_payload()?;
    let Ok(opened) = archive.open_container(entry) else {
        return Ok(Err(InteriorFindingKind::UnreadableContainer));
    };
    Ok(Ok(OpenedRoom {
        archive: opened,
        entry,
        container: receipt.physical_reference(capture)?,
    }))
}

impl OpenedRoom<'_> {
    pub(super) fn descriptor(
        &self,
        archive: &Archive,
        index: &PathIndex<'_>,
        capture: &EntryCapture<'_>,
        cancel: Option<&CancelToken>,
    ) -> Result<Result<EntryRead, InteriorFindingKind>, CatalogError> {
        let leaf = self
            .entry
            .path()
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or_default();
        let stem = leaf.rsplit_once('.').map_or(leaf, |(stem, _)| stem);
        let direct = format!("{stem}.xml");
        let nested = format!("{stem}\\{stem}.xml");
        let entry = match select(index, &[&direct, &nested], cancel)? {
            Selection::One(entry) => entry,
            Selection::Missing => return Ok(Err(InteriorFindingKind::MissingDescriptor)),
            Selection::Ambiguous => return Ok(Err(InteriorFindingKind::AmbiguousDescriptor)),
        };
        Ok(self
            .read(archive, capture, entry, cancel)?
            .map_err(|_| InteriorFindingKind::UnreadableDescriptor))
    }

    pub(super) fn stream(
        &self,
        archive: &Archive,
        index: &PathIndex<'_>,
        capture: &EntryCapture<'_>,
        name: &str,
        cancel: Option<&CancelToken>,
    ) -> Result<Result<EntryRead, InteriorFindingKind>, CatalogError> {
        let entry = match select(index, &[name], cancel)? {
            Selection::One(entry) => entry,
            Selection::Missing => return Ok(Err(InteriorFindingKind::MissingStream)),
            Selection::Ambiguous => return Ok(Err(InteriorFindingKind::AmbiguousStream)),
        };
        Ok(self
            .read(archive, capture, entry, cancel)?
            .map_err(|_| InteriorFindingKind::UnreadableStream))
    }

    fn read(
        &self,
        archive: &Archive,
        capture: &EntryCapture<'_>,
        entry: &Entry,
        cancel: Option<&CancelToken>,
    ) -> Result<Result<EntryRead, EntryReadError>, CatalogError> {
        let receipt = EntryReceipt::inner(capture, archive, self.entry, &self.archive, entry)?;
        EntryRead::read_all(receipt, &self.archive, entry, cancel)
    }
}

enum Selection<'archive> {
    One(&'archive Entry),
    Missing,
    Ambiguous,
}

fn select<'archive>(
    index: &PathIndex<'archive>,
    paths: &[&str],
    cancel: Option<&CancelToken>,
) -> Result<Selection<'archive>, CatalogError> {
    for path in paths {
        let mut candidates = index
            .matching_entries_with(path, |_| check_cancel(cancel))
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
        if section.name().eq_ignore_ascii_case("ChildObjectContainers") {
            if section
                .children()
                .any(|child| child.name().eq_ignore_ascii_case("Child"))
            {
                findings.push(InteriorFindingKind::UnexaminedChild);
            }
            continue;
        }
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
