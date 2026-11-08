// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use rayon::prelude::*;

use crate::entry_path;
use crate::filter::{Filter, FilterError};
use crate::texture_job;
use crate::texture_job::{Member, Role};

/// The most exclusion Filters [`Exclusion::parse`] accepts.
pub const MAX_EXCLUSIONS: usize = 64;

/// Filters that leave Entries out of a selection: an Entry is left out when any of them matches
/// its path, with the rules of [`Filter`].
///
/// A split texture is left out as a whole when its header is. The parts of a texture whose header
/// is kept are left out one by one; they are listed in [`LeftOut::parts_of_kept_textures`]
/// because a Conversion of the texture reads them from the Archive all the same.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Exclusion {
    filters: Vec<Filter>,
}

/// The outcome of [`Exclusion::leave_out`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeftOut<T> {
    /// The items that remain, in their order.
    pub kept: Vec<T>,
    /// The number of items left out.
    pub removed: usize,
    /// The left-out items that are part or alpha tail Entries of textures; a texture whose header
    /// remains is still built from them.
    pub parts_of_kept_textures: Vec<T>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Verdict {
    Keep,
    Drop,
    DropPart,
}

impl Exclusion {
    /// Creates the exclusion for the Filter texts `texts`.
    ///
    /// # Errors
    ///
    /// Returns [`FilterError::TooManyExclusions`] for more than [`MAX_EXCLUSIONS`] texts,
    /// [`FilterError::EmptyExclusion`] for an empty text, and the errors of [`Filter::parse`].
    pub fn parse<I>(texts: I) -> Result<Self, FilterError>
    where
        I: IntoIterator,
        I::Item: AsRef<str>,
    {
        let mut filters = Vec::new();
        for text in texts {
            if filters.len() == MAX_EXCLUSIONS {
                let count = filters.len().saturating_add(1);
                return Err(FilterError::TooManyExclusions { count });
            }
            let text = text.as_ref();
            if text.trim().is_empty() {
                return Err(FilterError::EmptyExclusion);
            }
            filters.push(Filter::parse(text)?);
        }
        Ok(Self { filters })
    }

    /// Returns whether there is no Filter, so nothing is left out.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.filters.is_empty()
    }

    /// Returns the Filters in the order they were given.
    #[must_use]
    pub fn filters(&self) -> &[Filter] {
        &self.filters
    }

    /// Returns whether any Filter matches the Entry path `path`.
    #[must_use]
    pub fn matches(&self, path: &str) -> bool {
        self.filters.iter().any(|filter| filter.matches(path))
    }

    /// Returns whether the Entry or its valid texture header is excluded.
    #[must_use]
    pub fn leaves_out(&self, path: &str) -> bool {
        !matches!(self.verdict(path), Verdict::Keep)
    }

    /// Returns whether `path` belongs to the unexcluded texture header or alpha header.
    #[must_use]
    pub fn is_part_of_kept_texture(&self, header: &str, path: &str) -> bool {
        if self.matches(header) {
            return false;
        }
        match (texture_job::role(header), texture_job::role(path)) {
            (Some(Role::Header), Some(Role::Member(owner, _))) => {
                entry_path::same_entry_path(header, owner)
            }
            (
                Some(Role::Member(owner, Member::AlphaHeader)),
                Some(Role::Member(target, Member::AlphaPart(_))),
            ) => !self.matches(owner) && entry_path::same_entry_path(owner, target),
            _ => false,
        }
    }

    /// Leaves the items out of `items` whose Entry path this exclusion matches, keeping the order
    /// of the rest; `path` returns the Entry path of an item, an Entry or a handle to one.
    #[must_use]
    pub fn leave_out<'p, T>(&self, items: Vec<T>, path: impl Fn(&T) -> &'p str + Sync) -> LeftOut<T>
    where
        T: Send + Sync,
    {
        if self.is_empty() {
            return LeftOut {
                kept: items,
                removed: 0,
                parts_of_kept_textures: Vec::new(),
            };
        }
        let verdicts: Vec<Verdict> = items
            .par_iter()
            .map(|item| self.verdict(path(item)))
            .collect();
        let total = items.len();
        let mut parts_of_kept_textures = Vec::new();
        let kept: Vec<T> = items
            .into_iter()
            .zip(verdicts)
            .filter_map(|(item, verdict)| match verdict {
                Verdict::Keep => Some(item),
                Verdict::Drop => None,
                Verdict::DropPart => {
                    parts_of_kept_textures.push(item);
                    None
                }
            })
            .collect();
        LeftOut {
            removed: total.saturating_sub(kept.len()),
            kept,
            parts_of_kept_textures,
        }
    }

    fn verdict(&self, path: &str) -> Verdict {
        match texture_job::role(path) {
            Some(Role::Member(header, _)) if self.matches(header) => Verdict::Drop,
            Some(Role::Member(..)) if self.matches(path) => Verdict::DropPart,
            Some(Role::Member(..)) => Verdict::Keep,
            _ if self.matches(path) => Verdict::Drop,
            _ => Verdict::Keep,
        }
    }
}
