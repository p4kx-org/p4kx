// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

use p4kx_foundation::xml_name;

use crate::element::Attribute;

pub(crate) const DEFAULT_NAMESPACE_ATTRIBUTE: &str = "xmlns";

const PAIRWISE_LIMIT: usize = 16;
const ORDINARY_NAMESPACE_ATTRIBUTE: &str = "_xmlns";
const XML_NAMESPACE: &str = "http://www.w3.org/XML/1998/namespace";
const XMLNS_NAMESPACE: &str = "http://www.w3.org/2000/xmlns/";

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

const fn valid(index: usize, character: char) -> bool {
    if index == 0 {
        xml_name::is_start(character)
    } else {
        xml_name::is_continue(character)
    }
}

/// Reusable scratch space that turns an element's attribute names into distinct XML names.
#[derive(Default)]
pub(crate) struct AttributeNames<'d> {
    names: Vec<ResolvedName<'d>>,
}

pub(crate) struct ResolvedName<'d> {
    pub(crate) name: Cow<'d, str>,
    pub(crate) disambiguated: bool,
    reserved_default: bool,
}

impl ResolvedName<'_> {
    pub(crate) const fn uses_namespace_alias(&self) -> bool {
        self.reserved_default && !self.disambiguated
    }
}

impl<'d> AttributeNames<'d> {
    /// Returns distinct XML names in source order, with reserved default bindings made ordinary.
    pub(crate) fn resolve(
        &mut self,
        attributes: impl Iterator<Item = Attribute<'d>>,
    ) -> &[ResolvedName<'d>] {
        self.names.clear();
        self.names.extend(attributes.map(|attribute| ResolvedName {
            name: sanitized(attribute.name()),
            disambiguated: false,
            reserved_default: attribute.name() == DEFAULT_NAMESPACE_ATTRIBUTE
                && matches!(attribute.value(), XML_NAMESPACE | XMLNS_NAMESPACE),
        }));
        if has_duplicates(&self.names) {
            make_unique(&mut self.names);
        }
        repair_namespace_binding(&mut self.names);
        &self.names
    }
}

fn repair_namespace_binding(names: &mut [ResolvedName<'_>]) {
    let Some(index) = names.iter().position(ResolvedName::uses_namespace_alias) else {
        return;
    };
    let alias = if names
        .iter()
        .any(|name| name.name == ORDINARY_NAMESPACE_ATTRIBUTE)
    {
        let taken: HashSet<_> = names.iter().map(|name| name.name.as_ref()).collect();
        let mut suffix = 2;
        Cow::Owned(free_suffix(
            ORDINARY_NAMESPACE_ATTRIBUTE,
            &mut suffix,
            |candidate| taken.contains(candidate),
        ))
    } else {
        Cow::Borrowed(ORDINARY_NAMESPACE_ATTRIBUTE)
    };
    if let Some(name) = names.get_mut(index) {
        name.name = alias;
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
            let unique = free_suffix(&base, suffix, |candidate| taken.contains(candidate));
            name.name = Cow::Owned(unique);
            name.disambiguated = true;
        }
        taken.insert(name.name.to_string());
    }
}

fn free_suffix(base: &str, suffix: &mut u64, is_taken: impl Fn(&str) -> bool) -> String {
    loop {
        let candidate = format!("{base}_{suffix}");
        *suffix += 1;
        if !is_taken(&candidate) {
            return candidate;
        }
    }
}
