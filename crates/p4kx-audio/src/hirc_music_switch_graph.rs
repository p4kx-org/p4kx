// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;

use crate::BankError;
use crate::hirc_music_switch::{self, BankMusicSwitchNode};
use crate::hirc_music_switch_traversal::{BankMusicSwitchTarget, Traversal};

const NODE_WORDS: u64 = 4;
const ARGUMENT_WORDS: u64 = 2;

#[derive(Clone, Copy)]
pub(crate) enum RouteOutput {
    Ids,
    WithAncestors,
}

#[derive(Default)]
pub(crate) struct GraphInspection {
    pub(crate) target_prefix: u32,
    pub(crate) targets_compact: bool,
    pub(crate) error: Option<BankError>,
}

pub(crate) fn inspect<E: From<BankError>>(
    nodes: u32,
    depth: u32,
    output: RouteOutput,
    mut read: impl FnMut(u32) -> Result<BankMusicSwitchNode, E>,
    mut target: impl FnMut(&BankMusicSwitchTarget),
) -> Result<GraphInspection, E> {
    let structure = reachable(nodes, depth, &mut read)?;
    let mut result = GraphInspection {
        error: structure.error,
        ..GraphInspection::default()
    };
    let mut remaining = u64::from(structure.nodes) * NODE_WORDS
        + u64::from(depth.min(structure.nodes)) * ARGUMENT_WORDS;
    let mut traversal = Traversal::new(nodes, depth);
    loop {
        let next = traversal.next_target(|index| {
            remaining = remaining.checked_sub(1).ok_or(ProbeError::Compact)?;
            read(index).map_err(ProbeError::Read)
        });
        match next {
            Ok(Some(value)) => {
                if matches!(output, RouteOutput::WithAncestors) {
                    let Some(left) = remaining.checked_sub(u64::from(value.level)) else {
                        return Ok(result.compact());
                    };
                    remaining = left;
                }
                result.target_prefix += 1;
                target(&value);
            }
            Ok(None) => return Ok(result),
            Err(ProbeError::Format(error)) => {
                result.error = Some(error);
                return Ok(result);
            }
            Err(ProbeError::Compact) => return Ok(result.compact()),
            Err(ProbeError::Read(error)) => return Err(error),
        }
    }
}

impl GraphInspection {
    const fn compact(mut self) -> Self {
        self.target_prefix = 0;
        self.targets_compact = true;
        self
    }
}

enum ProbeError<E> {
    Format(BankError),
    Read(E),
    Compact,
}

impl<E> From<BankError> for ProbeError<E> {
    fn from(error: BankError) -> Self {
        Self::Format(error)
    }
}

#[derive(Default)]
struct Reachable {
    nodes: u32,
    error: Option<BankError>,
}

fn reachable<E>(
    nodes: u32,
    depth: u32,
    read: &mut impl FnMut(u32) -> Result<BankMusicSwitchNode, E>,
) -> Result<Reachable, E> {
    if nodes == 0 {
        return Ok(Reachable::default());
    }

    let mut remaining = Unvisited::new(nodes);
    let mut branches = BTreeMap::new();
    let mut reachable = Reachable {
        nodes: 1,
        error: None,
    };
    let mut frontier = std::iter::once(0..1).collect::<Vec<_>>();
    let mut next = Vec::new();
    // The first visit is at minimum depth and covers every possible use as a branch.
    for _ in 0..depth {
        for index in frontier.iter().cloned().flatten() {
            match hirc_music_switch::child_range(read(index)?.link.value, nodes) {
                Ok(children) if !children.is_empty() => {
                    while let Some(new_nodes) = remaining.take(&children) {
                        reachable.nodes += new_nodes.end - new_nodes.start;
                        next.push(new_nodes);
                    }
                    branches.insert(index, children);
                }
                Ok(_) => {}
                Err(error) => {
                    reachable.error.get_or_insert(error);
                }
            }
        }
        if next.is_empty() {
            break;
        }
        frontier.clear();
        std::mem::swap(&mut frontier, &mut next);
    }
    if reachable.error.is_none() && cyclic(branches) {
        reachable.error = Some(hirc_music_switch::cyclic());
    }
    Ok(reachable)
}

struct Branch {
    index: u32,
    children: Range<u32>,
}

fn cyclic(mut branches: BTreeMap<u32, Range<u32>>) -> bool {
    let Some(children) = branches.remove(&0) else {
        return false;
    };
    let mut active = BTreeSet::from([0]);
    let mut parents = vec![Branch { index: 0, children }];
    while let Some(parent) = parents.last() {
        if active.range(parent.children.clone()).next().is_some() {
            return true;
        }
        let Some((&index, _)) = branches.range(parent.children.clone()).next() else {
            active.remove(&parent.index);
            parents.pop();
            continue;
        };
        if let Some(children) = branches.remove(&index) {
            active.insert(index);
            parents.push(Branch { index, children });
        }
    }
    false
}

struct Unvisited {
    intervals: BTreeMap<u32, u32>,
}

impl Unvisited {
    fn new(nodes: u32) -> Self {
        let mut intervals = BTreeMap::new();
        if nodes > 1 {
            intervals.insert(1, nodes);
        }
        Self { intervals }
    }

    fn take(&mut self, bounds: &Range<u32>) -> Option<Range<u32>> {
        let (&start, &end) = self
            .intervals
            .range(..=bounds.start)
            .next_back()
            .filter(|&(_, &end)| end > bounds.start)
            .or_else(|| self.intervals.range(bounds.clone()).next())?;
        self.intervals.remove(&start);

        let claimed = start.max(bounds.start)..end.min(bounds.end);
        if start < claimed.start {
            self.intervals.insert(start, claimed.start);
        }
        if claimed.end < end {
            self.intervals.insert(claimed.end, end);
        }
        Some(claimed)
    }
}
