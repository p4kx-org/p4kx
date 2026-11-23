// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use p4kx_extraction::{FieldOrigin, Guid};

use super::{LocationObject, Objects};
use crate::contract::StarmapObject;
use crate::guid_text::CatalogGuid;
use crate::starmap_containers::proof::PlacementEvidence;

impl<'object> LocationObject<'object> {
    pub(crate) const fn new(
        mining_claim: bool,
        above: Option<Guid>,
        origin: Option<&'object FieldOrigin>,
        placement: Option<&'object PlacementEvidence>,
    ) -> Self {
        Self {
            mining_claim,
            above,
            origin,
            placement,
            unique: true,
        }
    }

    pub(crate) const fn with_incomplete_membership(mut self) -> Self {
        self.unique = false;
        self
    }
}

impl<'object> Objects<'object> {
    pub(crate) fn new<'source: 'object>(
        objects: impl IntoIterator<Item = &'object StarmapObject<'source>>,
    ) -> Self {
        Self::from_location_facts(objects.into_iter().map(|object| {
            let fact = LocationObject::new(
                object.mining_claim,
                object.placed_under_guid.map(CatalogGuid::get),
                object.location_origin.as_ref(),
                object.placement_proof.as_ref(),
            );
            (object.guid.0, fact)
        }))
    }

    /// Indexes ancestry facts in the producer order of their physical Records.
    pub(crate) fn from_location_facts(
        facts: impl IntoIterator<Item = (Guid, LocationObject<'object>)>,
    ) -> Self {
        let mut by_guid = HashMap::new();
        for (guid, mut fact) in facts {
            fact.unique &= !by_guid.contains_key(&guid);
            by_guid.insert(guid, fact);
        }
        Self { by_guid }
    }
}
