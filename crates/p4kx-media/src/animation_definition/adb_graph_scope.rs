// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{HashSet, VecDeque};

use p4kx_extraction::Entry;
use p4kx_extraction::cryxml::animation_database::AnimationDatabase;
use p4kx_extraction::cryxml::{SourceDocument, SourceReadOptions};

use super::definition_scope::{allocation, read_verified, selected_target};
use super::links;
use super::payload::LinkKind;
use crate::{DirectoryLookup, MediaError, MediaWriter};

/// Admits the physical animation databases reachable from the Entity's authored roots.
pub(super) fn expand(
    roots: impl IntoIterator<Item = usize>,
    definitions: &mut HashSet<usize>,
    references: &mut HashSet<usize>,
    lookup: &DirectoryLookup<'_>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let mut scope = Scope {
        definitions,
        references,
        pending: VecDeque::new(),
    };
    for position in roots {
        writer.checkpoint()?;
        scope.admit(position, writer)?;
    }
    while let Some(position) = scope.pending.pop_front() {
        scope.read(position, lookup, writer)?;
    }
    Ok(())
}

struct Scope<'plan> {
    definitions: &'plan mut HashSet<usize>,
    references: &'plan mut HashSet<usize>,
    pending: VecDeque<usize>,
}

impl Scope<'_> {
    fn admit(&mut self, position: usize, writer: &MediaWriter<'_>) -> Result<(), MediaError> {
        if self.definitions.contains(&position) {
            return Ok(());
        }
        self.definitions
            .try_reserve(1)
            .map_err(|cause| allocation(writer, cause))?;
        self.pending
            .try_reserve(1)
            .map_err(|cause| allocation(writer, cause))?;
        self.definitions.insert(position);
        self.pending.push_back(position);
        Ok(())
    }

    fn read(
        &mut self,
        position: usize,
        lookup: &DirectoryLookup<'_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let archive = lookup.index.archive;
        let entry = archive.entries().get(position).ok_or_else(|| {
            MediaError::Contract("selected animation database lost its original Entry".into())
        })?;
        lookup
            .budget
            .metadata(0, 1, writer.limits(), writer.checkpoint)?;
        if entry.is_encrypted() && !archive.has_key() {
            return Ok(());
        }
        let bytes = read_verified(lookup, entry, writer)?;
        let Ok(document) = SourceDocument::parse(&bytes, SourceReadOptions::default()) else {
            return writer.checkpoint();
        };
        writer.checkpoint()?;
        if document.is_lossy() {
            return Ok(());
        }
        if let Some(database) = AnimationDatabase::from_root(document.root()) {
            self.follow(database, lookup, writer)?;
        }
        Ok(())
    }

    fn follow(
        &mut self,
        database: AnimationDatabase<'_>,
        lookup: &DirectoryLookup<'_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let root = database.root();
        for attribute in root
            .attributes()
            .filter(|attribute| attribute.namespace().is_none())
        {
            lookup
                .budget
                .metadata(0, 1, writer.limits(), writer.checkpoint)?;
            if let Some(kind) = links::mannequin_kind(root.name(), None, attribute.name())
                && let Some(position) = target(kind, attribute.value(), lookup, writer)?
                && !self.references.contains(&position)
            {
                self.references
                    .try_reserve(1)
                    .map_err(|cause| allocation(writer, cause))?;
                self.references.insert(position);
            }
        }
        for branch in database.branches() {
            lookup
                .budget
                .metadata(0, 1, writer.limits(), writer.checkpoint)?;
            if let Ok(file) = links::unique_sub_database_file(branch.element())
                && let Some(position) = target(LinkKind::SubDatabase, file.value(), lookup, writer)?
            {
                self.admit(position, writer)?;
            }
        }
        writer.checkpoint()
    }
}

fn target(
    kind: LinkKind,
    authored: &str,
    lookup: &DirectoryLookup<'_>,
    writer: &mut MediaWriter<'_>,
) -> Result<Option<usize>, MediaError> {
    let Some(path) = links::target_path(kind, authored) else {
        return Ok(None);
    };
    Ok(selected_target(&path, lookup, writer)?.map(Entry::index))
}
