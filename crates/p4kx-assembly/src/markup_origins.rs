// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use crate::{ValueOrigin, VehicleAttributeEvidence, VehicleXmlSite};

#[derive(Debug, Clone, Default)]
pub(crate) struct AttributeOrigins {
    entries: Vec<AttributeOrigin>,
    element: Option<Arc<VehicleXmlSite>>,
}

#[derive(Debug, Clone)]
struct AttributeOrigin {
    name: String,
    origin: Option<ValueOrigin>,
    evidence: Option<Arc<VehicleAttributeEvidence>>,
    donor_ordinal: Option<usize>,
}

impl PartialEq for AttributeOrigins {
    fn eq(&self, other: &Self) -> bool {
        self.classification()
            .all(|(name, origin)| other.origin_of(name) == origin)
            && other
                .classification()
                .all(|(name, origin)| self.origin_of(name) == origin)
    }
}

impl Eq for AttributeOrigins {}

impl AttributeOrigins {
    pub(crate) fn set_element(&mut self, site: Option<VehicleXmlSite>) {
        self.element = site.map(Arc::new);
    }

    pub(crate) fn element(&self) -> Option<&VehicleXmlSite> {
        self.element.as_deref()
    }

    pub(crate) fn shared_element(&self) -> Option<Arc<VehicleXmlSite>> {
        self.element.clone()
    }

    fn classification(&self) -> impl Iterator<Item = (&str, ValueOrigin)> {
        self.entries
            .iter()
            .filter_map(|entry| entry.origin.map(|origin| (entry.name.as_str(), origin)))
    }

    pub(crate) fn origin_of(&self, name: &str) -> ValueOrigin {
        self.entries
            .iter()
            .find(|entry| entry.is_effective(name))
            .and_then(|entry| entry.origin)
            .unwrap_or_default()
    }

    pub(crate) fn evidence(&self, name: &str) -> Option<&Arc<VehicleAttributeEvidence>> {
        self.entries
            .iter()
            .find(|entry| entry.is_effective(name))?
            .evidence
            .as_ref()
    }

    pub(crate) fn retain_donor(
        &mut self,
        name: &str,
        ordinal: usize,
        evidence: Option<Arc<VehicleAttributeEvidence>>,
    ) {
        self.entries.push(AttributeOrigin {
            name: name.to_owned(),
            origin: None,
            evidence,
            donor_ordinal: Some(ordinal),
        });
    }

    pub(crate) fn donor_evidence(
        &self,
        name: &str,
        ordinal: usize,
    ) -> Option<&Arc<VehicleAttributeEvidence>> {
        if let Some(slot) = self
            .entries
            .iter()
            .find(|entry| entry.donor_ordinal == Some(ordinal) && entry.name == name)
        {
            return slot.evidence.as_ref();
        }
        self.evidence(name)
    }

    pub(crate) fn mark(&mut self, name: &str, origin: ValueOrigin) {
        self.update(name, |entry| entry.origin = Some(origin));
    }

    pub(crate) fn assign(
        &mut self,
        name: &str,
        origin: ValueOrigin,
        evidence: Option<Arc<VehicleAttributeEvidence>>,
    ) {
        let marked = (origin != ValueOrigin::Base).then_some(origin);
        if evidence.is_none()
            && marked.is_none()
            && !self.entries.iter().any(|entry| entry.is_effective(name))
        {
            return;
        }
        self.update(name, |entry| {
            entry.origin = marked;
            entry.evidence = evidence;
        });
    }

    fn update(&mut self, name: &str, change: impl FnOnce(&mut AttributeOrigin)) {
        if let Some(entry) = self
            .entries
            .iter_mut()
            .find(|entry| entry.is_effective(name))
        {
            change(entry);
            return;
        }
        let mut entry = AttributeOrigin {
            name: name.to_owned(),
            origin: None,
            evidence: None,
            donor_ordinal: None,
        };
        change(&mut entry);
        self.entries.push(entry);
    }
}

impl AttributeOrigin {
    fn is_effective(&self, name: &str) -> bool {
        self.donor_ordinal.is_none() && self.name.eq_ignore_ascii_case(name)
    }
}

#[cfg(test)]
mod tests {
    use super::AttributeOrigins;
    use crate::ValueOrigin;

    #[test]
    fn observed_slots_preserve_effective_attribute_equality() {
        let mut plain = AttributeOrigins::default();
        plain.mark("Flags", ValueOrigin::Patch);
        plain.mark("Id", ValueOrigin::Elem);

        let mut observed = AttributeOrigins::default();
        observed.update("id", |_| {});
        observed.update("flags", |_| {});
        observed.mark("Flags", ValueOrigin::Patch);
        observed.mark("Id", ValueOrigin::Elem);
        assert_eq!(observed, plain, "capture order and casing are descriptive");

        observed.mark("flags", ValueOrigin::Elem);
        assert_ne!(observed, plain, "the effective source category has changed");
    }
}
