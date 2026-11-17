// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! One group of an assortment: its alternatives and the choice among them.

use std::sync::Arc;

use p4kx_dataforge::{StructValue, Value};

use crate::LoadoutTargetGuid;
use crate::loadout_path::{PathRead, XML_LOADOUT_PATH};
use crate::pick::{Choice, LoadoutPicks};
use crate::spec::LoadoutSpec;
use crate::spec_loadout::{XML_LOADOUT, read_loadout};
use crate::{LoadoutError, LoadoutReference};

/// One admitted assortment alternative.
#[derive(Debug, Clone)]
pub(crate) enum Alternative {
    /// A decoded Loadout.
    Loadout(LoadoutSpec),
    /// An XML choice whose authored String path is empty.
    EmptyXmlPath,
    /// An alternative whose Loadout could not be read.
    Unreadable,
}

impl Alternative {
    pub(crate) fn read(loadout: &StructValue<'_>, record: &Arc<str>, paths: &PathRead<'_>) -> Self {
        if loadout.type_name() == XML_LOADOUT
            && matches!(loadout.field(XML_LOADOUT_PATH), Some(Value::String("")))
        {
            return Self::EmptyXmlPath;
        }
        read_loadout(loadout, record, paths).map_or(Self::Unreadable, Self::Loadout)
    }
}

/// A group of an assortment with its alternatives in order.
#[derive(Debug, Clone)]
pub(crate) struct Group {
    name: String,
    alternatives: Vec<Alternative>,
}

impl Group {
    pub(crate) const fn new(name: String, alternatives: Vec<Alternative>) -> Self {
        Self { name, alternatives }
    }

    /// Returns the name of the group.
    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    /// Returns the selected Loadout layer. A chosen unreadable alternative records an issue.
    pub(crate) fn chosen(
        &self,
        picks: &LoadoutPicks,
        reference: &LoadoutReference,
        issues: &mut Vec<LoadoutError>,
    ) -> Option<LoadoutSpec> {
        let index = self.index(picks, reference, issues);
        match self.alternatives.get(index)? {
            Alternative::Loadout(loadout) => Some(loadout.clone()),
            Alternative::EmptyXmlPath => None,
            Alternative::Unreadable => {
                let which = if index == 0 {
                    "the first alternative".to_owned()
                } else {
                    format!("the alternative {index}")
                };
                issues.push(LoadoutError::Assortment {
                    guid: LoadoutTargetGuid::from_reference(reference),
                    reason: format!("{which} of the group {} is no Loadout", self.name),
                });
                None
            }
        }
    }

    /// Returns the index of the alternative `picks` choose. A named index beyond the group is an
    /// issue of the assortment `reference` and gives 0; a bare index beyond it gives 0 silently.
    fn index(
        &self,
        picks: &LoadoutPicks,
        reference: &LoadoutReference,
        issues: &mut Vec<LoadoutError>,
    ) -> usize {
        let count = self.alternatives.len();
        match picks.choice(&self.name) {
            Choice::Named(index) if index > 0 && index >= count => {
                issues.push(LoadoutError::Assortment {
                    guid: LoadoutTargetGuid::from_reference(reference),
                    reason: format!(
                        "the pick {index} of the group {} is beyond its {count} \
                         alternatives; the first is used",
                        self.name
                    ),
                });
                0
            }
            Choice::Named(index) => index,
            Choice::Every(index) if index < count => index,
            Choice::Every(_) | Choice::First => 0,
        }
    }
}

#[cfg(test)]
#[path = "assortment_group_tests.rs"]
mod tests;
