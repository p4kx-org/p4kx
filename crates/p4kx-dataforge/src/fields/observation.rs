// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::iter;
use std::ops::Range;

use super::FieldKind;
use super::observation_metrics::FieldStatisticsUse;
use super::observation_route::{FieldStatisticsRouteEdge, Route};
use super::trie::{ROOT, Trie};

/// The physical storage examined for one field-statistics contribution.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct FieldStatisticsSlot {
    /// The original table index of the Record currently being scanned.
    pub record_index: u32,
    /// The Struct table index of the owning physical instance.
    pub owner_structure: u32,
    /// The original instance position within its owner Struct block.
    pub owner_instance: u32,
    /// The concrete layout used to read the immediate field.
    pub layout_structure: u32,
    /// The inherited-first position of the field in that layout.
    pub slot_position: u64,
    /// The original property-definition table index used for the field.
    pub property_index: usize,
    /// The storage kind of the original field before array elements are traversed.
    pub kind: FieldKind,
    /// The half-open decoded DCB byte range of the field, when nonempty and available.
    pub bytes: Option<Range<u64>>,
    /// The half-open decoded DCB byte range of the consulted field name.
    pub name: Option<Range<u64>>,
}

/// One physical slot or element examined by the statistics scanner.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum FieldStatisticsInput {
    /// A requested Record position, subject to the outcome reported by the scanner.
    Record(u32),
    /// An actual field slot, including an array header when the field is an array.
    Slot(FieldStatisticsSlot),
    /// An original element admitted by the enclosing array header.
    #[non_exhaustive]
    ArrayElement {
        /// The physical slot whose array header admitted the element.
        header: FieldStatisticsSlot,
        /// The zero-based index within the field's authored array.
        index: u32,
        /// The index in the original value or instance pool.
        pool_index: u32,
        /// The half-open decoded DCB byte range of a scalar element, when available.
        bytes: Option<Range<u64>>,
    },
}

/// An original input of a field-statistics scan and the path it was examined for.
#[derive(Debug)]
pub struct FieldStatisticsObservation<'scan> {
    trie: &'scan Trie<'scan>,
    node: u32,
    input: FieldStatisticsInput,
    usage: FieldStatisticsUse,
    route: Route<'scan>,
}

impl<'scan> FieldStatisticsObservation<'scan> {
    pub(super) const fn new(
        trie: &'scan Trie<'scan>,
        node: u32,
        input: FieldStatisticsInput,
        usage: FieldStatisticsUse,
        route: Route<'scan>,
    ) -> Self {
        Self {
            trie,
            node,
            input,
            usage,
            route,
        }
    }

    /// Returns the path segments from the examined field back towards the Record.
    pub fn leaf_to_root(&self) -> impl Iterator<Item = &str> + '_ {
        iter::successors(Some(self.node), |&node| {
            self.trie.nodes.get(node as usize).map(|node| node.parent)
        })
        .take_while(|&node| node != ROOT)
        .filter_map(|node| self.trie.nodes.get(node as usize).map(|node| node.segment))
    }

    /// Returns the actual enclosing branches, nearest parent first.
    pub fn enclosing_route(&self) -> impl Iterator<Item = &FieldStatisticsRouteEdge> + '_ {
        self.route.iter()
    }

    /// Returns how the scanner accounted for the consulted input.
    #[must_use]
    pub const fn usage(&self) -> &FieldStatisticsUse {
        &self.usage
    }

    /// Returns the consulted physical position independently of the path.
    #[must_use]
    pub const fn input(&self) -> &FieldStatisticsInput {
        &self.input
    }

    /// Returns the consulted physical position.
    #[must_use]
    pub const fn into_input(self) -> FieldStatisticsInput {
        self.input
    }
}

pub(super) type Observe<'scan> = &'scan (dyn Fn(FieldStatisticsObservation<'_>) + Sync);
