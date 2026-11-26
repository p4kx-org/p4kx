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

/// Original selection and membership selectors for a vehicle interpretation.
/// Join these XML observations with the actual caller and field evidence.
#[derive(Debug, Clone)]
pub struct VehicleAggregateEvidence {
    selectors: Vec<Arc<VehicleXmlSite>>,
    issues: Vec<VehicleAggregateIssue>,
    first_unpositioned: Option<usize>,
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
#[derive(Debug, Clone, Copy)]
pub(super) struct Checkpoint {
    selectors: usize,
    issues: usize,
}

impl Checkpoint {
    pub(super) const fn selector_count(self) -> usize {
        self.selectors
    }
}

impl VehicleAggregateEvidence {
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
            && self.first_unpositioned.is_none()
    }

    pub(super) fn prefix_selectors(
        &self,
        checkpoint: Checkpoint,
    ) -> impl Iterator<Item = &VehicleXmlSite> {
        self.selectors().take(checkpoint.selectors)
    }

    pub(super) fn prefix_selector(
        &self,
        checkpoint: Checkpoint,
        index: usize,
    ) -> Option<&VehicleXmlSite> {
        self.selectors
            .get(..checkpoint.selectors)?
            .get(index)
            .map(Arc::as_ref)
    }

    pub(super) fn prefix_complete(&self, checkpoint: Checkpoint) -> bool {
        checkpoint.issues == 0 && self.prefix_has_coordinates(checkpoint)
    }

    pub(super) fn prefix_has_only_missing_explicit_patch(&self, checkpoint: Checkpoint) -> bool {
        matches!(
            self.issues.get(..checkpoint.issues),
            Some([VehicleAggregateIssue::ExplicitPatchMissing])
        ) && self.prefix_has_coordinates(checkpoint)
    }

    fn prefix_has_coordinates(&self, checkpoint: Checkpoint) -> bool {
        checkpoint.selectors <= self.selectors.len()
            && self
                .first_unpositioned
                .is_none_or(|index| index >= checkpoint.selectors)
    }
}

pub(super) struct VehicleEvidenceCapture {
    evidence: VehicleAggregateEvidence,
    seen: HashSet<SelectorSite>,
}

impl VehicleEvidenceCapture {
    pub(super) fn new() -> Self {
        Self {
            evidence: VehicleAggregateEvidence {
                selectors: Vec::new(),
                issues: Vec::new(),
                first_unpositioned: None,
            },
            seen: HashSet::new(),
        }
    }

    pub(super) fn finish(self) -> VehicleAggregateEvidence {
        self.evidence
    }

    #[cfg(test)]
    pub(super) const fn evidence(&self) -> &VehicleAggregateEvidence {
        &self.evidence
    }

    pub(super) fn issue(&mut self, issue: VehicleAggregateIssue) {
        if !self.evidence.issues.contains(&issue) {
            self.evidence.issues.push(issue);
        }
    }

    pub(super) fn selection(&mut self, site: Option<VehicleXmlSite>) {
        let Some(site) = site else {
            self.issue(VehicleAggregateIssue::OriginalEvidenceUnavailable);
            return;
        };
        let key = SelectorSite(Arc::new(site));
        if self.seen.insert(key.clone()) {
            if key.0.location().is_none() && self.evidence.first_unpositioned.is_none() {
                self.evidence.first_unpositioned = Some(self.evidence.selectors.len());
            }
            self.evidence.selectors.push(key.0);
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
            selectors: self.evidence.selectors.len(),
            issues: self.evidence.issues.len(),
        }
    }

    pub(super) fn restore(&mut self, checkpoint: Checkpoint) {
        for site in self.evidence.selectors.drain(checkpoint.selectors..) {
            self.seen.remove(&SelectorSite(site));
        }
        self.evidence.issues.truncate(checkpoint.issues);
        if self
            .evidence
            .first_unpositioned
            .is_some_and(|index| index >= checkpoint.selectors)
        {
            self.evidence.first_unpositioned = None;
        }
    }
}

impl VehicleImplementation {
    /// Returns whether the effective Part graph contains a wheel or tread.
    #[must_use]
    pub const fn has_rolling_parts(&self) -> bool {
        self.rolling_parts
    }

    /// Iterates the document choices and effective Part prefix used by the rolling decision.
    pub fn rolling_part_selectors(&self) -> impl Iterator<Item = &VehicleXmlSite> {
        let declarations = &self.declarations;
        declarations
            .aggregate
            .as_ref()
            .zip(declarations.rolling_documents)
            .into_iter()
            .flat_map(|(evidence, end)| evidence.prefix_selectors(end))
            .chain(
                declarations
                    .rolling_parts
                    .iter()
                    .flat_map(VehicleAggregateEvidence::selectors),
            )
    }

    /// Reports whether all original observations of the rolling decision were retained.
    #[must_use]
    pub fn rolling_part_selectors_complete(&self) -> bool {
        let declarations = &self.declarations;
        declarations
            .aggregate
            .as_ref()
            .zip(declarations.rolling_documents)
            .is_some_and(|(evidence, end)| evidence.prefix_complete(end))
            && declarations
                .rolling_parts
                .as_ref()
                .is_some_and(VehicleAggregateEvidence::has_complete_coordinates)
    }
    /// Borrows opt-in original aggregate evidence from the actual interpretation.
    /// Publish owned results only after the outer resolution succeeds.
    #[must_use]
    pub const fn aggregate_evidence(&self) -> Option<&VehicleAggregateEvidence> {
        if self.declarations.totals_observed {
            self.declarations.aggregate.as_ref()
        } else {
            None
        }
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

#[cfg(test)]
#[path = "structure_evidence_tests.rs"]
mod structure_tests;
