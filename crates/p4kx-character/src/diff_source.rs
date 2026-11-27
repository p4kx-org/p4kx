// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt::Write as _;

use crate::{ChfChange, ChfNameKind};

/// The membership selector used when a compared item is absent from one source list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ChfDiffMissing {
    /// The original positional index checked in a list.
    Index(usize),
    /// The hash and repeated-hash occurrence used to pair a list entry.
    Hash {
        /// The original hash.
        hash: u32,
        /// The occurrence associated with the present side.
        ordinal: usize,
    },
}

/// The original field or membership query on one side of a comparison.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChfDiffSite {
    pointer: String,
    missing: Option<ChfDiffMissing>,
}

impl ChfDiffSite {
    pub(crate) const fn root() -> Self {
        Self {
            pointer: String::new(),
            missing: None,
        }
    }

    /// Returns the exact source field or containing list pointer.
    #[must_use]
    pub fn pointer(&self) -> &str {
        &self.pointer
    }

    /// Returns the compared membership query when the field has no original item.
    #[must_use]
    pub const fn missing(&self) -> Option<ChfDiffMissing> {
        self.missing
    }

    pub(crate) fn field(&self, key: &str) -> Self {
        Self {
            pointer: format!("{}/{key}", self.pointer),
            missing: self.missing,
        }
    }

    pub(crate) fn member(&self, index: Option<usize>, missing: ChfDiffMissing) -> Self {
        index.map_or_else(
            || Self {
                pointer: self.pointer.clone(),
                missing: Some(missing),
            },
            |index| Self {
                pointer: format!("{}/{index}", self.pointer),
                missing: None,
            },
        )
    }
}

/// The actual raw selectors and optional resolved name emitted for one path or order segment.
#[derive(Debug, Clone)]
pub struct ChfDiffLookup {
    pub(crate) kind: Option<ChfNameKind>,
    pub(crate) hash: u32,
    pub(crate) before: Option<ChfDiffSite>,
    pub(crate) after: Option<ChfDiffSite>,
}

impl ChfDiffLookup {
    /// Returns the table kind when a name actually supplied the rendered label.
    #[must_use]
    pub const fn kind(&self) -> Option<ChfNameKind> {
        self.kind
    }

    /// Returns the original hash or numbered slot used for this segment.
    #[must_use]
    pub const fn hash(&self) -> u32 {
        self.hash
    }

    /// Returns the original selector in the first file when present.
    #[must_use]
    pub const fn before(&self) -> Option<&ChfDiffSite> {
        self.before.as_ref()
    }

    /// Returns the original selector in the second file when present.
    #[must_use]
    pub const fn after(&self) -> Option<&ChfDiffSite> {
        self.after.as_ref()
    }
}

/// A comparison change and the exact inputs used to address and display it.
#[derive(Debug)]
pub struct ChfObservedChange {
    pub(crate) change: ChfChange,
    pub(crate) before: ChfDiffSite,
    pub(crate) after: ChfDiffSite,
    pub(crate) path: Vec<ChfDiffLookup>,
    pub(crate) order: Vec<ChfDiffLookup>,
}

impl ChfObservedChange {
    /// Returns the unchanged comparison row.
    #[must_use]
    pub const fn change(&self) -> &ChfChange {
        &self.change
    }

    /// Returns the first file's original field or membership query.
    #[must_use]
    pub const fn before(&self) -> &ChfDiffSite {
        &self.before
    }

    /// Returns the second file's original field or membership query.
    #[must_use]
    pub const fn after(&self) -> &ChfDiffSite {
        &self.after
    }

    /// Returns the actual raw hash and optional name selectors of the displayed path.
    #[must_use]
    pub fn path_lookups(&self) -> &[ChfDiffLookup] {
        &self.path
    }

    /// Returns the paired label selectors rendered in an order change.
    #[must_use]
    pub fn order_lookups(&self) -> &[ChfDiffLookup] {
        &self.order
    }
}

pub(crate) struct MatchSite {
    pub(crate) before: Option<usize>,
    pub(crate) after: Option<usize>,
    pub(crate) hash: u32,
    pub(crate) ordinal: usize,
    pub(crate) hash_field: &'static str,
    pub(crate) name_kind: Option<ChfNameKind>,
}

#[derive(Clone)]
pub(crate) struct Trail {
    pub(crate) text: String,
    pub(crate) raw: String,
    capture: bool,
    before: ChfDiffSite,
    after: ChfDiffSite,
    lookups: Vec<ChfDiffLookup>,
}

impl Trail {
    pub(crate) const fn root(capture: bool) -> Self {
        Self {
            text: String::new(),
            raw: String::new(),
            capture,
            before: ChfDiffSite::root(),
            after: ChfDiffSite::root(),
            lookups: Vec::new(),
        }
    }

    pub(crate) fn key(&self, key: &str) -> Self {
        self.display_key(key).source_key(key)
    }

    pub(crate) fn display_key(&self, key: &str) -> Self {
        self.display_segment(key, key)
    }

    fn display_segment(&self, text: &str, raw: &str) -> Self {
        Self {
            text: dotted(&self.text, text),
            raw: dotted(&self.raw, raw),
            capture: self.capture,
            before: self.before.clone(),
            after: self.after.clone(),
            lookups: self.lookups.clone(),
        }
    }

    pub(crate) fn source_key(mut self, key: &str) -> Self {
        if self.capture {
            self.before = self.before.field(key);
            self.after = self.after.field(key);
        }
        self
    }

    pub(crate) fn source_indices(mut self, index: usize, before: bool, after: bool) -> Self {
        if self.capture {
            let missing = ChfDiffMissing::Index(index);
            self.before = self.before.member(before.then_some(index), missing);
            self.after = self.after.member(after.then_some(index), missing);
        }
        self
    }

    pub(crate) fn segment(&self, text: &str, raw: &str, site: &MatchSite) -> Self {
        let mut child = self.display_segment(text, raw);
        if site.ordinal != 0 {
            let _infallible = write!(child.text, "#{}", site.ordinal);
            let _infallible = write!(child.raw, "#{}", site.ordinal);
        }
        if self.capture {
            let missing = ChfDiffMissing::Hash {
                hash: site.hash,
                ordinal: site.ordinal,
            };
            child.before = self.before.member(site.before, missing);
            child.after = self.after.member(site.after, missing);
            if let Some(lookup) = self.lookup(site) {
                child.lookups.push(lookup);
            }
        }
        child
    }

    pub(crate) fn lookup(&self, site: &MatchSite) -> Option<ChfDiffLookup> {
        self.capture.then(|| {
            let selector = |side: &ChfDiffSite, index: Option<usize>| {
                index.map(|index| {
                    side.member(Some(index), ChfDiffMissing::Index(index))
                        .field(site.hash_field)
                })
            };
            ChfDiffLookup {
                kind: site.name_kind,
                hash: site.hash,
                before: selector(&self.before, site.before),
                after: selector(&self.after, site.after),
            }
        })
    }

    pub(crate) fn observe(
        &self,
        change: ChfChange,
        order: Vec<ChfDiffLookup>,
    ) -> ChfObservedChange {
        ChfObservedChange {
            change,
            before: self.before.clone(),
            after: self.after.clone(),
            path: self.lookups.clone(),
            order,
        }
    }
}

fn dotted(base: &str, part: &str) -> String {
    if base.is_empty() {
        part.to_owned()
    } else {
        format!("{base}.{part}")
    }
}
