// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{HashSet, VecDeque};

use p4kx_extraction::Entry;
use p4kx_extraction::cryxml::{SourceDocument, SourceElement, SourceReadOptions};

use super::chrparams_selectors::{self, Target};
use super::definition_scope::{allocation, excluded, lookup_work, read_verified, selected_target};
use super::graph_source::GraphRole;
use super::links;
use super::payload::{DefinitionFamily, LinkKind};
use super::wildcard_match::{Family, Rule};
use crate::{DirectoryLookup, MediaError, MediaWriter};

pub(super) struct DependencyScope<'scope> {
    definitions: &'scope mut HashSet<usize>,
    references: &'scope mut HashSet<usize>,
    pending: VecDeque<Pending>,
}

struct Pending {
    position: usize,
    family: DependencyFamily,
}

enum DependencyFamily {
    Parameters,
    CombinedSpace,
}

impl<'scope> DependencyScope<'scope> {
    pub(super) const fn new(
        definitions: &'scope mut HashSet<usize>,
        references: &'scope mut HashSet<usize>,
    ) -> Self {
        Self {
            definitions,
            references,
            pending: VecDeque::new(),
        }
    }

    pub(super) fn observe_parameters(
        &mut self,
        position: usize,
        bytes: &[u8],
        lookup: &DirectoryLookup<'_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        if !remember(self.definitions, position, writer)? {
            return Ok(());
        }
        let limits = writer.limits();
        let checkpoint = writer.checkpoint;
        chrparams_selectors::visit_targets(
            bytes,
            |count| lookup.budget.metadata(0, count, limits, checkpoint),
            |target| self.follow(target, lookup, writer),
        )
    }

    pub(super) fn expand(
        &mut self,
        lookup: &DirectoryLookup<'_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        while let Some(pending) = self.pending.pop_front() {
            self.read(&pending, lookup, writer)?;
        }
        writer.checkpoint()
    }

    fn follow(
        &mut self,
        target: Target<'_>,
        lookup: &DirectoryLookup<'_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        match target {
            Target::Definition(path) => self.path(&path, lookup, writer),
            Target::Reference(path) => {
                if let Some(entry) = selected_target(&path, lookup, writer)? {
                    remember(self.references, entry.index(), writer)?;
                }
                Ok(())
            }
            Target::Wildcard(rule) => self.wildcard(&rule, lookup, writer),
        }
    }

    fn wildcard(
        &mut self,
        rule: &Rule<'_>,
        lookup: &DirectoryLookup<'_>,
        writer: &MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let work = lookup_work(lookup, writer)?;
        lookup
            .index
            .visit_animation_wildcard(rule, &work, |entry, _| {
                if excluded(lookup, entry) {
                    return Ok(());
                }
                match rule.family() {
                    Family::Caf => {
                        remember(self.references, entry.index(), writer)?;
                        Ok(())
                    }
                    Family::BlendSpace | Family::CombinedSpace => self.admit(entry, writer),
                }
            })?;
        Ok(())
    }

    fn path(
        &mut self,
        path: &str,
        lookup: &DirectoryLookup<'_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        if let Some(entry) = selected_target(path, lookup, writer)? {
            self.admit(entry, writer)?;
        }
        Ok(())
    }

    fn admit(&mut self, entry: &Entry, writer: &MediaWriter<'_>) -> Result<(), MediaError> {
        let position = entry.index();
        if !remember(self.definitions, position, writer)? {
            return Ok(());
        }
        let family = match DefinitionFamily::of(entry.path()) {
            Some(DefinitionFamily::Chrparams) => DependencyFamily::Parameters,
            Some(DefinitionFamily::Comb) => DependencyFamily::CombinedSpace,
            _ => return Ok(()),
        };
        self.pending
            .try_reserve(1)
            .map_err(|cause| allocation(writer, cause))?;
        self.pending.push_back(Pending { position, family });
        Ok(())
    }

    fn read(
        &mut self,
        pending: &Pending,
        lookup: &DirectoryLookup<'_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let archive = lookup.index.archive;
        let entry = archive.entries().get(pending.position).ok_or_else(|| {
            MediaError::Contract("selected animation dependency lost its original Entry".into())
        })?;
        charge(lookup, writer, 1)?;
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
        match pending.family {
            DependencyFamily::Parameters => {
                let limits = writer.limits();
                let checkpoint = writer.checkpoint;
                chrparams_selectors::visit_document_targets(
                    &document,
                    |count| lookup.budget.metadata(0, count, limits, checkpoint),
                    |target| self.follow(target, lookup, writer),
                )
            }
            DependencyFamily::CombinedSpace => self.combined(document.root(), lookup, writer),
        }
    }

    fn combined(
        &mut self,
        root: SourceElement<'_>,
        lookup: &DirectoryLookup<'_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let family = DefinitionFamily::Comb;
        let root_role = GraphRole::of(family, None, true, root);
        if !matches!(root_role, Some(GraphRole::CombinedSpace)) {
            return Ok(());
        }
        for items in root.elements() {
            charge(lookup, writer, 1)?;
            let items_role = GraphRole::of(family, root_role, false, items);
            if !matches!(items_role, Some(GraphRole::BlendSpaces)) {
                continue;
            }
            for item in items.elements() {
                charge(lookup, writer, 1)?;
                if matches!(
                    GraphRole::of(family, items_role, false, item),
                    Some(GraphRole::BlendSpaceInput)
                ) && let Some(authored) = combined_path(item, lookup, writer)?
                    && let Some(path) = links::target_path(LinkKind::CombinedSpace, authored)
                {
                    self.path(&path, lookup, writer)?;
                }
            }
        }
        Ok(())
    }
}

fn combined_path<'source>(
    element: SourceElement<'source>,
    lookup: &DirectoryLookup<'_>,
    writer: &MediaWriter<'_>,
) -> Result<Option<&'source str>, MediaError> {
    let mut path = None;
    for attribute in element.attributes() {
        charge(lookup, writer, 1)?;
        if attribute.namespace().is_none()
            && attribute.name().eq_ignore_ascii_case("AName")
            && path.replace(attribute.value()).is_some()
        {
            return Ok(None);
        }
    }
    Ok(path)
}

fn charge(
    lookup: &DirectoryLookup<'_>,
    writer: &MediaWriter<'_>,
    count: u64,
) -> Result<(), MediaError> {
    lookup
        .budget
        .metadata(0, count, writer.limits(), writer.checkpoint)
}

fn remember(
    positions: &mut HashSet<usize>,
    position: usize,
    writer: &MediaWriter<'_>,
) -> Result<bool, MediaError> {
    if positions.contains(&position) {
        return Ok(false);
    }
    positions
        .try_reserve(1)
        .map_err(|cause| allocation(writer, cause))?;
    Ok(positions.insert(position))
}
