// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{SocEntitySite, SocEntityTarget};
use p4kx_extraction::{PhysicalSourceRef, SourceUseRole, SourceValue, SourcedValue};
use p4kx_model::{SocEntity, SocEntityComponent, SocEntityComponentKind, SocEntityLink};
use schemars::JsonSchema;
use serde::Serialize;

use crate::soc_scene::SocSceneError;
use crate::soc_scene_class_mannequin::{children, string_attribute};
use crate::soc_scene_classes::ClassTarget;
use crate::soc_scene_lights::{Light, LightGroup};
use crate::soc_scene_loadouts::Loadout;
use crate::soc_scene_origin::{SocXmlPosition, XmlOrigin};
use crate::soc_scene_rooms::{Area, guid_value, position};
use crate::soc_scene_stream::IndexedStream;
use crate::soc_scene_tile_ports::TileAttachment;

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Entity {
    ordinal: usize,
    position: SocXmlPosition,
    guid: SourcedValue<String>,
    class_guid: SourcedValue<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    class: Option<SourcedValue<ClassTarget>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    class_sensor: Option<SourcedValue<bool>>,
    local_transform: SourcedValue<[[f32; 4]; 3]>,
    areas: Vec<Area>,
    components: Vec<Component>,
    links: Vec<Link>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    lights: Vec<Light>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    light_groups: Vec<LightGroup>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    tile_attachments: Vec<TileAttachment>,
}

impl Entity {
    pub(crate) fn read(
        entity: SocEntity<'_>,
        index: &IndexedStream<'_>,
        origin: &XmlOrigin<'_>,
    ) -> Result<Self, SocSceneError> {
        let element = entity.element();
        Ok(Self {
            ordinal: entity.ordinal(),
            position: position(element)?,
            guid: origin.field(
                element,
                entity.guid_attributes(),
                guid_value(entity.guid_attributes()),
            )?,
            class_guid: origin.field(
                element,
                entity.class_guid_attributes(),
                guid_value(entity.class_guid_attributes()),
            )?,
            class: index.classes.target(entity, origin)?,
            class_sensor: index.classes.sensor(entity, origin)?,
            local_transform: entity_transform(entity, origin)?,
            areas: entity
                .area_components()
                .map(|area| Area::read(area, index.room, origin))
                .collect::<Result<_, _>>()?,
            components: entity
                .authored_components()
                .map(|component| Component::read(component, origin))
                .collect::<Result<_, _>>()?,
            links: entity
                .entity_links()
                .flat_map(|group| group.links().map(move |link| (group.ordinal(), link)))
                .map(|(group, link)| Link::read(group, link, index, origin))
                .collect::<Result<_, _>>()?,
            lights: entity
                .authored_lights()
                .map(|light| Light::read(light, origin))
                .collect::<Result<_, _>>()?,
            light_groups: entity
                .authored_light_groups()
                .map(|group| LightGroup::read(group, origin))
                .collect::<Result<_, _>>()?,
            tile_attachments: entity
                .tile_attachments()
                .map(|attachment| TileAttachment::read(attachment, index.tile_ports, origin))
                .collect::<Result<_, _>>()?,
        })
    }
}

fn entity_transform(
    entity: SocEntity<'_>,
    origin: &XmlOrigin<'_>,
) -> Result<SourcedValue<[[f32; 4]; 3]>, SocSceneError> {
    let element = entity.element();
    let is_transform = |attribute: &p4kx_extraction::cryxml::SourceAttribute<'_>| {
        attribute.namespace().is_none() && matches!(attribute.name(), "Pos" | "Rotate" | "Scale")
    };
    let value = if element
        .attributes()
        .any(|attribute| is_transform(&attribute))
    {
        entity
            .local_transform()
            .map_or(SourceValue::Invalid, |transform| {
                SourceValue::Present(transform.rows)
            })
    } else {
        SourceValue::Absent
    };
    origin.field(element, element.attributes().filter(is_transform), value)
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct Component {
    ordinal: (Option<usize>, usize),
    position: SocXmlPosition,
    kind: ComponentKind,
    declared_type: SourcedValue<String>,
    geometry_paths: Vec<SourcedValue<String>>,
    loadouts: Vec<Loadout>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
enum ComponentKind {
    Door,
    ProximitySensor,
    ProximitySensorDeclaration,
    InteractionLinkController,
    GeometryResource,
    RoomConnector,
    DefaultLoadout,
    Other,
}

impl Component {
    fn read(
        component: SocEntityComponent<'_>,
        origin: &XmlOrigin<'_>,
    ) -> Result<Self, SocSceneError> {
        let element = component.element();
        let kind = match component.kind() {
            SocEntityComponentKind::Door => ComponentKind::Door,
            SocEntityComponentKind::ProximitySensorDeclaration
                if component.is_proximity_sensor() =>
            {
                ComponentKind::ProximitySensor
            }
            SocEntityComponentKind::ProximitySensorDeclaration => {
                ComponentKind::ProximitySensorDeclaration
            }
            SocEntityComponentKind::InteractionLinkController => {
                ComponentKind::InteractionLinkController
            }
            SocEntityComponentKind::GeometryResource => ComponentKind::GeometryResource,
            SocEntityComponentKind::RoomConnector => ComponentKind::RoomConnector,
            SocEntityComponentKind::DefaultLoadout => ComponentKind::DefaultLoadout,
            _ => ComponentKind::Other,
        };
        let geometry_paths = if component.kind() == SocEntityComponentKind::GeometryResource {
            children(element, "Geometry")
                .flat_map(|outer| children(outer, "Geometry"))
                .flat_map(|data| children(data, "Geometry"))
                .map(|geometry| string_attribute(geometry, "path", origin))
                .collect::<Result<_, _>>()?
        } else {
            Vec::new()
        };
        Ok(Self {
            ordinal: component.ordinal(),
            position: position(element)?,
            kind,
            declared_type: origin
                .named(element, "__type", component.declared_type())?
                .map(str::to_owned),
            geometry_paths,
            loadouts: Loadout::read_all(component.loadouts(), origin)?,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct Link {
    ordinal: (usize, usize),
    position: SocXmlPosition,
    name: SourcedValue<String>,
    target_guid: SourcedValue<String>,
    target: SourcedValue<EntityTarget>,
}

impl Link {
    fn read(
        group: usize,
        link: SocEntityLink<'_>,
        index: &IndexedStream<'_>,
        origin: &XmlOrigin<'_>,
    ) -> Result<Self, SocSceneError> {
        let element = link.element();
        Ok(Self {
            ordinal: (group, link.ordinal()),
            position: position(element)?,
            name: origin
                .named(element, "Name", link.name())?
                .map(str::to_owned),
            target_guid: origin.field(
                element,
                link.target_attributes(),
                guid_value(link.target_attributes()),
            )?,
            target: entity_target(link, index, origin)?,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
enum EntityTarget {
    Unmatched,
    Matched { site: EntitySite },
    Ambiguous { sites: Vec<EntitySite> },
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct EntitySite {
    stream: PhysicalSourceRef,
    chunk: usize,
    ordinal: usize,
    position: SocXmlPosition,
}

fn entity_target(
    link: SocEntityLink<'_>,
    index: &IndexedStream<'_>,
    origin: &XmlOrigin<'_>,
) -> Result<SourcedValue<EntityTarget>, SocSceneError> {
    use SourceUseRole::{Payload, Selector};
    let mut uses = vec![origin.node(link.element(), Selector)?];
    for attribute in link.target_attributes() {
        uses.push(origin.attribute(&attribute, Selector)?.1);
    }
    let mut add_target = |site: &SocEntitySite| {
        let input = index
            .admitted
            .get(&site.stream_index)
            .ok_or(SocSceneError::Position)?;
        let scope = XmlOrigin::new(origin.sources, input, Some(site.chunk_index))?;
        let position = SocXmlPosition::element(site.position.clone())?;
        uses.push(scope.at(&position, "$entityOwner", Payload)?);
        Ok::<_, SocSceneError>(EntitySite {
            stream: scope.source(),
            chunk: site.chunk_index,
            ordinal: site.entity_ordinal,
            position,
        })
    };
    let value = match index.index.target(link) {
        SocEntityTarget::Absent => SourceValue::Absent,
        SocEntityTarget::Invalid => SourceValue::Invalid,
        SocEntityTarget::Unmatched { .. } => SourceValue::Present(EntityTarget::Unmatched),
        SocEntityTarget::Matched { site, .. } => SourceValue::Present(EntityTarget::Matched {
            site: add_target(site)?,
        }),
        SocEntityTarget::Ambiguous { sites, .. } => SourceValue::Present(EntityTarget::Ambiguous {
            sites: sites.iter().map(add_target).collect::<Result<_, _>>()?,
        }),
        _ => SourceValue::Unsupported,
    };
    let complete = index.complete || !matches!(value, SourceValue::Present(_));
    Ok(origin.sources.value(value, &uses, complete))
}
