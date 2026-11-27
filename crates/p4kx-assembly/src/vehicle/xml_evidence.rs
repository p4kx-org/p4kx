// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_extraction::SourceLocation;

use crate::LoadoutFileObservation;

/// One original XML coordinate and its actual file operation.
#[derive(Debug, Clone)]
pub struct VehicleXmlSite {
    file: LoadoutFileObservation,
    location: Option<SourceLocation>,
    checked_absence: bool,
}

impl VehicleXmlSite {
    pub(crate) const fn new(
        file: LoadoutFileObservation,
        location: Option<SourceLocation>,
        checked_absence: bool,
    ) -> Self {
        Self {
            file,
            location,
            checked_absence,
        }
    }

    /// Returns the file operation whose XML supplied this site.
    #[must_use]
    pub const fn file(&self) -> &LoadoutFileObservation {
        &self.file
    }

    pub(crate) fn with_field(&self, field: &str) -> Self {
        let location = self.location.as_ref().and_then(|location| match location {
            SourceLocation::Subresource {
                namespace, indices, ..
            } => Some(SourceLocation::Subresource {
                namespace: namespace.clone(),
                indices: indices.clone(),
                field: field.to_owned(),
            }),
            _ => None,
        });
        Self::new(self.file.clone(), location, self.checked_absence)
    }

    /// Returns the original attribute or checked-absence coordinate when available.
    #[must_use]
    pub const fn location(&self) -> Option<&SourceLocation> {
        self.location.as_ref()
    }

    /// Reports whether the site observes an omitted attribute.
    #[must_use]
    pub const fn checked_absence(&self) -> bool {
        self.checked_absence
    }
}

/// Original input and observed selectors for an effective vehicle XML attribute.
#[derive(Debug)]
pub struct VehicleAttributeEvidence {
    original: VehicleXmlSite,
    selectors: Vec<VehicleXmlSite>,
}

impl VehicleAttributeEvidence {
    pub(crate) const fn new(original: VehicleXmlSite, selectors: Vec<VehicleXmlSite>) -> Self {
        Self {
            original,
            selectors,
        }
    }

    pub(crate) fn with_targets(
        assignment: &Arc<Self>,
        targets: &[Option<&Arc<Self>>],
    ) -> Arc<Self> {
        let mut targets = targets
            .iter()
            .flatten()
            .copied()
            .filter(|target| !Arc::ptr_eq(assignment, target))
            .peekable();
        if targets.peek().is_none() {
            return Arc::clone(assignment);
        }
        let mut selectors = assignment.selectors.clone();
        for target in targets {
            selectors.push(target.original.clone());
            selectors.extend(target.selectors.iter().cloned());
        }
        Arc::new(Self::new(assignment.original.clone(), selectors))
    }

    /// Returns the original attribute or scope supplying the effective value.
    #[must_use]
    pub const fn original(&self) -> &VehicleXmlSite {
        &self.original
    }

    /// Returns the selector inputs captured with the effective assignment.
    #[must_use]
    pub fn selectors(&self) -> &[VehicleXmlSite] {
        &self.selectors
    }
}
