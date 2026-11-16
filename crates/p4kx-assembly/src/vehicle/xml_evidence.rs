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
}

impl VehicleXmlSite {
    pub(crate) const fn new(
        file: LoadoutFileObservation,
        location: Option<SourceLocation>,
    ) -> Self {
        Self { file, location }
    }

    /// Returns the file operation whose XML supplied this site.
    #[must_use]
    pub const fn file(&self) -> &LoadoutFileObservation {
        &self.file
    }

    /// Returns the original attribute or checked-absence coordinate when available.
    #[must_use]
    pub const fn location(&self) -> Option<&SourceLocation> {
        self.location.as_ref()
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

    pub(crate) fn with_target(assignment: &Arc<Self>, target: Option<&Arc<Self>>) -> Arc<Self> {
        let Some(target) = target else {
            return Arc::clone(assignment);
        };
        let mut selectors = assignment.selectors.clone();
        selectors.push(target.original.clone());
        selectors.extend(target.selectors.iter().cloned());
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
