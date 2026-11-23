// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::aggregate_evidence::Checkpoint;
use super::{
    VehicleAggregateEvidence, VehicleAttributeEvidence, VehicleImplementation, VehicleXmlSite,
    structure,
};
use crate::LoadoutFileObservation;

/// Original XML observations for one effective Vehicle comparison operand.
#[derive(Debug)]
pub struct VehicleStructureEvidence<'vehicle> {
    vehicle: &'vehicle VehicleImplementation,
    documents: &'vehicle VehicleAggregateEvidence,
    prefix: Checkpoint,
}

impl VehicleImplementation {
    /// Borrows structural inputs captured by the vehicle port or totals observer.
    #[must_use]
    pub fn structure_evidence(&self) -> Option<VehicleStructureEvidence<'_>> {
        let origins = &self.declarations;
        Some(VehicleStructureEvidence {
            vehicle: self,
            documents: origins.aggregate.as_ref()?,
            prefix: origins.rolling_documents.or(origins.port_documents)?,
        })
    }
}

impl<'vehicle> VehicleStructureEvidence<'vehicle> {
    /// Borrows the original base operation followed by the successfully applied patch, if any.
    pub fn applied_documents(&self) -> impl Iterator<Item = &'vehicle LoadoutFileObservation> {
        let origins = &self.vehicle.declarations;
        origins
            .structure_base
            .iter()
            .chain(origins.structure_patch.iter())
    }

    /// Iterates selection sites and values of successfully applied comparison-relevant overrides.
    pub fn selectors(&self) -> impl Iterator<Item = &'vehicle VehicleXmlSite> {
        self.documents
            .prefix_selectors(self.prefix)
            .chain(self.elem_sites().flatten())
    }

    /// Reports whether the digest and its original XML observations are complete.
    /// The caller must separately bind the original request and archive lookup.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        let origins = &self.vehicle.declarations;
        self.vehicle.complete_structure_digest().is_ok()
            && self.documents.prefix_complete(self.prefix)
            && origins.structure_base.is_some()
            && (self.vehicle.patch_path().is_some() == origins.structure_patch.is_some())
            && self.applied_documents().all(|file| {
                file.read_outcome().is_ok() && matches!(file.projection_outcome(), Some(Ok(())))
            })
            && self
                .elem_sites()
                .all(|site| site.is_some_and(|site| site.location().is_some()))
    }

    fn elem_sites(&self) -> impl Iterator<Item = Option<&'vehicle VehicleXmlSite>> {
        self.vehicle
            .elems()
            .iter()
            .filter(|elem| structure::is_compared(elem.name(), false))
            .map(|elem| {
                elem.evidence
                    .as_deref()
                    .map(VehicleAttributeEvidence::original)
                    .or(elem.structure_value.as_ref())
            })
    }
}
