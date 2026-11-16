// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::model::{
    AuthoredAttribute, AuthoredAttributeKind, MaterialLibrary, RawElement,
};

pub(crate) fn selected_submaterials<Interrupted>(
    library: &MaterialLibrary,
    mut checkpoint: impl FnMut() -> Result<(), Interrupted>,
) -> Result<&[RawElement], Interrupted> {
    let Some((root, ordinal)) = library
        .authored
        .root
        .as_ref()
        .zip(library.authored.submaterial_list_ordinal)
    else {
        return Ok(&[]);
    };
    for child in &root.children {
        checkpoint()?;
        if child.authored.ordinal == Some(ordinal)
            && child.name.eq_ignore_ascii_case("SubMaterials")
        {
            return Ok(&child.children);
        }
    }
    Ok(&[])
}

pub(crate) fn attributes<'a>(
    node: &'a RawElement,
    name: &'a str,
) -> impl Iterator<Item = (usize, &'a AuthoredAttribute)> {
    node.authored
        .attributes
        .iter()
        .enumerate()
        .filter(move |(_, attribute)| {
            attribute.name.eq_ignore_ascii_case(name)
                && matches!(
                    &attribute.kind,
                    AuthoredAttributeKind::Attribute { namespace: None }
                )
        })
}

pub(crate) struct Tokens<'a> {
    remaining: &'a str,
}

impl<'a> Tokens<'a> {
    pub(crate) const fn new(value: &'a str) -> Self {
        Self { remaining: value }
    }
}

impl<'a> Iterator for Tokens<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.remaining.is_empty() {
                return None;
            }
            let next = if self.remaining.starts_with('%') {
                self.remaining.get(1..)?.find('%').map(|offset| offset + 1)
            } else {
                self.remaining.find('%')
            }
            .unwrap_or(self.remaining.len());
            let (token, rest) = self.remaining.split_at(next);
            self.remaining = rest;
            if !token.trim().is_empty() {
                return Some(token);
            }
        }
    }
}
