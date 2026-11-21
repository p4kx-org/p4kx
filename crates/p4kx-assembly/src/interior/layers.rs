// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, Entry, PathKeys};

use super::{ChildContainerPath, InteriorWatch};
use crate::{AssemblyError, archive_member_selection};

pub(super) struct ChildMatches<'archive> {
    pub(super) layer: usize,
    pub(super) depth: usize,
    pub(super) entries: Vec<&'archive Entry>,
}

/// The Archives an Interior is read from: the ones the caller opened, outermost first, and the
/// containers nested in them that the walk opened.
pub(super) struct Layers<'a> {
    pub(super) given: &'a [&'a Archive],
    pub(super) opened: Vec<Archive>,
    pub(super) keys: Vec<PathKeys>,
}

impl<'a> Layers<'a> {
    pub(super) fn new(given: &'a [&'a Archive]) -> Self {
        Self {
            given,
            opened: Vec::new(),
            keys: given.iter().map(|archive| PathKeys::new(archive)).collect(),
        }
    }

    pub(super) fn get(&self, layer: usize) -> Option<&Archive> {
        layer
            .checked_sub(self.given.len())
            .map_or_else(|| self.given.get(layer).copied(), |at| self.opened.get(at))
    }

    pub(super) fn child_matches(
        &self,
        chain: &[usize],
        path: &ChildContainerPath<'_>,
        watch: &InteriorWatch<'_>,
    ) -> Result<Option<ChildMatches<'_>>, AssemblyError> {
        let check = || {
            if watch.is_cancelled() {
                Err(AssemblyError::Cancelled)
            } else {
                Ok(())
            }
        };
        for (depth, &layer) in chain.iter().enumerate().rev() {
            let Some((archive, keys)) = self.get(layer).zip(self.keys.get(layer)) else {
                continue;
            };
            let index = keys.index(archive);
            let matches = if layer == 0 {
                archive_member_selection::first_matching(&index, path.root_candidates(), check)
            } else {
                archive_member_selection::first_matching(&index, [path.relative()], check)
            }
            .map_err(|error| match error.into_cause() {
                Ok(cancelled) => cancelled,
                Err(error) => AssemblyError::Provider(Box::new(error)),
            })?;
            if !matches.is_empty() {
                return Ok(Some(ChildMatches {
                    layer,
                    depth,
                    entries: matches,
                }));
            }
        }
        Ok(None)
    }

    pub(super) const fn len(&self) -> usize {
        self.given.len() + self.opened.len()
    }

    pub(super) fn push(&mut self, archive: Archive) -> usize {
        self.keys.push(PathKeys::new(&archive));
        self.opened.push(archive);
        self.len() - 1
    }
}
