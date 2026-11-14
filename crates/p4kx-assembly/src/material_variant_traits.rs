// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt;
use std::mem;

use crate::MaterialVariant;

impl Clone for MaterialVariant {
    fn clone(&self) -> Self {
        let mut current = copy_fields(self);
        let mut siblings = self.children.iter();
        let mut parents = Vec::new();
        loop {
            if let Some(child) = siblings.next() {
                parents.push((siblings, current));
                current = copy_fields(child);
                siblings = child.children.iter();
            } else if let Some((remaining, mut parent)) = parents.pop() {
                parent.children.push(current);
                current = parent;
                siblings = remaining;
            } else {
                return current;
            }
        }
    }
}

fn copy_fields(node: &MaterialVariant) -> MaterialVariant {
    MaterialVariant {
        tags: node.tags.clone(),
        material: node.material.clone(),
        children: Vec::with_capacity(node.children.len()),
    }
}

impl PartialEq for MaterialVariant {
    fn eq(&self, other: &Self) -> bool {
        let mut pending = vec![(self, other)];
        while let Some((left, right)) = pending.pop() {
            if left.tags != right.tags
                || left.material != right.material
                || left.children.len() != right.children.len()
            {
                return false;
            }
            pending.extend(left.children.iter().zip(&right.children));
        }
        true
    }
}

impl Eq for MaterialVariant {}

impl Drop for MaterialVariant {
    fn drop(&mut self) {
        let mut pending = mem::take(&mut self.children);
        while let Some(mut node) = pending.pop() {
            pending.append(&mut node.children);
        }
    }
}

impl fmt::Debug for MaterialVariant {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MaterialVariant")
            .field("nodes", &DebugNodes(self))
            .finish()
    }
}

struct DebugNodes<'nodes>(&'nodes MaterialVariant);

struct DebugNode<'node> {
    depth: usize,
    tags: &'node [String],
    material: Option<&'node str>,
    children: usize,
}

impl fmt::Debug for DebugNodes<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut pending = vec![(0, self.0)];
        let nodes = std::iter::from_fn(move || {
            let (depth, node) = pending.pop()?;
            pending.extend(node.children.iter().rev().map(|child| (depth + 1, child)));
            Some(DebugNode {
                depth,
                tags: node.tags(),
                material: node.material(),
                children: node.children().len(),
            })
        });
        formatter.debug_list().entries(nodes).finish()
    }
}

impl fmt::Debug for DebugNode<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MaterialNode")
            .field("depth", &self.depth)
            .field("tags", &self.tags)
            .field("material", &self.material)
            .field("children", &self.children)
            .finish()
    }
}
