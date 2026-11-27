// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceUseRole, SourceUses, SourceValue, SourcedValue};

use crate::CatalogError;
use crate::catalog_read::original::{EntryXml, XmlAttributeSource};
use crate::starmap_xml::{Node, attribute};

#[derive(Debug)]
pub(crate) struct RegistryAttribute {
    text: String,
    origin: RegistryAttributeOrigin,
}

impl RegistryAttribute {
    pub(crate) fn read<'node>(
        xml: &EntryXml<'_, '_, '_>,
        node: impl Node<'node>,
        name: &'static str,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            text: attribute(node, name).unwrap_or_default().to_owned(),
            origin: RegistryAttributeOrigin::read(xml, node, name)?,
        })
    }

    pub(crate) fn text(&self) -> &str {
        &self.text
    }

    pub(crate) fn value<T>(
        &self,
        sources: &SourceUses,
        value: Option<T>,
    ) -> Result<SourcedValue<T>, CatalogError> {
        self.origin.value(sources, value)
    }

    #[cfg(test)]
    pub(crate) const fn fixture(text: String) -> Self {
        Self {
            text,
            origin: RegistryAttributeOrigin::Fixture,
        }
    }
}

#[derive(Debug)]
pub(crate) enum RegistryAttributeOrigin {
    Original {
        attribute: XmlAttributeSource,
        complete: bool,
    },
    #[cfg(test)]
    Fixture,
}

impl RegistryAttributeOrigin {
    pub(crate) const fn matches_node(&self, index: u32) -> bool {
        match self {
            Self::Original { attribute, .. } => attribute.node_index() == index,
            #[cfg(test)]
            Self::Fixture => false,
        }
    }

    pub(crate) fn read<'node>(
        xml: &EntryXml<'_, '_, '_>,
        node: impl Node<'node>,
        name: &'static str,
    ) -> Result<Self, CatalogError> {
        Ok(Self::Original {
            attribute: xml.attribute_source(node, name)?,
            complete: !xml.is_lossy(),
        })
    }

    pub(crate) fn value<T>(
        &self,
        sources: &SourceUses,
        value: Option<T>,
    ) -> Result<SourcedValue<T>, CatalogError> {
        let value = value.map_or(SourceValue::Absent, SourceValue::Present);
        match self {
            Self::Original {
                attribute,
                complete,
            } => {
                let usage = attribute.observe(SourceUseRole::Conversion)?;
                Ok(sources.value(value, &[usage], *complete))
            }
            #[cfg(test)]
            Self::Fixture => Ok(sources.value(value, &[], false)),
        }
    }
}
