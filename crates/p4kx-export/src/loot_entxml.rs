// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

#[cfg(test)]
use std::convert::Infallible;

use p4kx_extraction::Guid;

use crate::catalog_read::original::{AuthoredEntryPath, EntryCapture, EntryRead, EntryXml};
use crate::{CatalogError, ExportAccountingError};

#[cfg(test)]
use crate::starmap_xml::Document;
use crate::starmap_xml::{Node, XmlError, attribute};

/// The attribute of an entity component that names the container the entity places.
const CONTAINER_ATTRIBUTE: &str = "objectContainer";

/// The attribute of a harvestable component that names its `SubHarvestableMultiConfigRecord`.
const CONFIG_ATTRIBUTE: &str = "multiConfigRef";

/// What the property file of one entity (`entdata/*.entxml` of a container) says about the
/// container it places and the multi config it carries.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct EntityLink {
    /// The path of the container the entity places, as written; the first in document order.
    pub(crate) container: Option<String>,
    /// The `SubHarvestableMultiConfigRecord` the entity carries; the first in document order.
    pub(crate) config: Option<Guid>,
    /// Whether the file names a second, different container or config that is not used.
    pub(crate) several: bool,
    /// A populated multi-config attribute is not a GUID.
    pub(crate) malformed_config: bool,
}

/// An entity link paired privately to its actual selected authored container token.
pub(crate) struct CapturedEntityLink {
    value: EntityLink,
    selected: Option<AuthoredEntryPath>,
}

impl CapturedEntityLink {
    pub(crate) const fn value(&self) -> &EntityLink {
        &self.value
    }

    pub(crate) fn into_parts(self) -> (EntityLink, Option<AuthoredEntryPath>) {
        (self.value, self.selected)
    }
}

enum AttributeValue<'source> {
    Plain(&'source str),
    Captured(AuthoredEntryPath),
}

impl AttributeValue<'_> {
    fn text(&self) -> &str {
        match self {
            Self::Plain(value) => value,
            Self::Captured(value) => value.as_str(),
        }
    }
    fn receipt(self) -> Option<AuthoredEntryPath> {
        match self {
            Self::Plain(_) => None,
            Self::Captured(value) => Some(value),
        }
    }
}

/// Reads configured entity links from an independent plain XML projection.
#[cfg(test)]
pub(crate) fn read(bytes: &[u8]) -> Result<EntityLink, XmlError> {
    let document = Document::parse(bytes)?;
    let result = link_below(document.root(), |node, name| {
        Ok::<_, Infallible>(attribute(node, name).map(AttributeValue::Plain))
    });
    Ok(match result {
        Ok(link) => link.value,
        Err(never) => match never {},
    })
}

/// Reads entity links in the same parse as their actual admitted source token.
pub(crate) fn read_original(
    read: &EntryRead,
    capture: &EntryCapture<'_>,
) -> Result<Result<CapturedEntityLink, XmlError>, CatalogError> {
    let document = match EntryXml::parse(read, capture)? {
        Ok(document) => document,
        Err(error) => return Ok(Err(error)),
    };
    let link = link_below(document.root(), |node, name| {
        if name == CONTAINER_ATTRIBUTE {
            document
                .attribute(node, name)
                .map(|value| value.map(AttributeValue::Captured))
        } else {
            Ok(attribute(node, name).map(AttributeValue::Plain))
        }
    })?;
    if link.value.container.is_some() && link.selected.is_none() {
        return Err(ExportAccountingError::UnboundHealthSubject.into());
    }
    Ok(Ok(link))
}

fn link_below<'document, N: Node<'document>, E>(
    root: N,
    mut attribute: impl FnMut(N, &'static str) -> Result<Option<AttributeValue<'document>>, E>,
) -> Result<CapturedEntityLink, E> {
    let mut link = CapturedEntityLink {
        value: EntityLink::default(),
        selected: None,
    };
    let mut pending = vec![root];
    while let Some(node) = pending.pop() {
        if let Some(path) =
            attribute(node, CONTAINER_ATTRIBUTE)?.filter(|path| !path.text().is_empty())
        {
            match link.value.container.as_deref() {
                None => {
                    link.value.container = Some(path.text().to_owned());
                    link.selected = path.receipt();
                }
                Some(first) => link.value.several |= first != path.text(),
            }
        }
        if let Some(text) =
            attribute(node, CONFIG_ATTRIBUTE)?.filter(|text| !text.text().trim().is_empty())
        {
            if let Ok(guid) = text.text().trim().parse() {
                match link.value.config {
                    None => link.value.config = Some(guid),
                    Some(first) => link.value.several |= first != guid,
                }
            } else {
                link.value.malformed_config = true;
            }
        }
        let mark = pending.len();
        pending.extend(node.children());
        if let Some(added) = pending.get_mut(mark..) {
            added.reverse();
        }
    }
    Ok(link)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(bytes: &[u8]) -> EntityLink {
        super::read(bytes).expect("valid source XML")
    }

    #[test]
    fn malformed_source_is_distinct_from_a_valid_empty_projection() {
        assert!(
            super::read(b"not XML").is_err(),
            "malformed source is reported"
        );
        assert_eq!(
            super::read(b"<Entity/>"),
            Ok(EntityLink::default()),
            "empty source is valid"
        );
    }

    const CONFIG: &str = "687af3db-4c0d-4e8d-9261-102274f8b6ed";
    const OTHER: &str = "11111111-4c0d-4e8d-9261-102274f8b6ed";

    fn guid(text: &str) -> Guid {
        text.parse().expect("a GUID")
    }

    #[test]
    fn a_harvestable_container_names_both() {
        let xml = format!(
            r#"<Entity Name="loot"><PropertiesDataCore>
            <EntityComponentObjectContainer objectContainer="ObjectContainers/PU/loot.socpak"/>
            <HarvestableComponent><looseSubConfigBase multiConfigRef="{CONFIG}"/></HarvestableComponent>
            </PropertiesDataCore></Entity>"#
        );
        assert_eq!(
            read(xml.as_bytes()),
            EntityLink {
                container: Some("ObjectContainers/PU/loot.socpak".to_owned()),
                config: Some(guid(CONFIG)),
                several: false,
                malformed_config: false,
            }
        );
    }

    #[test]
    fn a_plain_container_names_only_the_container() {
        let xml =
            r#"<Entity><EntityComponentObjectContainer objectContainer="a/b.socpak"/></Entity>"#;
        let link = read(xml.as_bytes());
        assert_eq!(link.container.as_deref(), Some("a/b.socpak"));
        assert_eq!(link.config, None);
    }

    #[test]
    fn a_file_with_neither_attribute_is_the_empty_link() {
        let empty = EntityLink::default();
        assert_eq!(read(b"<Entity Name=\"x\"/>"), empty);
        assert!(
            super::read(b"not xml but objectContainer").is_err(),
            "invalid XML is reported"
        );
        assert!(
            super::read(b"").is_err(),
            "empty bytes are not an XML document"
        );
    }

    #[test]
    fn a_config_that_is_no_guid_is_reported_without_losing_other_links() {
        let link = read(br#"<Entity><a multiConfigRef="nope"/></Entity>"#);
        assert_eq!(link.config, None, "no invented GUID");
        assert!(
            link.malformed_config,
            "populated invalid source is distinguished from absence"
        );
    }

    #[test]
    fn the_first_of_several_different_links_in_document_order_is_used() {
        let xml = format!(
            r#"<Entity><a objectContainer="first.socpak"><b multiConfigRef="{CONFIG}"/></a>
            <c objectContainer="second.socpak" multiConfigRef="{OTHER}"/></Entity>"#
        );
        let link = read(xml.as_bytes());
        assert_eq!(link.container.as_deref(), Some("first.socpak"));
        assert_eq!(link.config, Some(guid(CONFIG)));
        assert!(link.several);
    }

    #[test]
    fn the_same_link_written_twice_is_not_several() {
        let xml =
            r#"<Entity><a objectContainer="x.socpak"/><b objectContainer="x.socpak"/></Entity>"#;
        assert!(!read(xml.as_bytes()).several);
    }
}
