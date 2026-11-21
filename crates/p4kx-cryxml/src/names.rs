// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

use crate::element::Attribute;

const PAIRWISE_LIMIT: usize = 16;

/// Returns `name` as an XML name without namespace prefix: every character a name cannot hold,
/// `:` included, becomes `_`, and an empty name becomes `_`.
pub(crate) fn sanitized(name: &str) -> Cow<'_, str> {
    if name.is_empty() {
        return Cow::Borrowed("_");
    }
    if name
        .char_indices()
        .all(|(index, character)| valid(index, character))
    {
        return Cow::Borrowed(name);
    }
    name.char_indices()
        .map(|(index, character)| {
            if valid(index, character) {
                character
            } else {
                '_'
            }
        })
        .collect()
}

fn valid(index: usize, character: char) -> bool {
    character.is_alphabetic()
        || character == '_'
        || (index > 0 && (character.is_alphanumeric() || matches!(character, '-' | '.')))
}

/// Reusable scratch space that turns an element's attribute names into distinct XML names.
#[derive(Default)]
pub(crate) struct AttributeNames<'d> {
    names: Vec<ResolvedName<'d>>,
}

pub(crate) struct ResolvedName<'d> {
    pub(crate) name: Cow<'d, str>,
    pub(crate) disambiguated: bool,
}

impl<'d> AttributeNames<'d> {
    /// Returns the sanitised names of `attributes` in order; a name that repeats an earlier one
    /// gets the suffix `_2`, `_3`, … , the first that is not taken.
    pub(crate) fn resolve(
        &mut self,
        attributes: impl Iterator<Item = Attribute<'d>>,
    ) -> &[ResolvedName<'d>] {
        self.names.clear();
        self.names.extend(attributes.map(|attribute| ResolvedName {
            name: sanitized(attribute.name()),
            disambiguated: false,
        }));
        if has_duplicates(&self.names) {
            make_unique(&mut self.names);
        }
        &self.names
    }
}

fn has_duplicates(names: &[ResolvedName<'_>]) -> bool {
    if names.len() <= PAIRWISE_LIMIT {
        return names.iter().enumerate().any(|(index, name)| {
            names
                .iter()
                .skip(index + 1)
                .any(|other| other.name == name.name)
        });
    }
    let mut seen = HashSet::with_capacity(names.len());
    !names.iter().all(|name| seen.insert(&*name.name))
}

fn make_unique(names: &mut [ResolvedName<'_>]) {
    let mut taken: HashSet<String> = HashSet::with_capacity(names.len());
    let mut next_suffix: HashMap<String, u64> = HashMap::new();
    for name in names.iter_mut() {
        if taken.contains(&*name.name) {
            let base = name.name.to_string();
            let suffix = next_suffix.entry(base.clone()).or_insert(2);
            let unique = loop {
                let candidate = format!("{base}_{suffix}");
                *suffix += 1;
                if !taken.contains(&candidate) {
                    break candidate;
                }
            };
            name.name = Cow::Owned(unique);
            name.disambiguated = true;
        }
        taken.insert(name.name.to_string());
    }
}
