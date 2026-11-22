// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

#[cfg(test)]
use std::convert::Infallible;

use p4kx_extraction::{CancelToken, FieldOrigin, Guid, SourceUse};

use crate::catalog_query_session::checkpoint;
use crate::catalog_read::original::{AuthoredEntryPath, EntryCapture, EntryRead, EntryXml};
use crate::{CatalogError, ExportAccountingError};

#[cfg(test)]
use crate::starmap_xml::Document;
use crate::starmap_xml::{Node, XmlError};

#[path = "loot_entity_container_document.rs"]
pub(crate) mod documents;

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

/// An entity link paired privately to its selected original attributes.
pub(crate) struct CapturedEntityLink {
    value: EntityLink,
    selected: Option<AuthoredEntryPath>,
    selected_config: Option<AuthoredEntryPath>,
    has_container_attribute: bool,
    survey: Option<SourceUse>,
    original: Option<documents::CapturedDocument>,
    original_complete: bool,
}

impl CapturedEntityLink {
    pub(crate) const fn value(&self) -> &EntityLink {
        &self.value
    }

    pub(crate) const fn original_complete(&self) -> bool {
        self.original_complete
    }

    pub(crate) const fn selected_container(&self) -> Option<&AuthoredEntryPath> {
        self.selected.as_ref()
    }

    pub(crate) const fn take_original(&mut self) -> Option<documents::CapturedDocument> {
        self.original.take()
    }

    pub(crate) fn placement_survey(&self) -> Result<&SourceUse, ExportAccountingError> {
        self.survey
            .as_ref()
            .ok_or(ExportAccountingError::UnboundHealthSubject)
    }

    pub(crate) fn config_origin(
        &self,
        capture: &EntryCapture<'_>,
    ) -> Result<FieldOrigin, CatalogError> {
        let original = self
            .selected_config
            .as_ref()
            .ok_or(ExportAccountingError::UnboundHealthSubject)?
            .retain_in(capture)?;
        let (_, mut inputs, complete) = original.into_parts();
        inputs.push(self.placement_survey()?.clone());
        Ok(capture
            .source_uses()
            .origin(&inputs, complete && self.original_complete))
    }
}

/// Reads configured entity links from an independent plain XML projection.
#[cfg(test)]
pub(crate) fn read(bytes: &[u8]) -> Result<EntityLink, XmlError> {
    let document = Document::parse(bytes)?;
    let result = link_below(document.root(), || Ok(()), |_, _| Ok::<_, Infallible>(None));
    Ok(match result {
        Ok(link) => link.value,
        Err(never) => match never {},
    })
}

/// Reads entity links in the same parse as their actual admitted source token.
pub(crate) fn read_original(
    read: &EntryRead,
    capture: &EntryCapture<'_>,
    cancel: Option<&CancelToken>,
) -> Result<Result<CapturedEntityLink, XmlError>, CatalogError> {
    let document = match EntryXml::parse(read, capture)? {
        Ok(document) => document,
        Err(error) => return Ok(Err(error)),
    };
    Ok(Ok(read_document(&document, cancel)?))
}

fn read_document(
    document: &EntryXml<'_, '_, '_>,
    cancel: Option<&CancelToken>,
) -> Result<CapturedEntityLink, CatalogError> {
    let mut link = link_below(
        document.root(),
        || checkpoint(cancel),
        |node, ordinal| document.attribute_at(node, ordinal),
    )?;
    if (link.value.container.is_some() && link.selected.is_none())
        || (link.value.config.is_some() && link.selected_config.is_none())
    {
        return Err(ExportAccountingError::UnboundHealthSubject.into());
    }
    if link.has_container_attribute {
        link.original = documents::capture(document, cancel)?;
    }
    if link.value.config.is_some() {
        link.survey = Some(document.document_source()?);
    }
    link.original_complete = !document.is_lossy();
    Ok(link)
}

fn container_attribute<'document>(node: impl Node<'document>) -> Option<(usize, &'document str)> {
    node.attributes()
        .enumerate()
        .find_map(|(ordinal, (name, path))| {
            name.eq_ignore_ascii_case(CONTAINER_ATTRIBUTE)
                .then_some((ordinal, path))
        })
}

fn link_below<'document, N: Node<'document>, E>(
    root: N,
    mut checkpoint: impl FnMut() -> Result<(), E>,
    mut capture_attribute: impl FnMut(N, usize) -> Result<Option<AuthoredEntryPath>, E>,
) -> Result<CapturedEntityLink, E> {
    let mut link = CapturedEntityLink {
        value: EntityLink::default(),
        selected: None,
        selected_config: None,
        has_container_attribute: false,
        survey: None,
        original: None,
        original_complete: true,
    };
    let mut pending = vec![root];
    while let Some(node) = pending.pop() {
        checkpoint()?;
        if let Some((ordinal, path)) = container_attribute(node) {
            link.has_container_attribute = true;
            if !path.is_empty() {
                match link.value.container.as_deref() {
                    None => {
                        link.value.container = Some(path.to_owned());
                        link.selected = capture_attribute(node, ordinal)?;
                    }
                    Some(first) => link.value.several |= first != path,
                }
            }
        }
        read_configs(&mut link, node, |ordinal| capture_attribute(node, ordinal))?;
        let mark = pending.len();
        pending.extend(node.children());
        if let Some(added) = pending.get_mut(mark..) {
            added.reverse();
        }
    }
    Ok(link)
}

fn read_configs<'document, N: Node<'document>, E>(
    link: &mut CapturedEntityLink,
    node: N,
    mut capture_attribute: impl FnMut(usize) -> Result<Option<AuthoredEntryPath>, E>,
) -> Result<(), E> {
    for (ordinal, (name, text)) in node.attributes().enumerate() {
        let text = text.trim();
        if !name.eq_ignore_ascii_case(CONFIG_ATTRIBUTE) || text.is_empty() {
            continue;
        }
        let Ok(guid) = text.parse() else {
            link.value.malformed_config = true;
            continue;
        };
        match link.value.config {
            None => {
                link.value.config = Some(guid);
                link.selected_config = capture_attribute(ordinal)?;
            }
            Some(first) => link.value.several |= first != guid,
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "loot_selector_shape.rs"]
mod selector_shape;

#[cfg(test)]
#[path = "loot_selector_witness.rs"]
mod selector_witness;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancelled_link_scan_stops_before_visiting_the_next_node() -> Result<(), XmlError> {
        let document = Document::parse(
            br#"<Root objectContainer="first.socpak"><Child objectContainer="next.socpak"/></Root>"#,
        )?;
        let cancel = CancelToken::new();
        let mut visited = 0;
        let result = link_below(
            document.root(),
            || checkpoint(Some(&cancel)),
            |_, _| {
                visited += 1;
                cancel.cancel();
                Ok(None)
            },
        );
        assert!(matches!(result, Err(CatalogError::Cancelled)));
        assert_eq!(
            visited, 1,
            "no further authored selector is read after cancellation"
        );
        Ok(())
    }

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
