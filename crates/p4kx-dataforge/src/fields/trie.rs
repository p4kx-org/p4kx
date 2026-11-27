// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use crate::fields::metric_coverage::Gaps;
use crate::fields::number::{Distinct, Extremes};
use crate::fields::{FieldKind, FieldStatisticsError, MAX_FIELD_PATHS};

/// The node of the empty path, whose children are the fields of the Records.
pub(crate) const ROOT: u32 = 0;

/// What is counted at one path.
#[derive(Debug)]
pub(crate) struct Node<'f> {
    pub(crate) parent: u32,
    pub(crate) segment: &'f str,
    pub(crate) occurrences: u64,
    pub(crate) empty: u64,
    pub(crate) records: u64,
    pub(crate) last_record: u32,
    pub(crate) kinds: Vec<(FieldKind, u64)>,
    pub(crate) extremes: Option<Box<Extremes>>,
    pub(crate) distinct: Option<Box<Distinct>>,
    pub(super) gaps: Gaps,
}

impl<'f> Node<'f> {
    const fn new(parent: u32, segment: &'f str) -> Self {
        Self {
            parent,
            segment,
            occurrences: 0,
            empty: 0,
            records: 0,
            last_record: u32::MAX,
            kinds: Vec::new(),
            extremes: None,
            distinct: None,
            gaps: Gaps::new(),
        }
    }

    /// Counts one value of `kind` in the Record `record`.
    pub(crate) fn count(&mut self, record: u32, kind: FieldKind, empty: bool) {
        self.occurrences += 1;
        self.empty += u64::from(empty);
        if self.last_record != record {
            self.last_record = record;
            self.records += 1;
        }
        match self.kinds.iter_mut().find(|(known, _)| *known == kind) {
            Some((_, count)) => *count += 1,
            None => self.kinds.push((kind, 1)),
        }
    }

    fn absorb(&mut self, other: Self) {
        self.gaps.merge(other.gaps);
        self.occurrences += other.occurrences;
        self.empty += other.empty;
        self.records += other.records;
        for (kind, count) in other.kinds {
            match self.kinds.iter_mut().find(|(known, _)| *known == kind) {
                Some((_, known)) => *known += count,
                None => self.kinds.push((kind, count)),
            }
        }
        if let Some(extremes) = other.extremes {
            match &mut self.extremes {
                Some(own) => own.merge(*extremes),
                none => *none = Some(extremes),
            }
        }
        if let Some(distinct) = other.distinct {
            match &mut self.distinct {
                Some(own) => own.merge(*distinct),
                none => *none = Some(distinct),
            }
        }
    }
}

/// The paths seen so far as a tree of segments, with the counts of each.
///
/// A parent is always created before its children, so a node's index is greater than its
/// parent's.
#[derive(Debug)]
pub(crate) struct Trie<'f> {
    pub(crate) nodes: Vec<Node<'f>>,
    children: HashMap<(u32, &'f str), u32>,
}

impl<'f> Trie<'f> {
    pub(crate) fn new() -> Self {
        Self {
            nodes: vec![Node::new(ROOT, "")],
            children: HashMap::new(),
        }
    }

    /// Returns the child of `parent` named `segment`, creating it.
    pub(crate) fn child(
        &mut self,
        parent: u32,
        segment: &'f str,
    ) -> Result<u32, FieldStatisticsError> {
        if let Some(&known) = self.children.get(&(parent, segment)) {
            return Ok(known);
        }
        if self.nodes.len() >= MAX_FIELD_PATHS {
            return Err(FieldStatisticsError::TooManyPaths {
                limit: MAX_FIELD_PATHS,
            });
        }
        let id =
            u32::try_from(self.nodes.len()).map_err(|_| FieldStatisticsError::TooManyPaths {
                limit: MAX_FIELD_PATHS,
            })?;
        self.nodes.push(Node::new(parent, segment));
        self.children.insert((parent, segment), id);
        Ok(id)
    }

    /// Adds the counts of `other` to this trie, the smaller one into the larger.
    pub(crate) fn merge(self, other: Self) -> Result<Self, FieldStatisticsError> {
        let (mut base, small) = if other.nodes.len() > self.nodes.len() {
            (other, self)
        } else {
            (self, other)
        };
        if let Some(root) = base.nodes.first_mut()
            && let Some(other_root) = small.nodes.first()
        {
            root.gaps.merge(other_root.gaps);
        }
        let mut ids = vec![ROOT; small.nodes.len()];
        for (index, node) in small.nodes.into_iter().enumerate().skip(1) {
            let parent = ids.get(node.parent as usize).copied().unwrap_or(ROOT);
            let target = base.child(parent, node.segment)?;
            if let Some(slot) = ids.get_mut(index) {
                *slot = target;
            }
            if let Some(into) = base.nodes.get_mut(target as usize) {
                into.absorb(node);
            }
        }
        Ok(base)
    }
}
