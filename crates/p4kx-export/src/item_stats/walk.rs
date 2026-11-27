// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::slice::Iter;

use p4kx_extraction::{Field, StructValue, Value};

/// Returns the first struct, the root first and then depth first in stored order, that holds a
/// field called `name`.
pub(super) fn owner_of<'tree, 'source>(
    root: &'tree StructValue<'source>,
    name: &str,
) -> Option<&'tree StructValue<'source>> {
    find(root, |structure| structure.field(name).is_some())
}

/// Returns the first struct, the root first and then depth first in stored order, that holds a
/// field called any of `names`.
pub(super) fn owner_of_any<'tree, 'source>(
    root: &'tree StructValue<'source>,
    names: &[&str],
) -> Option<&'tree StructValue<'source>> {
    find(root, |structure| {
        names.iter().any(|name| structure.field(name).is_some())
    })
}

/// Returns the first struct of the type `type_name`, the root first and then depth first in
/// stored order.
pub(super) fn struct_named<'tree, 'source>(
    root: &'tree StructValue<'source>,
    type_name: &str,
) -> Option<&'tree StructValue<'source>> {
    find(root, |structure| structure.type_name() == type_name)
}

fn find<'tree, 'source>(
    root: &'tree StructValue<'source>,
    mut accepts: impl FnMut(&StructValue<'source>) -> bool,
) -> Option<&'tree StructValue<'source>> {
    if accepts(root) {
        return Some(root);
    }
    let mut parents = Vec::new();
    let mut children = Children::Fields(root.fields().iter());
    loop {
        match children.next() {
            Some(Value::Struct(structure)) => {
                if accepts(structure) {
                    return Some(structure);
                }
                parents.push(children);
                children = Children::Fields(structure.fields().iter());
            }
            Some(Value::Array(items)) => {
                parents.push(children);
                children = Children::Values(items.iter());
            }
            Some(_) => {}
            None => children = parents.pop()?,
        }
    }
}

enum Children<'tree, 'source> {
    Fields(Iter<'tree, Field<'source>>),
    Values(Iter<'tree, Value<'source>>),
}

impl<'tree, 'source> Iterator for Children<'tree, 'source> {
    type Item = &'tree Value<'source>;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Fields(fields) => fields.next().map(Field::value),
            Self::Values(values) => values.next(),
        }
    }
}

#[cfg(test)]
#[path = "walk_tests.rs"]
mod tests;
