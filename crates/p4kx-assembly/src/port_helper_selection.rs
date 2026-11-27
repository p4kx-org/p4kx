// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::port_sub_helper::PortSubHelper;
use crate::{PortDefinition, PortTransform};

#[path = "port_helper_walk.rs"]
mod walk;

/// Evaluation of one complete authored tag selector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum PortHelperTagMatch {
    /// The supported selector matches the attached item's tags.
    Matched,
    /// The supported selector does not match the attached item's tags.
    NotMatched,
    /// The selector cannot be evaluated with the available rules or evidence.
    Unsupported,
}

/// The original root or sequence of nested helper array slots.
#[derive(Debug, Clone, Copy)]
pub struct PortHelperPath<'definition> {
    pub(super) helpers: &'definition [PortSubHelper],
    pub(super) index: Option<usize>,
}

impl PortHelperPath<'_> {
    /// Returns the original array ordinals in root-to-leaf order.
    #[must_use]
    pub fn ordinals(&self) -> Vec<usize> {
        let mut ordinals = Vec::new();
        let mut index = self.index;
        while let Some(node) = index.and_then(|index| self.helpers.get(index)) {
            ordinals.push(node.ordinal());
            index = node.parent();
        }
        ordinals.reverse();
        ordinals
    }
}

/// An original selector consulted by the helper decision.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub struct PortHelperObservation<'definition> {
    /// The original owner of the selector.
    pub path: PortHelperPath<'definition>,
    /// The original text, including an authored empty string.
    pub text: Option<&'definition str>,
    /// The result when the original selector was given to the evaluator.
    pub evaluation: Option<PortHelperTagMatch>,
    /// Whether this branch's original children contributed to the decision.
    pub inspected_children: bool,
}

/// A reason the original helper tree cannot establish a safe selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum PortHelperProblem {
    /// The attachment or helper root could not be read.
    UnreadableTree,
    /// A nonempty selector was authored on the fallback root.
    TaggedRoot,
    /// The authored selector could not be read.
    UnreadableSelector,
    /// The original selector could not be interpreted.
    UnsupportedSelector,
    /// The original children array could not be read.
    UnreadableChildren,
    /// An original child was unreadable at this ordinal in the parent's array.
    UnreadableChild(usize),
    /// A matched branch has no usable helper.
    UnusableHelper,
}

/// The outcome of choosing a helper for a particular attachment.
#[derive(Debug)]
#[non_exhaustive]
pub enum PortHelperChoice<'definition> {
    /// No authored subhelper selector matched, so the root remains the fallback.
    Root(Option<&'definition PortTransform>),
    /// One original, usable subhelper was selected.
    Selected {
        /// The chosen authored transform.
        transform: &'definition PortTransform,
        /// The exact source path of the transform.
        path: PortHelperPath<'definition>,
    },
    /// Multiple usable helpers matched, in their original source order.
    Ambiguous(Vec<PortHelperPath<'definition>>),
    /// Incomplete source evidence or unsupported selection prevented a choice.
    Unresolved {
        /// The source node that prevented selection.
        path: PortHelperPath<'definition>,
        /// The reason selection is unresolved.
        problem: PortHelperProblem,
    },
}

/// A port helper decision and the original selectors it consulted.
#[derive(Debug)]
#[non_exhaustive]
pub struct PortHelperSelection<'definition> {
    /// The transform choice or the reason selection was unresolved.
    pub choice: PortHelperChoice<'definition>,
    /// The consulted source nodes in depth-first order.
    pub observations: Vec<PortHelperObservation<'definition>>,
}

impl PortDefinition {
    /// Selects a subhelper using the caller's evaluation of each original selector.
    ///
    /// All nonempty ancestor selectors on a candidate branch must match. Competing usable
    /// transforms and undecidable authored inputs remain unresolved.
    #[must_use]
    pub fn select_helper(
        &self,
        evaluate: impl FnMut(&str) -> PortHelperTagMatch,
    ) -> PortHelperSelection<'_> {
        walk::select(self, evaluate)
    }
}
