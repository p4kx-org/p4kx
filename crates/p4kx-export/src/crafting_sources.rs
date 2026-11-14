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
    let mut pending = vec![Frame::new(tree.root(), Scope::default())];
    while let Some(frame) = pending.last_mut() {
        let Some(value) = frame.next() else {
            pending.pop();
            continue;
        };
        let node = frame.node;
        let scope = frame.scope;
        match value {
            Value::Reference(reference)
                if reference.location().is_none()
                    && reference.guid() != tree.guid()
                    && pools.contains(&reference.guid()) =>
            {
                out.push(RewardSource {
                    record: tree.guid(),
                    record_path: tree.record_path(),
                    contract_id: scope.id,
                    contract_name: scope.name,
                    pool: reference.guid(),
                    chance: number(Some(node), "chance"),
                });
            }
            Value::Struct(child) => pending.push(Frame::new(child, scope)),
            _ => {}
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
    type Item = &'tree Value<'source>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(child) = self
                .children
                .find(|value| matches!(value, Value::Struct(_)))
            {
                return Some(child);
            }
            let value = self.fields.next()?.value();
            if let Value::Array(children) = value {
                self.children = children.iter();
            } else {
                return Some(value);
            }
        }
    }
}

#[cfg(test)]
#[path = "crafting_sources_tests.rs"]
mod tests;
