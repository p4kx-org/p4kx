// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{HashMap, TryReserveError};

use crate::input_budget::InputBudget;
use crate::{MediaError, MediaWriter, origin_error};

pub(super) struct Work<'run, 'cancel> {
    budget: &'run InputBudget,
    writer: &'run MediaWriter<'cancel>,
}

impl<'run, 'cancel> Work<'run, 'cancel> {
    pub(super) const fn new(budget: &'run InputBudget, writer: &'run MediaWriter<'cancel>) -> Self {
        Self { budget, writer }
    }

    pub(super) fn charge(&self) -> Result<(), MediaError> {
        self.budget
            .metadata(0, 1, self.writer.limits(), self.writer.checkpoint)
    }

    pub(super) fn allocation(&self, cause: &TryReserveError) -> MediaError {
        super::sub_database_cycles::allocation(cause, self.writer)
    }
}

#[derive(Default)]
pub(super) struct Graph {
    indices: HashMap<usize, usize>,
    nodes: Vec<Node>,
    edges: Vec<Endpoints>,
}

struct Node {
    entry: usize,
    targets: Vec<usize>,
    sources: Vec<usize>,
}

pub(super) struct Endpoints {
    source: usize,
    target: usize,
}

impl Graph {
    pub(super) fn insert_edge(
        &mut self,
        source: usize,
        target: usize,
        work: &Work<'_, '_>,
    ) -> Result<(), MediaError> {
        let source = self.insert_node(source, work)?;
        let target = self.insert_node(target, work)?;
        let outgoing = &mut self
            .nodes
            .get_mut(source)
            .ok_or_else(origin_error::counter)?
            .targets;
        push(outgoing, target, work)?;
        let incoming = &mut self
            .nodes
            .get_mut(target)
            .ok_or_else(origin_error::counter)?
            .sources;
        push(incoming, source, work)?;
        push(&mut self.edges, Endpoints { source, target }, work)
    }

    pub(super) fn edges(&self) -> &[Endpoints] {
        &self.edges
    }

    fn insert_node(&mut self, entry: usize, work: &Work<'_, '_>) -> Result<usize, MediaError> {
        if let Some(index) = self.indices.get(&entry) {
            return Ok(*index);
        }
        let index = self.nodes.len();
        self.indices
            .try_reserve(1)
            .map_err(|cause| work.allocation(&cause))?;
        push(
            &mut self.nodes,
            Node {
                entry,
                targets: Vec::new(),
                sources: Vec::new(),
            },
            work,
        )?;
        self.indices.insert(entry, index);
        Ok(index)
    }

    pub(super) fn components(&self, work: &Work<'_, '_>) -> Result<Components, MediaError> {
        let order = self.finish_order(work)?;
        let mut components = Components {
            index: filled(self.nodes.len(), None, work)?,
            groups: Vec::new(),
        };
        let mut pending = Vec::new();
        for root in order.into_iter().rev() {
            work.charge()?;
            let group = components.groups.len();
            if !mark(&mut components.index, root, Some(group))? {
                continue;
            }
            let mut stats = Group {
                representative: usize::MAX,
                members: 0,
            };
            push(&mut pending, root, work)?;
            while let Some(index) = pending.pop() {
                work.charge()?;
                let node = self.nodes.get(index).ok_or_else(origin_error::counter)?;
                stats.representative = stats.representative.min(node.entry);
                stats.members = stats
                    .members
                    .checked_add(1)
                    .ok_or_else(origin_error::counter)?;
                for &source in &node.sources {
                    work.charge()?;
                    if mark(&mut components.index, source, Some(group))? {
                        push(&mut pending, source, work)?;
                    }
                }
            }
            push(&mut components.groups, stats, work)?;
        }
        Ok(components)
    }

    // Finishing order on the forward graph seeds the reverse pass.
    fn finish_order(&self, work: &Work<'_, '_>) -> Result<Vec<usize>, MediaError> {
        let mut visited = filled(self.nodes.len(), None, work)?;
        let mut order = Vec::new();
        let mut pending = Vec::new();
        for root in 0..self.nodes.len() {
            work.charge()?;
            if !mark(&mut visited, root, Some(()))? {
                continue;
            }
            push(
                &mut pending,
                Visit {
                    node: root,
                    next: 0,
                },
                work,
            )?;
            while let Some(visit) = pending.last_mut() {
                work.charge()?;
                let node = self
                    .nodes
                    .get(visit.node)
                    .ok_or_else(origin_error::counter)?;
                if let Some(&target) = node.targets.get(visit.next) {
                    visit.next += 1;
                    if mark(&mut visited, target, Some(()))? {
                        push(
                            &mut pending,
                            Visit {
                                node: target,
                                next: 0,
                            },
                            work,
                        )?;
                    }
                } else {
                    push(&mut order, visit.node, work)?;
                    pending.pop();
                }
            }
        }
        Ok(order)
    }
}

struct Visit {
    node: usize,
    next: usize,
}

pub(super) struct Components {
    index: Vec<Option<usize>>,
    groups: Vec<Group>,
}

pub(super) struct Group {
    pub(super) representative: usize,
    pub(super) members: u64,
}

impl Components {
    pub(super) const fn count(&self) -> usize {
        self.groups.len()
    }

    pub(super) fn group(&self, index: usize) -> Result<&Group, MediaError> {
        self.groups.get(index).ok_or_else(origin_error::counter)
    }

    pub(super) fn cycle(&self, edge: &Endpoints) -> Result<Option<usize>, MediaError> {
        let source = self.component(edge.source)?;
        let target = self.component(edge.target)?;
        let cycle =
            source == target && (edge.source == edge.target || self.group(source)?.members > 1);
        Ok(cycle.then_some(source))
    }

    fn component(&self, node: usize) -> Result<usize, MediaError> {
        self.index
            .get(node)
            .copied()
            .flatten()
            .ok_or_else(origin_error::counter)
    }
}

fn mark<T>(slots: &mut [Option<T>], key: usize, value: Option<T>) -> Result<bool, MediaError> {
    let slot = slots.get_mut(key).ok_or_else(origin_error::counter)?;
    if slot.is_some() {
        return Ok(false);
    }
    *slot = value;
    Ok(true)
}

fn filled<T: Clone>(length: usize, value: T, work: &Work<'_, '_>) -> Result<Vec<T>, MediaError> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(length)
        .map_err(|cause| work.allocation(&cause))?;
    values.resize(length, value);
    Ok(values)
}

fn push<T>(values: &mut Vec<T>, value: T, work: &Work<'_, '_>) -> Result<(), MediaError> {
    values
        .try_reserve(1)
        .map_err(|cause| work.allocation(&cause))?;
    values.push(value);
    Ok(())
}
