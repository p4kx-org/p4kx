// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Guid, SourceUse, SourceUseRole, SourceUses, SourceValue, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use crate::CatalogError;
use crate::catalog_read::original::EntryXml;
use crate::guid_text::CatalogGuid;
use crate::schema_fields::every_field_required;
use crate::starmap_xml::{Node, Root, attribute};

pub(super) const ENTITY: &str = "Entity";
pub(super) const ENTITY_GUID: &str = "EntityCryGUID";
const ENTITY_CLASS_GUID: &str = "EntityClassGUID";
const HARVESTABLE: &str = "HarvestableComponent";
const SUBCONFIG_GUID: &str = "SubConfigGUID";
const SPHERE: &str = "AreaSphereComponent";
const ENTITIES: &str = "Entities";

/// The identity and original component declarations of one opened entity member.
#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct EntXmlDocument {
    root_guid: SourcedValue<CatalogGuid>,
    class_guid: SourcedValue<CatalogGuid>,
    harvestable_components: SourcedValue<Vec<HarvestableComponent>>,
    area_entities: SourcedValue<Vec<AreaEntity>>,
}

#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct HarvestableComponent {
    node_index: SourcedValue<u32>,
    owner_guid: SourcedValue<CatalogGuid>,
    sub_config_guid: SourcedValue<CatalogGuid>,
    sub_config_text: SourcedValue<String>,
}

#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct AreaEntity {
    node_index: SourcedValue<u32>,
    sphere_node_index: SourcedValue<u32>,
    guid: SourcedValue<CatalogGuid>,
    guid_text: SourcedValue<String>,
}

impl EntXmlDocument {
    pub(super) fn project(xml: &EntryXml<'_, '_, '_>) -> Result<(Self, SourceUse), CatalogError> {
        let fields = Fields {
            xml,
            sources: xml.capture().source_uses(),
        };
        let root = xml.root();
        let root_selector = xml
            .attribute_source(root, ENTITY_GUID)?
            .observe(SourceUseRole::Selector)?;
        let (harvestable_components, area_entities) = fields.children(root)?;
        Ok((
            Self {
                root_guid: fields.guid(root, ENTITY_GUID)?,
                class_guid: fields.guid(root, ENTITY_CLASS_GUID)?,
                harvestable_components,
                area_entities,
            },
            root_selector,
        ))
    }
}

struct Fields<'a, 'read, 'capture, 'input> {
    xml: &'a EntryXml<'read, 'capture, 'input>,
    sources: &'a SourceUses,
}

type LocationChildren = (
    SourcedValue<Vec<HarvestableComponent>>,
    SourcedValue<Vec<AreaEntity>>,
);

impl Fields<'_, '_, '_, '_> {
    fn children(&self, root: Root<'_>) -> Result<LocationChildren, CatalogError> {
        let source = self.xml.node_source(root)?.selected()?;
        let mut components = Vec::new();
        let mut areas = Vec::new();
        let mut component_uses = vec![source.clone()];
        let mut area_uses = vec![source];
        for child in root.children() {
            if child.name() == HARVESTABLE {
                component_uses.push(self.xml.node_source(child)?.selected()?);
                components.push(HarvestableComponent {
                    node_index: self.node_index(child)?,
                    owner_guid: self.owner_guid(root, child)?,
                    sub_config_guid: self.guid(child, SUBCONFIG_GUID)?,
                    sub_config_text: self.text(child, SUBCONFIG_GUID)?,
                });
            }
            if child.name() == SPHERE {
                self.area_entities(child, &mut areas, &mut area_uses)?;
            }
        }
        Ok((
            self.sources.value(
                SourceValue::Present(components),
                &component_uses,
                self.complete(),
            ),
            self.sources
                .value(SourceValue::Present(areas), &area_uses, self.complete()),
        ))
    }

    fn owner_guid(
        &self,
        root: Root<'_>,
        component: Root<'_>,
    ) -> Result<SourcedValue<CatalogGuid>, CatalogError> {
        let value = guid_state(attribute(root, ENTITY_GUID));
        let owner = self.xml.attribute_source(root, ENTITY_GUID)?;
        let relationship = self.xml.node_source(component)?.selected()?;
        Ok(self.sources.value(
            value,
            &[owner.observe(SourceUseRole::Conversion)?, relationship],
            self.complete(),
        ))
    }

    fn area_entities(
        &self,
        sphere: Root<'_>,
        output: &mut Vec<AreaEntity>,
        selection: &mut Vec<SourceUse>,
    ) -> Result<(), CatalogError> {
        selection.push(self.xml.node_source(sphere)?.selected()?);
        for list in sphere.children().filter(|node| node.name() == ENTITIES) {
            selection.push(self.xml.node_source(list)?.selected()?);
            for entity in list.children().filter(|node| node.name() == ENTITY) {
                selection.push(self.xml.node_source(entity)?.selected()?);
                output.push(AreaEntity {
                    node_index: self.node_index(entity)?,
                    sphere_node_index: self.sphere_owner(sphere, list, entity)?,
                    guid: self.guid(entity, ENTITY_GUID)?,
                    guid_text: self.text(entity, ENTITY_GUID)?,
                });
            }
        }
        Ok(())
    }

    fn sphere_owner(
        &self,
        sphere: Root<'_>,
        list: Root<'_>,
        entity: Root<'_>,
    ) -> Result<SourcedValue<u32>, CatalogError> {
        let parent = self.xml.node_source(sphere)?;
        Ok(self.sources.value(
            SourceValue::Present(parent.index()),
            &[
                parent.selected()?,
                self.xml.node_source(list)?.selected()?,
                self.xml.node_source(entity)?.selected()?,
            ],
            self.complete(),
        ))
    }

    fn node_index(&self, node: Root<'_>) -> Result<SourcedValue<u32>, CatalogError> {
        let source = self.xml.node_source(node)?;
        Ok(self.sources.value(
            SourceValue::Present(source.index()),
            &[source.selected()?],
            self.complete(),
        ))
    }

    fn guid(
        &self,
        node: Root<'_>,
        field: &'static str,
    ) -> Result<SourcedValue<CatalogGuid>, CatalogError> {
        let value = guid_state(attribute(node, field));
        let source = self.xml.attribute_source(node, field)?;
        Ok(self.sources.value(
            value,
            &[source.observe(SourceUseRole::Conversion)?],
            self.complete(),
        ))
    }

    fn text(
        &self,
        node: Root<'_>,
        field: &'static str,
    ) -> Result<SourcedValue<String>, CatalogError> {
        let value = attribute(node, field).map_or(SourceValue::Absent, |text| {
            SourceValue::Present(text.to_owned())
        });
        let source = self.xml.attribute_source(node, field)?;
        Ok(self.sources.value(
            value,
            &[source.observe(SourceUseRole::Payload)?],
            self.complete(),
        ))
    }

    const fn complete(&self) -> bool {
        !self.xml.is_lossy()
    }
}

fn guid_state(raw: Option<&str>) -> SourceValue<CatalogGuid> {
    raw.map_or(SourceValue::Absent, |text| {
        parse_guid(text).map_or(SourceValue::Invalid, |guid| {
            SourceValue::Present(CatalogGuid(guid))
        })
    })
}

pub(super) fn parse_guid(text: &str) -> Option<Guid> {
    let guid = text.trim().parse().ok()?;
    (guid != Guid::default()).then_some(guid)
}
