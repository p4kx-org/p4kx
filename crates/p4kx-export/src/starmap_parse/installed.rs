// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Archive;

use super::children::{child_lists, external};
use super::{Body, Bounds, CHILD, CHILDREN, DescriptorScope, parse};
use crate::CatalogError;
use crate::catalog_read::original::{AuthoredEntryPath, EntryCapture, EntryXml};
use crate::starmap_index::Index;
use crate::starmap_xml::{Node, Root};

#[path = "selector_scope.rs"]
mod selector_scope;

pub(crate) fn read_jump_witness_body(
    archive: &Archive,
    index: &Index<'_>,
    entry: usize,
    capture: &EntryCapture<'_>,
) -> Result<Body, CatalogError> {
    let bounds = Bounds {
        cancel: None,
        children: None,
        scope: DescriptorScope::catalog(true),
        directory: capture
            .source_uses()
            .observe_root_directory_paths(archive)?,
    };
    parse(archive, index, entry, &bounds, capture).map(|(body, _)| body)
}

pub(crate) fn is_child_list(node: Root<'_>) -> bool {
    node.name().eq_ignore_ascii_case(CHILDREN)
}

pub(crate) fn visit_original_children<'xml>(
    xml: &'xml EntryXml<'_, '_, '_>,
    mut visit: impl FnMut(Root<'xml>, Option<AuthoredEntryPath>) -> Result<(), CatalogError>,
) -> Result<(), CatalogError> {
    let mut pending = vec![xml.root()];
    while let Some(parent) = pending.pop() {
        for list in child_lists(parent) {
            for child in list
                .children()
                .filter(|node| node.name().eq_ignore_ascii_case(CHILD))
            {
                let name = xml.name(child)?;
                visit(child, external(child, &name, xml)?)?;
                pending.push(child);
            }
        }
    }
    Ok(())
}
