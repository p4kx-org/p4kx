// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! One group of an assortment: its alternatives and the choice among them.

use p4kx_dataforge::Guid;

use crate::LoadoutError;
use crate::pick::{Choice, LoadoutPicks};
use crate::spec::LoadoutSpec;

/// A group of an assortment with every alternative in order; `None` is an alternative that is no
/// Loadout.
#[derive(Debug, Clone)]
pub(crate) struct Group {
    name: String,
    alternatives: Vec<Option<LoadoutSpec>>,
}

impl Group {
    pub(crate) const fn new(name: String, alternatives: Vec<Option<LoadoutSpec>>) -> Self {
        Self { name, alternatives }
    }

    /// Returns the name of the group.
    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    /// Returns the Loadout of the alternative `picks` choose, or `None` when the group gives
    /// nothing: it has no alternatives, or the chosen one is no Loadout, which is an issue of the
    /// assortment `guid`.
    pub(crate) fn chosen(
        &self,
        picks: &LoadoutPicks,
        guid: Guid,
        issues: &mut Vec<LoadoutError>,
    ) -> Option<LoadoutSpec> {
        let index = self.index(picks, guid, issues);
        let alternative = self.alternatives.get(index)?;
        if alternative.is_none() {
            let which = if index == 0 {
                "the first alternative".to_owned()
            } else {
                format!("the alternative {index}")
            };
            issues.push(LoadoutError::Assortment {
                guid,
                reason: format!("{which} of the group {} is no Loadout", self.name),
            });
        }
        alternative.clone()
    }

    /// Returns the index of the alternative `picks` choose. A named index beyond the group is an
    /// issue of the assortment `guid` and gives 0; a bare index beyond it gives 0 silently.
    fn index(&self, picks: &LoadoutPicks, guid: Guid, issues: &mut Vec<LoadoutError>) -> usize {
        let count = self.alternatives.len();
        match picks.choice(&self.name) {
            Choice::Named(index) if index > 0 && index >= count => {
                issues.push(LoadoutError::Assortment {
                    guid,
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
