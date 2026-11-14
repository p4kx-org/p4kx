// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::Cell;
use std::collections::BTreeMap;

use p4kx_extraction::FieldPath;

use crate::MediaError;
use crate::origin_capture::OriginCapture;
use crate::origin_contributions::OriginContributions;

#[derive(Debug, Default)]
struct Node {
    children: BTreeMap<String, Self>,
    contribution: Option<OriginContributions>,
    visited: Cell<bool>,
    subtree: bool,
}

/// Prefix-sharing exact addresses retaining immutable original contribution sets.
#[derive(Debug, Default)]
pub(crate) struct OriginAddresses {
    root: Node,
}

impl OriginAddresses {
    pub(crate) fn insert(
        &mut self,
        path: &FieldPath,
        contribution: OriginContributions,
    ) -> Result<(), MediaError> {
        let node = self.insert_at(path, contribution)?;
        node.subtree = false;
        Ok(())
    }

    pub(crate) fn insert_subtree(
        &mut self,
        path: &FieldPath,
        contribution: OriginContributions,
    ) -> Result<(), MediaError> {
        let node = self.insert_at(path, contribution)?;
        node.subtree = true;
        Ok(())
    }

    fn insert_at(
        &mut self,
        path: &FieldPath,
        contribution: OriginContributions,
    ) -> Result<&mut Node, MediaError> {
        let mut node = &mut self.root;
        for segment in segments(path.as_str()) {
            node = node.children.entry(segment.to_owned()).or_default();
        }
        if node.contribution.is_some() {
            return Err(MediaError::Contract(
                "duplicate prepared media origin address".into(),
            ));
        }
        node.contribution = Some(contribution);
        Ok(node)
    }

    pub(crate) fn mount(
        &mut self,
        from: Self,
        source: &FieldPath,
        destination: &FieldPath,
    ) -> Result<(), MediaError> {
        let mut original = from.root;
        for name in segments(source.as_str()) {
            if original.contribution.is_some() || original.children.len() != 1 {
                return Err(MediaError::Contract(
                    "packed origin has unexpected addresses".into(),
                ));
            }
            original = original.children.remove(name).ok_or_else(|| {
                MediaError::Contract("packed origin has no requested source subtree".into())
            })?;
        }
        let mut names = segments(destination.as_str()).peekable();
        let mut node = &mut self.root;
        while let Some(name) = names.next() {
            if names.peek().is_none() {
                return match node.children.entry(name.to_owned()) {
                    std::collections::btree_map::Entry::Vacant(entry) => {
                        entry.insert(original);
                        Ok(())
                    }
                    std::collections::btree_map::Entry::Occupied(_) => Err(MediaError::Contract(
                        "duplicate packed media origin address".into(),
                    )),
                };
            }
            node = node.children.entry(name.to_owned()).or_default();
        }
        Err(MediaError::Contract(
            "packed origin requires a nested destination".into(),
        ))
    }

    pub(crate) fn visit(&self, path: &str) -> Option<&OriginContributions> {
        let mut node = &self.root;
        let mut inherited = node.subtree.then_some(node);
        for segment in segments(path) {
            let Some(child) = node.children.get(segment) else {
                return inherited.and_then(visit_contribution);
            };
            node = child;
            if node.subtree {
                inherited = Some(node);
            }
        }
        visit_contribution(node).or_else(|| inherited.and_then(visit_contribution))
    }

    pub(crate) fn has_known_descendants(&self, prefix: &str) -> bool {
        let mut node = &self.root;
        for segment in segments(prefix) {
            let Some(child) = node.children.get(segment) else {
                return false;
            };
            node = child;
        }
        node.children.values().any(has_known)
    }
    pub(crate) fn include_known(
        &self,
        prefix: &str,
        capture: &mut OriginCapture,
    ) -> Result<(), MediaError> {
        let mut node = &self.root;
        for segment in segments(prefix) {
            let Some(child) = node.children.get(segment) else {
                return Ok(());
            };
            node = child;
        }
        include_known(node, capture)
    }
    pub(crate) fn require_consumed(&self) -> Result<(), MediaError> {
        if consumed(&self.root) {
            Ok(())
        } else {
            Err(MediaError::Contract(
                "prepared media origin has no actual payload address".into(),
            ))
        }
    }
}

fn visit_contribution(node: &Node) -> Option<&OriginContributions> {
    node.visited.set(true);
    node.contribution.as_ref()
}

fn segments(pointer: &str) -> impl Iterator<Item = &str> {
    pointer
        .strip_prefix('/')
        .into_iter()
        .flat_map(|pointer| pointer.split('/'))
}
fn consumed(node: &Node) -> bool {
    (node.contribution.is_none() || node.visited.get()) && node.children.values().all(consumed)
}
fn include_known(node: &Node, capture: &mut OriginCapture) -> Result<(), MediaError> {
    if let Some(contribution) = &node.contribution {
        capture.include_original(contribution)?;
    }
    for child in node.children.values() {
        include_known(child, capture)?;
    }
    Ok(())
}
fn has_known(node: &Node) -> bool {
    node.contribution.is_some() || node.children.values().any(has_known)
}
