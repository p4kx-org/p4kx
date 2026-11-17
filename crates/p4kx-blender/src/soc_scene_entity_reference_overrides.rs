// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{SocContainerOccurrence, SocSourceGuid};
use p4kx_extraction::cryxml::{SourceAttribute, SourceElement};
use p4kx_extraction::{CancelToken, SourceUses, SourceValue, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::{Member, companion_xml};
use crate::soc_scene::SocSceneError;
use crate::soc_scene_classes;
use crate::soc_scene_entities::EntityTarget;
use crate::soc_scene_origin::{SocXmlPosition, XmlOrigin};
use crate::soc_scene_stream::IndexedStream;
use crate::soc_scene_xml::XmlDocuments;

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct EntityReferenceOverrideDocument {
    xml: usize,
    entities: Vec<EntityReferenceOverrideEntity>,
}

impl EntityReferenceOverrideDocument {
    pub(super) fn read(
        occurrence: &SocContainerOccurrence,
        sources: &SourceUses,
        index: &IndexedStream<'_>,
        documents: &mut XmlDocuments<'_>,
        cancel: &CancelToken,
    ) -> Result<Vec<Member<Self>>, SocSceneError> {
        let mut members = Vec::new();
        members.try_reserve_exact(occurrence.entity_reference_overrides.len())?;
        for member in &occurrence.entity_reference_overrides {
            soc_scene_classes::check(cancel)?;
            members.push(Member::read_physical(member, sources, |bytes, origin| {
                companion_xml::read(
                    bytes,
                    origin,
                    "EntityComponentEntityReferenceOverride",
                    documents,
                    cancel,
                    |xml, document, origin| {
                        let entities = children(document.root(), "Entity")
                            .map(|(ordinal, element)| {
                                EntityReferenceOverrideEntity::read(
                                    ordinal, element, index, origin, cancel,
                                )
                            })
                            .collect::<Result<_, _>>()?;
                        Ok(Self { xml, entities })
                    },
                )
            })?);
        }
        Ok(members)
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct EntityReferenceOverrideEntity {
    ordinal: usize,
    position: SocXmlPosition,
    cry_guid: SourcedValue<String>,
    owner: SourcedValue<EntityTarget>,
    auto_spawn_overrides: Vec<AutoSpawnOverrideGroup>,
}

impl EntityReferenceOverrideEntity {
    fn read(
        ordinal: usize,
        element: SourceElement<'_>,
        index: &IndexedStream<'_>,
        origin: &XmlOrigin<'_>,
        cancel: &CancelToken,
    ) -> Result<Self, SocSceneError> {
        soc_scene_classes::check(cancel)?;
        let guid_attributes = || attributes(element, "CryGUID");
        Ok(Self {
            ordinal,
            position: SocXmlPosition::element(element.position())?,
            cry_guid: literal(element, "CryGUID", origin)?,
            owner: EntityTarget::read_guid(
                element,
                guid_attributes(),
                SocSourceGuid::read(guid_attributes()),
                index,
                origin,
            )?,
            auto_spawn_overrides: children(element, "AutoSpawnOverrides")
                .map(|(ordinal, child)| {
                    AutoSpawnOverrideGroup::read(ordinal, child, origin, cancel)
                })
                .collect::<Result<_, _>>()?,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct AutoSpawnOverrideGroup {
    ordinal: usize,
    position: SocXmlPosition,
    elements: Vec<AutoSpawnOverrideElement>,
}

impl AutoSpawnOverrideGroup {
    fn read(
        ordinal: usize,
        element: SourceElement<'_>,
        origin: &XmlOrigin<'_>,
        cancel: &CancelToken,
    ) -> Result<Self, SocSceneError> {
        soc_scene_classes::check(cancel)?;
        Ok(Self {
            ordinal,
            position: SocXmlPosition::element(element.position())?,
            elements: children(element, "AutoSpawnOverrideElement")
                .map(|(ordinal, child)| {
                    AutoSpawnOverrideElement::read(ordinal, child, origin, cancel)
                })
                .collect::<Result<_, _>>()?,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct AutoSpawnOverrideElement {
    ordinal: usize,
    position: SocXmlPosition,
    auto_spawn_idx: SourcedValue<String>,
    target: SourcedValue<String>,
    aa: SourcedValue<String>,
}

impl AutoSpawnOverrideElement {
    fn read(
        ordinal: usize,
        element: SourceElement<'_>,
        origin: &XmlOrigin<'_>,
        cancel: &CancelToken,
    ) -> Result<Self, SocSceneError> {
        soc_scene_classes::check(cancel)?;
        Ok(Self {
            ordinal,
            position: SocXmlPosition::element(element.position())?,
            auto_spawn_idx: literal(element, "AutoSpawnIdx", origin)?,
            target: literal(element, "Target", origin)?,
            aa: literal(element, "AA", origin)?,
        })
    }
}

fn children<'document>(
    element: SourceElement<'document>,
    name: &'static str,
) -> impl Iterator<Item = (usize, SourceElement<'document>)> {
    element
        .elements()
        .enumerate()
        .filter(move |(_, child)| child.namespace().is_none() && child.name() == name)
}

fn attributes<'document>(
    element: SourceElement<'document>,
    name: &'static str,
) -> impl Iterator<Item = SourceAttribute<'document>> {
    element
        .attributes()
        .filter(move |attribute| attribute.namespace().is_none() && attribute.name() == name)
}

fn literal(
    element: SourceElement<'_>,
    name: &'static str,
    origin: &XmlOrigin<'_>,
) -> Result<SourcedValue<String>, SocSceneError> {
    let mut values = attributes(element, name);
    let value = match (values.next(), values.next()) {
        (None, _) => SourceValue::Absent,
        (Some(attribute), None) => SourceValue::Present(attribute.value().to_owned()),
        _ => SourceValue::Invalid,
    };
    origin.field(element, attributes(element, name), value)
}
