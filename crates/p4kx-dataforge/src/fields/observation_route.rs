// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::iter;

use super::FieldStatisticsInput;

/// A source selector that admitted an enclosing branch of a statistics read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldStatisticsRouteEdge {
    pub(super) selector: FieldStatisticsInput,
    pub(super) target: FieldStatisticsRouteTarget,
}

impl FieldStatisticsRouteEdge {
    /// Borrows the original input that selected the enclosing branch.
    #[must_use]
    pub const fn selector(&self) -> &FieldStatisticsInput {
        &self.selector
    }

    /// Returns the physical instance or array extent reached by the selector.
    #[must_use]
    pub const fn target(&self) -> FieldStatisticsRouteTarget {
        self.target
    }
}

/// An original branch actually entered by the scanner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum FieldStatisticsRouteTarget {
    /// An accepted structure or inline layout.
    #[non_exhaustive]
    Instance {
        /// The physical Struct table index of the owning instance.
        owner_structure: u32,
        /// The position of the owning instance in its Struct block.
        owner_instance: u32,
        /// The layout used to traverse this branch.
        layout_structure: u32,
    },
    /// An accepted array extent, with the original header available on the edge.
    #[non_exhaustive]
    Array {
        /// The first position in the original value or instance pool.
        first: u32,
        /// The original number of elements in the selected extent.
        count: u32,
    },
}

#[derive(Debug)]
struct Node {
    parent: Option<usize>,
    edge: FieldStatisticsRouteEdge,
}

#[derive(Debug, Default)]
pub(super) struct Routes {
    nodes: Vec<Node>,
    current: Option<usize>,
}

impl Routes {
    pub(super) const fn new() -> Self {
        Self {
            nodes: Vec::new(),
            current: None,
        }
    }

    pub(super) fn reset(&mut self) {
        self.nodes.clear();
        self.current = None;
    }

    pub(super) fn enter(
        &mut self,
        selector: FieldStatisticsInput,
        target: FieldStatisticsRouteTarget,
    ) {
        let current = self.nodes.len();
        self.nodes.push(Node {
            parent: self.current,
            edge: FieldStatisticsRouteEdge { selector, target },
        });
        self.current = Some(current);
    }

    pub(super) fn leave(&mut self) {
        self.current = self
            .current
            .and_then(|current| self.nodes.get(current)?.parent);
    }

    pub(super) const fn position(&self) -> Option<usize> {
        self.current
    }

    pub(super) fn at(&self, position: Option<usize>) -> Route<'_> {
        Route {
            nodes: &self.nodes,
            position,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct Route<'scan> {
    nodes: &'scan [Node],
    position: Option<usize>,
}

impl<'scan> Route<'scan> {
    pub(super) fn iter(self) -> impl Iterator<Item = &'scan FieldStatisticsRouteEdge> {
        iter::successors(self.position, move |&index| self.nodes.get(index)?.parent)
            .filter_map(move |index| self.nodes.get(index).map(|node| &node.edge))
    }
}
