// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use p4kx_extraction::SourceLocation;

use crate::markup::Element;
use crate::{VehicleAttributeEvidence, VehicleImplementation, VehicleXmlSite};

/// An unavailable stage of the requested vehicle aggregate interpretation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum VehicleAggregateIssue {
    /// An original selector could not be retained.
    OriginalEvidenceUnavailable,
    /// The requested Modification and its patch could not be selected.
    ModificationUnavailable,
    /// The explicitly authored patch was absent.
    ExplicitPatchMissing,
    /// An authored patch path was outside the permitted vehicle directory.
    PatchPathRejected,
    /// A configured patch could not be read.
    PatchReadFailed,
    /// A patch could not be parsed or applied.
    PatchApplicationFailed,
    /// The selected Elem overrides could not be applied.
    ElementOverridesFailed,
}

/// Original selection and membership selectors shared by both vehicle sums.
/// Join these same-read XML witnesses with the current caller and numeric field evidence.
#[derive(Debug, Clone)]
pub struct VehicleAggregateEvidence {
    selectors: Vec<Arc<VehicleXmlSite>>,
    seen: HashSet<SelectorSite>,
    issues: Vec<VehicleAggregateIssue>,
}

#[derive(Debug, Clone)]
struct SelectorSite(Arc<VehicleXmlSite>);

impl PartialEq for SelectorSite {
    fn eq(&self, other: &Self) -> bool {
        self.0.file().is_same_operation(other.0.file()) && self.0.location() == other.0.location()
    }
}

impl Eq for SelectorSite {}

impl Hash for SelectorSite {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.file().operation_key().hash(state);
        if let Some(SourceLocation::Subresource {
            namespace,
            indices,
            field,
        }) = self.0.location()
        {
            namespace.hash(state);
            indices.hash(state);
            field.hash(state);
        }
    }
}

/// A reversible prefix of staged aggregate evidence.
#[derive(Clone, Copy)]
pub(super) struct Checkpoint {
    selectors: usize,
    issues: usize,
}

impl VehicleAggregateEvidence {
    pub(super) fn new() -> Self {
        Self {
            selectors: Vec::new(),
            seen: HashSet::new(),
            issues: Vec::new(),
        }
    }

    /// Borrows distinct actual selectors in their first observed order.
    /// Numeric conversion uses come from [`VehicleImplementation::totals_with_field_uses`].
    pub fn selectors(&self) -> impl Iterator<Item = &VehicleXmlSite> {
        self.selectors.iter().map(Arc::as_ref)
    }

    /// Borrows unavailable interpretation stages independently of original coordinates.
    #[must_use]
    pub fn issues(&self) -> &[VehicleAggregateIssue] {
        &self.issues
    }

    /// Returns whether every retained selector has its original coordinate.
    /// Caller binding and requested-stage health require separate checks.
    #[must_use]
    pub fn has_complete_coordinates(&self) -> bool {
        !self
            .issues
            .contains(&VehicleAggregateIssue::OriginalEvidenceUnavailable)
            && self.selectors.iter().all(|site| site.location().is_some())
    }

    pub(super) fn issue(&mut self, issue: VehicleAggregateIssue) {
        if !self.issues.contains(&issue) {
            self.issues.push(issue);
        }
    }

    pub(super) fn selection(&mut self, site: Option<VehicleXmlSite>) {
        let Some(site) = site else {
            self.issue(VehicleAggregateIssue::OriginalEvidenceUnavailable);
            return;
        };
        let key = SelectorSite(Arc::new(site));
        if self.seen.insert(key.clone()) {
            self.selectors.push(key.0);
        }
    }

    pub(super) fn element(&mut self, element: &Element, field: &str) {
        self.selection(element.origins.element().map(|site| site.with_field(field)));
    }

    pub(super) fn field(&mut self, element: &Element, name: &str) {
        match element.origins.evidence(name) {
            Some(evidence) => self.assignment(evidence),
            None => self.element(element, "$attributes"),
        }
    }

    pub(super) fn assignment(&mut self, evidence: &VehicleAttributeEvidence) {
        self.selection(Some(evidence.original().clone()));
        for selector in evidence.selectors() {
            self.selection(Some(selector.clone()));
        }
    }

    pub(super) const fn checkpoint(&self) -> Checkpoint {
        Checkpoint {
            selectors: self.selectors.len(),
            issues: self.issues.len(),
        }
    }

    pub(super) fn restore(&mut self, checkpoint: Checkpoint) {
        for site in self.selectors.drain(checkpoint.selectors..) {
            self.seen.remove(&SelectorSite(site));
        }
        self.issues.truncate(checkpoint.issues);
    }
}

impl VehicleImplementation {
    /// Borrows opt-in original aggregate evidence from the actual interpretation.
    /// Publish owned results only after the outer resolution succeeds.
    #[must_use]
    pub const fn aggregate_evidence(&self) -> Option<&VehicleAggregateEvidence> {
        self.declarations.aggregate.as_ref()
    }
}

#[cfg(test)]
#[path = "aggregate_membership_tests.rs"]
mod membership_tests;
#[cfg(test)]
#[path = "aggregate_patch_tests.rs"]
mod patch_tests;
#[cfg(test)]
#[path = "aggregate_presence_tests.rs"]
mod presence_tests;
#[cfg(test)]
#[path = "aggregate_test_support.rs"]
pub(crate) mod support;
