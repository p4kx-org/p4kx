// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The alternative an export takes from each group of a generated Loadout.

use std::collections::HashSet;
use std::fmt;

use crate::ChangeError;

/// The alternative to take from the groups of the assortments of a generated Loadout.
///
/// The game picks one alternative per group at random; without a pick a group gives its first
/// alternative. Indices count from 0, so index 0 is the first alternative. A group is named by
/// its `Group` field in the assortment, matched ignoring ASCII case, in whichever assortment it
/// occurs. A named pick of a group that no group of the resolved Loadout has is an issue of it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct LoadoutPicks {
    every: Option<usize>,
    groups: Vec<(String, usize)>,
}

/// What the picks say about one group.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Choice {
    /// The group is named: an index beyond its alternatives is an issue.
    Named(usize),
    /// Every group is picked: a group that lacks the index keeps its first alternative.
    Every(usize),
    /// Nothing is picked: the first alternative.
    First,
}

impl LoadoutPicks {
    /// Creates picks that leave every group at its first alternative.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            every: None,
            groups: Vec::new(),
        }
    }

    /// Takes the alternative `index` of every group that has it; the other groups keep their
    /// first alternative, which is no issue.
    #[must_use]
    pub const fn every(mut self, index: usize) -> Self {
        self.every = Some(index);
        self
    }

    /// Takes the alternative `index` of the group `group`, replacing an earlier pick of it. An
    /// index beyond the group is an issue of the Loadout and the group keeps its first
    /// alternative.
    #[must_use]
    pub fn group(mut self, group: impl Into<String>, index: usize) -> Self {
        let group = group.into();
        self.groups
            .retain(|(name, _)| !name.eq_ignore_ascii_case(&group));
        self.groups.push((group, index));
        self
    }

    /// Adds the pick `text`, either `<group>=<index>` or a bare `<index>` for every group.
    ///
    /// # Errors
    ///
    /// Returns [`ChangeError::InvalidPick`] when `text` is neither form, or names no group.
    pub fn add_text(self, text: &str) -> Result<Self, ChangeError> {
        let invalid = || ChangeError::InvalidPick {
            text: text.to_owned(),
        };
        match text.split_once('=') {
            Some((group, index)) => {
                let group = group.trim();
                let index = parse_index(index).ok_or_else(invalid)?;
                if group.is_empty() {
                    return Err(invalid());
                }
                Ok(self.group(group, index))
            }
            None => Ok(self.every(parse_index(text).ok_or_else(invalid)?)),
        }
    }

    /// Reads picks from `text`, each `<group>=<index>` or a bare `<index>` as for
    /// [`add_text`](Self::add_text), separated by commas; blank parts are skipped.
    ///
    /// # Errors
    ///
    /// Returns [`ChangeError::InvalidPick`] for the first part that is not a pick.
    pub fn from_list(text: &str) -> Result<Self, ChangeError> {
        text.split(',')
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .try_fold(Self::new(), Self::add_text)
    }

    /// Returns whether no group is picked.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.every.is_none() && self.groups.is_empty()
    }

    /// Returns the index picked for every group, if any.
    #[must_use]
    pub const fn every_index(&self) -> Option<usize> {
        self.every
    }

    /// Returns the groups picked by name as `(group, index)`, as given.
    #[must_use]
    pub fn groups(&self) -> &[(String, usize)] {
        &self.groups
    }

    /// Returns the same picks with the group names in lower case and sorted, so that picks that
    /// choose the same alternatives compare and hash alike.
    #[must_use]
    pub fn normalised(&self) -> Self {
        let mut groups: Vec<(String, usize)> = self
            .groups
            .iter()
            .map(|(name, index)| (name.to_ascii_lowercase(), *index))
            .collect();
        groups.sort();
        Self {
            every: self.every,
            groups,
        }
    }

    /// Returns the named picks whose group is not among `seen`, the lower-case names of the
    /// groups of the resolved Loadout, as `(group, index)` in the order given.
    pub(crate) fn unseen<'a>(
        &'a self,
        seen: &'a HashSet<String>,
    ) -> impl Iterator<Item = (&'a str, usize)> {
        self.groups
            .iter()
            .filter(|(name, _)| !seen.contains(&name.to_ascii_lowercase()))
            .map(|(name, index)| (name.as_str(), *index))
    }

    /// Returns what the picks say about `group`; a named pick wins over the pick of every group.
    pub(crate) fn choice(&self, group: &str) -> Choice {
        self.groups
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(group))
            .map(|&(_, index)| Choice::Named(index))
            .or_else(|| self.every.map(Choice::Every))
            .unwrap_or(Choice::First)
    }
}

/// Reads an index: ASCII digits only, so no sign, that fits `usize`.
fn parse_index(text: &str) -> Option<usize> {
    let text = text.trim();
    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    text.parse().ok()
}

/// Writes the picks in the form [`LoadoutPicks::from_list`] reads: the index of every group
/// first, then the named picks, separated by commas.
impl fmt::Display for LoadoutPicks {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let every = self.every.map(|index| index.to_string());
        let named = self
            .groups
            .iter()
            .map(|(name, index)| format!("{name}={index}"));
        let parts: Vec<String> = every.into_iter().chain(named).collect();
        f.write_str(&parts.join(", "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_index_is_digits_only_and_must_fit() {
        for text in [
            "Body=+1",
            "+1",
            "Body=-1",
            "Body=99999999999999999999999",
            "Body=1.5",
        ] {
            assert!(LoadoutPicks::new().add_text(text).is_err(), "{text:?}");
        }
        assert_eq!(
            LoadoutPicks::new().add_text("Body = 2"),
            Ok(LoadoutPicks::new().group("Body", 2)),
            "blanks around the name and the index are trimmed"
        );
        assert_eq!(
            LoadoutPicks::new().add_text("Body=99999999999"),
            Ok(LoadoutPicks::new().group("Body", 99_999_999_999))
        );
    }

    #[test]
    fn the_display_form_reads_back_and_normalising_ignores_case_and_order() {
        let picks = LoadoutPicks::new()
            .group("Eggs", 1)
            .group("Body", 2)
            .every(3);

        assert_eq!(picks.to_string(), "3, Eggs=1, Body=2");
        assert_eq!(
            LoadoutPicks::from_list(&picks.to_string()),
            Ok(picks.clone())
        );
        assert_eq!(
            picks.normalised(),
            LoadoutPicks::new()
                .group("BODY", 2)
                .group("eggs", 1)
                .every(3)
                .normalised()
        );
    }

    #[test]
    fn text_is_a_group_pick_or_a_bare_index() {
        let picks = LoadoutPicks::new()
            .add_text("Body=2")
            .and_then(|picks| picks.add_text(" 1 "));

        assert_eq!(
            picks,
            Ok(LoadoutPicks::new().group("Body", 2).every(1)),
            "the group name is trimmed, the bare index is every group"
        );
    }

    #[test]
    fn a_list_is_comma_separated_and_skips_blank_parts() {
        assert_eq!(
            LoadoutPicks::from_list("Body=2, ,Egg=1,"),
            Ok(LoadoutPicks::new().group("Body", 2).group("Egg", 1))
        );
        assert_eq!(LoadoutPicks::from_list("  "), Ok(LoadoutPicks::new()));
        assert!(LoadoutPicks::from_list("Body=2, x").is_err());
    }

    #[test]
    fn malformed_text_is_invalid() {
        for text in ["", "Body=", "=1", "Body=x", "x", "Body=-1", "1=2=3"] {
            assert_eq!(
                LoadoutPicks::new().add_text(text),
                Err(ChangeError::InvalidPick {
                    text: text.to_owned()
                }),
                "{text:?}"
            );
        }
    }

    #[test]
    fn a_named_pick_wins_over_every_and_ignores_case() {
        let picks = LoadoutPicks::new().every(1).group("Body", 2);

        assert_eq!(picks.choice("body"), Choice::Named(2));
        assert_eq!(picks.choice("Eggs"), Choice::Every(1));
        assert_eq!(LoadoutPicks::new().choice("Body"), Choice::First);
    }

    #[test]
    fn a_later_pick_of_a_group_replaces_the_earlier() {
        let picks = LoadoutPicks::new().group("Body", 1).group("BODY", 3);

        assert_eq!(picks.groups(), [("BODY".to_owned(), 3)]);
    }
}
