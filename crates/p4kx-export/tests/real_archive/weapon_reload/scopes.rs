// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet};

use p4kx_extraction::Archive;
use p4kx_extraction::cryxml::{SourceDocument, SourceElement, SourceReadOptions};
use serde_json::Value;

use super::TestResult;
use super::proof;

#[derive(PartialEq, Eq)]
pub(super) struct Declaration {
    pub(super) entry: usize,
    pub(super) position: Value,
}

#[derive(Default)]
pub(super) struct Scope {
    pub(super) complete: bool,
    names: BTreeMap<String, Vec<Declaration>>,
}

impl Scope {
    pub(super) fn named(&self, name: &str) -> &[Declaration] {
        self.names
            .get(&name.to_ascii_lowercase())
            .map_or(&[], Vec::as_slice)
    }
}

pub(super) fn children<'xml>(
    owner: SourceElement<'xml>,
    name: &'static str,
) -> impl Iterator<Item = SourceElement<'xml>> {
    owner
        .elements()
        .filter(move |node| node.name() == name && node.namespace().is_none())
}

pub(super) fn source_attribute<'xml>(
    element: SourceElement<'xml>,
    name: &str,
) -> Option<&'xml str> {
    let mut values = element
        .attributes()
        .filter(|attribute| attribute.name() == name && attribute.namespace().is_none());
    let first = values.next()?;
    values.next().is_none().then_some(first.value())
}

pub(super) fn definition(archive: &Archive, name: Option<&str>) -> TestResult<Scope> {
    let mut scope = Scope {
        complete: name.is_some(),
        ..Scope::default()
    };
    let mut visited = BTreeSet::new();
    let mut pending = name.map(str::to_owned).into_iter().collect::<Vec<_>>();
    while let Some(path) = pending.pop() {
        let Ok(entry) = proof::resolve(archive, &path) else {
            scope.complete = false;
            continue;
        };
        if !visited.insert(entry.index()) {
            continue;
        }
        let source = proof::read_bytes(archive, entry)?;
        let Ok(document) = SourceDocument::parse(&source, SourceReadOptions::default()) else {
            scope.complete = false;
            continue;
        };
        let root = document.root();
        if root.name() != "TagDefinition" || root.namespace().is_some() {
            scope.complete = false;
            continue;
        }
        scope.complete &= source_attribute(root, "version") == Some("2");
        read_declarations(&mut scope, root, entry.index())?;
        for import in children(root, "Imports").flat_map(|list| children(list, "Import")) {
            if let Some(path) = source_attribute(import, "filename") {
                pending.push(path.to_owned());
            } else {
                scope.complete = false;
            }
        }
    }
    Ok(scope)
}

fn read_declarations(scope: &mut Scope, root: SourceElement<'_>, entry: usize) -> TestResult {
    for section in children(root, "Tags") {
        for tag in children(section, "Tag")
            .chain(children(section, "Group").flat_map(|group| children(group, "Tag")))
        {
            let Some(name) =
                source_attribute(tag, "name").filter(|label| !label.is_empty() && label.is_ascii())
            else {
                scope.complete = false;
                continue;
            };
            scope
                .names
                .entry(name.to_ascii_lowercase())
                .or_default()
                .push(Declaration {
                    entry,
                    position: proof::node_location(tag, "$node")?,
                });
        }
    }
    Ok(())
}
