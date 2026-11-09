// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::{DataForgeError, Guid, Record, RecordTree};

#[path = "tag_path_source.rs"]
mod tag_path_source;

pub use tag_path_source::{TagPathEdgeKind, TagPathIncoming};

/// A limit that prevents a canonically read tag from entering the tag tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TagProjectionFailure {
    /// The tag exceeds the permitted depth.
    Depth,
    /// The tag name exceeds its byte limit.
    NameBytes,
    /// The tag path exceeds its byte limit.
    PathBytes,
    /// The accumulated tag paths exceed their byte budget.
    TextBytes,
    /// The node count exceeds the supported index range.
    NodeCount,
    /// A discarded ancestor prevents this tag from being reached.
    AncestorDiscarded,
}

/// Why a tag relation was not added to the tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TagRelationIssue {
    /// The relation does not identify an available Tag Record.
    Dangling,
    /// The target was already visited through an earlier relation.
    Repeated,
    /// A populated relation value is not a Reference.
    MalformedValue,
}

/// An actual operation performed while building a tag tree.
#[derive(Debug)]
pub enum TagBuildEvent<'r, 'f> {
    /// Record metadata cannot establish whether the Record belongs to the tag tree.
    MetadataFailed {
        /// The physical Record whose metadata was inspected.
        record: &'r Record<'f>,
        /// The metadata decoding error.
        error: &'r DataForgeError,
    },
    /// One canonical Record read completed or failed.
    CanonicalRead {
        /// The physical source Record.
        record: &'r Record<'f>,
        /// The actual canonical result before optional projection helpers run.
        outcome: Result<&'r RecordTree<'f>, &'r DataForgeError>,
    },
    /// A later canonically readable Tag Record duplicates an earlier GUID.
    DuplicateSkipped {
        /// The physical duplicate Record.
        record: &'r Record<'f>,
    },
    /// A canonical Tag or `TagDatabase` was projected into its tree representation.
    Projected {
        /// The physical projected Record.
        record: &'r Record<'f>,
    },
    /// One actual first-selected tag path with its original incoming occurrence.
    PathChosen {
        /// The physical Tag Record whose name contributes to this path.
        record: &'r Record<'f>,
        /// Position of the selected node in the tree's pre-order.
        position: u32,
        /// Position of its selected parent, null for a root.
        parent_position: Option<u32>,
        /// The actual selected source edge, absent for an unlisted root.
        incoming: Option<TagPathIncoming<'r, 'f>>,
        /// The joined path actually materialized by the tree.
        path: &'r str,
        /// The actual selected tree depth.
        depth: u16,
        /// Whether this path descends from a declared database root.
        listed: bool,
    },
    /// A canonical Tag was discarded by a projection limit.
    ProjectionFailed {
        /// The physical discarded Record.
        record: &'r Record<'f>,
        /// The exact projection limit.
        reason: TagProjectionFailure,
    },
    /// A relation was omitted without replacing the source Record's projection outcome.
    RelationSkipped {
        /// The physical Record that owns the relation.
        record: &'r Record<'f>,
        /// The owned relation field.
        field: &'static str,
        /// The target identity when the relation contains one.
        target: Option<Guid>,
        /// Why the relation was omitted.
        reason: TagRelationIssue,
    },
    /// The caller's cancellation predicate stopped the build.
    Cancelled,
}

#[cfg(test)]
#[path = "tag_observer_tests.rs"]
mod tests;
