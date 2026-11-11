// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;
use std::ops::Deref;
use std::sync::Arc;

use crate::LoadoutXmlClassUse;
use crate::spec::{EntrySpec, LoadoutSpec};

/// A shared list of manual entries and the exact XML use inherited by its nested items.
#[derive(Debug, Clone)]
pub(crate) struct ManualLoadout {
    entries: Arc<[EntrySpec]>,
    usage: Option<LoadoutXmlClassUse>,
}

impl ManualLoadout {
    pub(crate) const fn new(entries: Arc<[EntrySpec]>) -> Self {
        Self {
            entries,
            usage: None,
        }
    }

    pub(crate) const fn shared(&self) -> &Arc<[EntrySpec]> {
        &self.entries
    }

    pub(crate) const fn usage(&self) -> Option<&LoadoutXmlClassUse> {
        self.usage.as_ref()
    }

    pub(crate) fn bind(&mut self, usage: LoadoutXmlClassUse) {
        if self.usage.is_none() {
            self.usage = Some(usage);
        }
    }
}

pub(crate) fn entries<'entries>(
    list: &'entries [EntrySpec],
    usage: Option<&'entries LoadoutXmlClassUse>,
) -> impl Iterator<Item = Cow<'entries, EntrySpec>> {
    list.iter().map(move |entry| match usage {
        Some(usage) if entry.xml_use.is_none() => Cow::Owned(entry.clone().for_xml_use(usage)),
        _ => Cow::Borrowed(entry),
    })
}

impl Deref for ManualLoadout {
    type Target = [EntrySpec];

    fn deref(&self) -> &Self::Target {
        &self.entries
    }
}

impl PartialEq for ManualLoadout {
    fn eq(&self, other: &Self) -> bool {
        self.entries == other.entries
    }
}

impl Eq for ManualLoadout {}

impl EntrySpec {
    pub(crate) fn for_xml_use(mut self, root: &LoadoutXmlClassUse) -> Self {
        let selected = root.with_route(self.xml_route.take());
        if let Some(LoadoutSpec::Manual(nested)) = &mut self.loadout {
            nested.bind(selected.clone());
        }
        self.xml_use = Some(selected);
        self
    }
}

impl PartialEq for EntrySpec {
    fn eq(&self, other: &Self) -> bool {
        self.origin == other.origin
            && self.port == other.port
            && self.target == other.target
            && self.loadout == other.loadout
    }
}

impl Eq for EntrySpec {}
