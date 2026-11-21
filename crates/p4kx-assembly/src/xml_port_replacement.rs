// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use crate::loadout_xml_use::XmlRoute;
use crate::spec::EntrySpec;
use crate::{LoadoutFileObservation, LoadoutPath};

/// A physical XML port replacement performed by the Loadout merge.
#[derive(Debug)]
pub struct XmlPortReplacement {
    document: Arc<str>,
    ordinal: usize,
    displaced: XmlPortSelection,
    winner: XmlPortSelection,
}

impl XmlPortReplacement {
    pub(crate) fn new(
        document: &LoadoutPath,
        ordinal: usize,
        displaced: &EntrySpec,
        winner: &EntrySpec,
    ) -> Self {
        Self {
            document: Arc::clone(document.text()),
            ordinal,
            displaced: XmlPortSelection::of(displaced),
            winner: XmlPortSelection::of(winner),
        }
    }

    /// Borrows the path of the XML document whose port list is being merged.
    #[must_use]
    pub fn document(&self) -> &str {
        &self.document
    }

    /// Returns the replaced sibling position in the merge document's port list.
    #[must_use]
    pub const fn ordinal(&self) -> usize {
        self.ordinal
    }

    /// Borrows the displaced port selector.
    #[must_use]
    pub const fn displaced(&self) -> &XmlPortSelection {
        &self.displaced
    }

    /// Borrows the winning port selector.
    #[must_use]
    pub const fn winner(&self) -> &XmlPortSelection {
        &self.winner
    }
}

/// An original XML port selector and the Include route that supplied it.
#[derive(Debug)]
pub struct XmlPortSelection {
    name: LoadoutPath,
    route: Option<Arc<XmlRoute>>,
}

impl XmlPortSelection {
    fn of(entry: &EntrySpec) -> Self {
        Self {
            name: entry
                .port_proof
                .clone()
                .unwrap_or_else(|| LoadoutPath::unbound(entry.port.as_str())),
            route: entry.xml_route.clone(),
        }
    }

    /// Borrows the original port name with its source selection when available.
    #[must_use]
    pub const fn name(&self) -> &LoadoutPath {
        &self.name
    }

    /// Visits the original Include selectors from the merge document to this port.
    pub fn includes(
        &self,
    ) -> impl Iterator<
        Item = (
            &LoadoutFileObservation,
            &LoadoutFileObservation,
            &LoadoutPath,
        ),
    > {
        XmlRoute::includes(self.route.as_deref())
    }
}
