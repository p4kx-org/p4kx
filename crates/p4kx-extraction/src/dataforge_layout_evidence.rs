// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_dataforge::LayoutSource;
use p4kx_dataforge::query::{QueryByteUse, QuerySource};

use crate::query_source_evidence::QuerySourceSite;
use crate::{SourceEvidenceError, SourceLocation, SourceScope, SourceUse, SourceUseRole};

/// Returns the physical declaration sites actually examined for a layout.
pub(crate) fn sites(source: &LayoutSource) -> impl Iterator<Item = QuerySourceSite> {
    let sites = match source {
        LayoutSource::Structure { index, .. } => {
            ["parent", "firstProperty", "propertyCount", "size"]
                .map(|field| Some(structure(*index, field, SourceUseRole::Selector)))
        }
        LayoutSource::Property { index, name, .. } => [
            Some(property(*index, "name", SourceUseRole::Selector)),
            Some(property(*index, "instanceLayout", SourceUseRole::Selector)),
            name.as_ref().and_then(text),
            None,
        ],
        LayoutSource::TypeName { index, name, .. } => {
            let [header, text] = type_name(*index, name.as_ref());
            [header, text, None, None]
        }
        LayoutSource::TypeParent { index, .. } => [
            Some(structure(*index, "parent", SourceUseRole::Selector)),
            None,
            None,
            None,
        ],
        LayoutSource::InlineClassWidth {
            property_index,
            struct_index,
            ..
        } => [
            Some(property(*property_index, "target", SourceUseRole::Selector)),
            Some(structure(*struct_index, "size", SourceUseRole::Selector)),
            None,
            None,
        ],
        LayoutSource::PropertyTarget { property_index, .. } => [
            Some(property(*property_index, "target", SourceUseRole::Selector)),
            None,
            None,
            None,
        ],
        LayoutSource::UnreadablePropertyName { index, .. } => [
            Some(property(*index, "name", SourceUseRole::Attempted)),
            None,
            None,
            None,
        ],
        LayoutSource::UnavailableLayout { index, .. } => [
            Some(structure(*index, "layout", SourceUseRole::Attempted)),
            None,
            None,
            None,
        ],
        _ => [None, None, None, None],
    };
    sites.into_iter().flatten()
}

pub(crate) fn bind(
    source: &LayoutSource,
    scope: &SourceScope,
    selectors: &mut Vec<SourceUse>,
    attempts: &mut Vec<SourceUse>,
) -> Result<(), SourceEvidenceError> {
    for site in sites(source) {
        let usage = site.bind(scope)?;
        if usage.role().contributes() {
            selectors.push(usage);
        } else {
            attempts.push(usage);
        }
    }
    Ok(())
}

pub(crate) fn type_name(index: usize, span: Option<&Range<u64>>) -> [Option<QuerySourceSite>; 2] {
    let role = if span.is_some() {
        SourceUseRole::Selector
    } else {
        SourceUseRole::Attempted
    };
    [Some(structure(index, "name", role)), span.and_then(text)]
}

pub(crate) fn text(span: &Range<u64>) -> Option<QuerySourceSite> {
    QuerySourceSite::of(
        QuerySource::Bytes {
            start: span.start,
            end: span.end,
            kind: QueryByteUse::Text,
        },
        SourceUseRole::Selector,
    )
}

fn structure(index: usize, field: &str, role: SourceUseRole) -> QuerySourceSite {
    QuerySourceSite::structure(index, field, role)
}

fn property(index: usize, field: &str, role: SourceUseRole) -> QuerySourceSite {
    declaration("dataforgePropertyDefinition", index, field, role)
}

fn declaration(namespace: &str, index: usize, field: &str, role: SourceUseRole) -> QuerySourceSite {
    QuerySourceSite {
        location: SourceLocation::Subresource {
            namespace: namespace.into(),
            indices: vec![index as u64],
            field: field.into(),
        },
        role,
    }
}
