// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::OnceLock;

use super::{SourceLocation, Subresource};

const COMMON_FIELDS: &[&str] = &[
    "$name",
    "$node",
    "$text",
    "@pos",
    "@rot",
    "$absent:@pos",
    "$absent:@rot",
    "@RainOcclusion",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Namespace {
    Ordinal,
    Text,
    CryXml,
}

impl Namespace {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Ordinal => "xml",
            Self::Text => "xmlSource.text",
            Self::CryXml => "xmlSource.cryXml",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Indices {
    One([u64; 1]),
    Two([u64; 2]),
}

impl Indices {
    const fn as_slice(&self) -> &[u64] {
        match self {
            Self::One(indices) => indices.as_slice(),
            Self::Two(indices) => indices.as_slice(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Field {
    Static(&'static str),
    Owned(Box<str>),
}

impl Field {
    fn new(field: String) -> Self {
        COMMON_FIELDS
            .iter()
            .copied()
            .find(|name| *name == field)
            .map_or_else(|| Self::Owned(field.into_boxed_str()), Self::Static)
    }

    fn as_str(&self) -> &str {
        match self {
            Self::Static(field) => field,
            Self::Owned(field) => field,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct XmlSite {
    indices: Indices,
    field: Field,
    namespace: Namespace,
}

impl XmlSite {
    fn subresource(&self) -> Subresource<'_> {
        Subresource {
            namespace: self.namespace.as_str(),
            indices: self.indices.as_slice(),
            field: self.field.as_str(),
        }
    }
}

/// An exact XML coordinate with inline positions and a deferred owned view.
#[derive(Debug)]
pub(crate) struct StoredXmlSite {
    site: XmlSite,
    original: OnceLock<Box<SourceLocation>>,
}

impl StoredXmlSite {
    pub(super) fn from_original(location: SourceLocation) -> Result<Self, SourceLocation> {
        let SourceLocation::Subresource {
            namespace,
            indices,
            field,
        } = location
        else {
            return Err(location);
        };
        let Some((kind, positions)) = coordinates(&namespace, &indices, &field) else {
            return Err(SourceLocation::Subresource {
                namespace,
                indices,
                field,
            });
        };
        Ok(Self {
            site: XmlSite {
                indices: positions,
                field: Field::new(field),
                namespace: kind,
            },
            original: OnceLock::new(),
        })
    }

    pub(super) fn subresource(&self) -> Subresource<'_> {
        self.site.subresource()
    }

    pub(super) fn original(&self) -> &SourceLocation {
        self.original
            .get_or_init(|| Box::new(self.subresource().to_owned_location()))
    }

    #[cfg(test)]
    pub(super) fn is_materialized(&self) -> bool {
        self.original.get().is_some()
    }
}

impl Clone for StoredXmlSite {
    fn clone(&self) -> Self {
        Self {
            site: self.site.clone(),
            original: OnceLock::new(),
        }
    }
}

impl PartialEq for StoredXmlSite {
    fn eq(&self, other: &Self) -> bool {
        self.site == other.site
    }
}

impl Eq for StoredXmlSite {}

fn coordinates(namespace: &str, indices: &[u64], field: &str) -> Option<(Namespace, Indices)> {
    let attribute = field.starts_with('@');
    let xml_node = matches!(field, "$node" | "$name");
    match (namespace, indices) {
        ("xml", [node])
            if u32::try_from(*node).is_ok()
                && (field == "$name" || attribute || field.starts_with("$absent:@")) =>
        {
            Some((Namespace::Ordinal, Indices::One([*node])))
        }
        ("xml", [node, ordinal])
            if u32::try_from(*node).is_ok() && (attribute || field == "$text") =>
        {
            Some((Namespace::Ordinal, Indices::Two([*node, *ordinal])))
        }
        ("xmlSource.text", [start, end]) if start <= end && (attribute || xml_node) => {
            Some((Namespace::Text, Indices::Two([*start, *end])))
        }
        ("xmlSource.cryXml", [node]) if u32::try_from(*node).is_ok() && xml_node => {
            Some((Namespace::CryXml, Indices::One([*node])))
        }
        ("xmlSource.cryXml", [node, attribute_index])
            if u32::try_from(*node).is_ok()
                && u32::try_from(*attribute_index).is_ok()
                && attribute =>
        {
            Some((Namespace::CryXml, Indices::Two([*node, *attribute_index])))
        }
        _ => None,
    }
}
