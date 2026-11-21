// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::EntryOrigin;

use super::aggregate_evidence::Checkpoint;
use super::{VehicleAggregateEvidence, VehicleImplementation};
use crate::{LoadoutFileObservation, VehicleAttributeEvidence, VehicleXmlSite};

/// Where the value of a part's attribute comes from once the Modification is applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ValueOrigin {
    /// The implementation file named by the Entity.
    #[default]
    Base,
    /// The patch file of the Modification, which replaced or merged the part.
    Patch,
    /// An `Elem` of the Modification, which set the attribute.
    Elem,
}

impl ValueOrigin {
    /// Returns the stable code of the origin, e.g. `elem`.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Base => "base",
            Self::Patch => "patch",
            Self::Elem => "elem",
        }
    }
}

#[derive(Debug, Clone, Default)]
pub(super) struct DeclarationOrigins {
    pub(super) aggregate: Option<VehicleAggregateEvidence>,
    pub(super) totals_observed: bool,
    pub(super) port_documents: Option<Checkpoint>,
    pub(super) port_parts: Option<VehicleAggregateEvidence>,
    pub(super) port_occurrences: Vec<Checkpoint>,
    pub(super) rolling_documents: Option<Checkpoint>,
    pub(super) rolling_parts: Option<VehicleAggregateEvidence>,
    pub(super) base: Option<EntryOrigin>,
    pub(super) patch: Option<EntryOrigin>,
    pub(super) patch_route: Option<PatchRoute>,
}

#[derive(Debug, Clone)]
pub(super) struct PatchRoute {
    pub(super) file: LoadoutFileObservation,
    pub(super) selector: VehicleXmlSite,
}

impl PartialEq for DeclarationOrigins {
    fn eq(&self, other: &Self) -> bool {
        let binding = |origin: &Option<EntryOrigin>| origin.as_ref().map(EntryOrigin::binding);
        binding(&self.base) == binding(&other.base) && binding(&self.patch) == binding(&other.patch)
    }
}

impl VehicleImplementation {
    pub(crate) fn explicit_patch_selector(
        &self,
        evidence: &VehicleAttributeEvidence,
    ) -> Option<&VehicleXmlSite> {
        let route = self.declarations.patch_route.as_ref()?;
        std::iter::once(evidence.original())
            .chain(evidence.selectors())
            .any(|site| site.file().is_same_operation(&route.file))
            .then_some(&route.selector)
    }

    /// Returns the original archive input supplying the effective declaration.
    #[must_use]
    pub const fn declaration_origin(&self, origin: ValueOrigin) -> Option<&EntryOrigin> {
        match origin {
            ValueOrigin::Base | ValueOrigin::Elem => self.declarations.base.as_ref(),
            ValueOrigin::Patch => self.declarations.patch.as_ref(),
        }
    }

    /// Returns the path of the file a value with `origin` was read from: the implementation file
    /// for [`ValueOrigin::Base`] and [`ValueOrigin::Elem`], whose `Elem` it holds, and the patch
    /// file for [`ValueOrigin::Patch`]. `None` when the origin is `Patch` and no patch file was
    /// applied.
    #[must_use]
    pub fn source_path(&self, origin: ValueOrigin) -> Option<&str> {
        match origin {
            ValueOrigin::Patch => self.patch_path(),
            ValueOrigin::Base | ValueOrigin::Elem => Some(self.path()),
        }
    }
}
