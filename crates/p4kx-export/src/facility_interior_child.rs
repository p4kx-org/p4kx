// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::model::Transform;
use p4kx_extraction::{CancelToken, SourceUse, SourceUses, SourceValue, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::routes;
use super::types::{InteriorFindingKind, SocAttribute};
use crate::catalog_read::original::{AuthoredEntryPath, EntryCapture, EntryXml, PhysicalEntryRef};
use crate::schema_fields::every_field_required;
use crate::starmap_xml::{self, Node};
use crate::{CatalogError, ExportAccountingError};

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct InteriorChild {
    position: SourcedValue<InteriorChildPosition>,
    name: SourcedValue<String>,
    local_transform: SourcedValue<[[f32; 4]; 3]>,
    attributes: Vec<SocAttribute>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct InteriorChildPosition {
    descriptor: PhysicalEntryRef,
    descriptor_node_index: u32,
}

pub(super) struct ChildDeclaration {
    descriptor_node_index: u32,
    selection: SourceUse,
    attributes: Vec<ChildAttribute>,
    transform: Option<Transform>,
}

struct ChildAttribute {
    name: String,
    value: AuthoredEntryPath,
}

pub(super) fn declarations(
    xml: &EntryXml<'_, '_, '_>,
    cancel: Option<&CancelToken>,
) -> Result<Vec<ChildDeclaration>, CatalogError> {
    let mut declarations = Vec::new();
    for list in xml.root().children() {
        routes::check_cancel(cancel)?;
        if !list.name().eq_ignore_ascii_case("ChildObjectContainers") {
            continue;
        }
        for node in list.children() {
            routes::check_cancel(cancel)?;
            if node.name().eq_ignore_ascii_case("Child") {
                declarations.push(ChildDeclaration::read(xml, node)?);
            }
        }
    }
    Ok(declarations)
}

impl ChildDeclaration {
    fn read<'node>(
        xml: &EntryXml<'_, '_, '_>,
        node: impl Node<'node>,
    ) -> Result<Self, CatalogError> {
        let source = xml.node_source(node)?;
        let mut attributes = Vec::new();
        for (ordinal, (name, _)) in node.attributes().enumerate() {
            let value = xml
                .attribute_at(node, ordinal)?
                .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
            attributes.push(ChildAttribute {
                name: name.to_owned(),
                value,
            });
        }
        let transform = transform(node, &attributes);
        Ok(Self {
            descriptor_node_index: source.index(),
            selection: source.selected()?,
            attributes,
            transform,
        })
    }

    pub(super) fn name(&self) -> Result<&AuthoredEntryPath, InteriorFindingKind> {
        let mut names = matching(&self.attributes, "name");
        let Some(name) = names.next() else {
            return Err(InteriorFindingKind::InvalidChildPath);
        };
        if names.next().is_some() {
            return Err(InteriorFindingKind::AmbiguousChildName);
        }
        Ok(&name.value)
    }

    pub(super) const fn transform(&self) -> Option<Transform> {
        self.transform
    }

    pub(super) fn selection_uses(&self) -> Result<Vec<SourceUse>, CatalogError> {
        let mut uses = vec![self.selection.clone()];
        if let Ok(name) = self.name() {
            uses.extend(name.selector_inputs()?.0);
        }
        Ok(uses)
    }

    pub(super) fn transform_uses(
        &self,
        capture: &EntryCapture<'_>,
    ) -> Result<Vec<SourceUse>, CatalogError> {
        let mut uses = vec![self.selection.clone()];
        for attribute in &self.attributes {
            if is_transform_attribute(&attribute.name) {
                let value = attribute.value.retain_in(capture)?;
                uses.extend_from_slice(value.contributions().0);
            }
        }
        Ok(uses)
    }

    pub(super) fn into_source(
        self,
        capture: &EntryCapture<'_>,
        sources: &SourceUses,
        descriptor: PhysicalEntryRef,
    ) -> Result<InteriorChild, CatalogError> {
        let name = match self.name() {
            Ok(name) => name
                .retain_in(capture)?
                .map(|text| text.as_str().to_owned())?
                .into_cell(sources),
            Err(InteriorFindingKind::AmbiguousChildName) => {
                let uses = self.named_attribute_uses(capture)?;
                sources.value(SourceValue::Invalid, &uses, true)
            }
            Err(_) => sources.value(
                SourceValue::Absent,
                std::slice::from_ref(&self.selection),
                true,
            ),
        };
        let position = InteriorChildPosition {
            descriptor,
            descriptor_node_index: self.descriptor_node_index,
        };
        let value = self.transform.map_or(SourceValue::Invalid, |transform| {
            SourceValue::Present(transform.rows)
        });
        let uses = self.transform_uses(capture)?;
        let attributes = self
            .attributes
            .into_iter()
            .map(|attribute| attribute.into_source(capture, sources))
            .collect::<Result<_, _>>()?;
        Ok(InteriorChild {
            position: sources.value(
                SourceValue::Present(position),
                std::slice::from_ref(&self.selection),
                true,
            ),
            name,
            local_transform: sources.value(value, &uses, true),
            attributes,
        })
    }

    fn named_attribute_uses(
        &self,
        capture: &EntryCapture<'_>,
    ) -> Result<Vec<SourceUse>, CatalogError> {
        let mut uses = vec![self.selection.clone()];
        for attribute in matching(&self.attributes, "name") {
            let value = attribute.value.retain_in(capture)?;
            uses.extend_from_slice(value.contributions().0);
        }
        Ok(uses)
    }
}

impl ChildAttribute {
    fn into_source(
        self,
        capture: &EntryCapture<'_>,
        sources: &SourceUses,
    ) -> Result<SocAttribute, CatalogError> {
        let original = self.value.retain_in(capture)?;
        let (uses, complete) = original.contributions();
        let name = sources.value(SourceValue::Present(self.name), uses, complete);
        let value = original
            .map(|text| text.as_str().to_owned())?
            .into_cell(sources);
        Ok(SocAttribute { name, value })
    }
}

fn matching<'attrs>(
    attributes: &'attrs [ChildAttribute],
    name: &'static str,
) -> impl Iterator<Item = &'attrs ChildAttribute> {
    attributes
        .iter()
        .filter(move |attribute| attribute.name.eq_ignore_ascii_case(name))
}

const fn is_transform_attribute(name: &str) -> bool {
    name.eq_ignore_ascii_case("pos") || name.eq_ignore_ascii_case("rot")
}

fn transform<'node>(node: impl Node<'node>, attributes: &[ChildAttribute]) -> Option<Transform> {
    if matching(attributes, "pos").nth(1).is_some() || matching(attributes, "rot").nth(1).is_some()
    {
        return None;
    }
    let pose = starmap_xml::pose(node)?;
    Transform::from_quaternion(
        pose.rotation.unwrap_or([1.0, 0.0, 0.0, 0.0]),
        pose.position.unwrap_or([0.0; 3]),
    )
}
