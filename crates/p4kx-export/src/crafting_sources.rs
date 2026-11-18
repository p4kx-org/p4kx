// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;
use std::slice::Iter;

use p4kx_extraction::{Field, Guid, RecordTree, StructValue, Value};

use crate::crafting_types::RewardSource;
use crate::record_fields::{guid_value, number, text};

/// The nearest enclosing identity of a struct: the `id` and the `debugName` of the nearest struct
/// with an `id`, or, above any such struct, the nearest `debugName`.
#[derive(Clone, Copy, Default)]
struct Scope<'f> {
    id: Option<Guid>,
    name: Option<&'f str>,
}

impl<'f> Scope<'f> {
    /// Returns the scope inside `node`: its own `id` and `debugName` when it has an `id`, this
    /// scope when this one already has an `id`, otherwise the nearest `debugName`.
    fn within(self, node: &StructValue<'f>) -> Self {
        let name = text(Some(node), "debugName");
        match guid_value(Some(node), "id") {
            Some(id) => Self { id: Some(id), name },
            None if self.id.is_some() => self,
            None => Self {
                id: None,
                name: name.or(self.name),
            },
        }
    }
}

/// Appends selected direct pool References from owned structs in source order.
pub(crate) fn collect<'source>(
    tree: &RecordTree<'source>,
    pools: &HashSet<Guid>,
    out: &mut Vec<RewardSource<'source>>,
) {
    out.extend(walk(tree).filter_map(|step| step.reward(tree, pools)));
}

pub(crate) struct WalkStep<'tree, 'source> {
    pub(crate) node: &'tree StructValue<'source>,
    pub(crate) value: &'tree Value<'source>,
    scope: Scope<'source>,
    edge: Edge,
}

#[derive(Clone, Copy)]
enum Edge {
    Field,
    ArrayElement,
}

struct VisitedValue<'tree, 'source> {
    value: &'tree Value<'source>,
    edge: Edge,
}

impl<'source> WalkStep<'_, 'source> {
    pub(crate) fn reward(
        &self,
        tree: &RecordTree<'source>,
        pools: &HashSet<Guid>,
    ) -> Option<RewardSource<'source>> {
        let Value::Reference(reference) = self.value else {
            return None;
        };
        if !matches!(self.edge, Edge::Field)
            || reference.location().is_some()
            || reference.guid() == tree.guid()
            || !pools.contains(&reference.guid())
        {
            return None;
        }
        Some(RewardSource {
            record: tree.guid(),
            record_path: tree.record_path(),
            contract_id: self.scope.id,
            contract_name: self.scope.name,
            pool: reference.guid(),
            chance: number(Some(self.node), "chance"),
        })
    }
}

pub(crate) struct Walk<'tree, 'source> {
    pending: Vec<Frame<'tree, 'source>>,
}

pub(crate) fn walk<'tree, 'source>(tree: &'tree RecordTree<'source>) -> Walk<'tree, 'source> {
    Walk {
        pending: vec![Frame::new(tree.root(), Scope::default())],
    }
}

impl<'tree, 'source> Iterator for Walk<'tree, 'source> {
    type Item = WalkStep<'tree, 'source>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let frame = self.pending.last_mut()?;
            let Some(visited) = frame.next() else {
                self.pending.pop();
                continue;
            };
            let step = WalkStep {
                node: frame.node,
                value: visited.value,
                scope: frame.scope,
                edge: visited.edge,
            };
            if let Value::Struct(child) = visited.value {
                self.pending.push(Frame::new(child, step.scope));
            }
            return Some(step);
        }
    }
}

struct Frame<'tree, 'source> {
    node: &'tree StructValue<'source>,
    scope: Scope<'source>,
    fields: Iter<'tree, Field<'source>>,
    children: Iter<'tree, Value<'source>>,
}

impl<'tree, 'source> Frame<'tree, 'source> {
    fn new(node: &'tree StructValue<'source>, parent: Scope<'source>) -> Self {
        Self {
            node,
            scope: parent.within(node),
            fields: node.fields().iter(),
            children: [].iter(),
        }
    }
}

impl<'tree, 'source> Iterator for Frame<'tree, 'source> {
    type Item = VisitedValue<'tree, 'source>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(value) = self.children.next() {
            return Some(VisitedValue {
                value,
                edge: Edge::ArrayElement,
            });
        }
        let value = self.fields.next()?.value();
        if let Value::Array(children) = value {
            self.children = children.iter();
        }
        Some(VisitedValue {
            value,
            edge: Edge::Field,
        })
    }
}

#[cfg(test)]
#[path = "crafting_sources_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "crafting_source_fixture.rs"]
pub(crate) mod fixture;
#[cfg(test)]
#[path = "crafting_source_scope_fixture.rs"]
pub(crate) mod scope_fixture;
