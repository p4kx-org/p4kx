// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::iter;
use std::sync::Arc;

use p4kx_extraction::{
    SharedFieldOrigin, SourceUse, SourceUseRole, SourceUses, SourceValue, SourcedValue,
};

use crate::CatalogError;
use crate::catalog_read::original::{
    AuthoredEntryPath, EntryXml, XmlAttributeSource, XmlNodeSource,
};
use crate::starmap_xml::{Root, attribute};

#[derive(Debug)]
pub(crate) struct ChildOrigins {
    node: XmlNodeSource,
    list: XmlNodeSource,
    position: XmlAttributeSource,
    rotation: XmlAttributeSource,
    record: XmlAttributeSource,
    entity: ChildText,
    label: ChildText,
}

impl ChildOrigins {
    pub(crate) fn capture(
        xml: &EntryXml<'_, '_, '_>,
        list: &XmlNodeSource,
        child: Root<'_>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            node: xml.node_source(child)?,
            list: list.clone(),
            position: xml.attribute_source(child, "pos")?,
            rotation: xml.attribute_source(child, "rot")?,
            record: xml.attribute_source(child, "starMapRecord")?,
            entity: ChildText::read(xml, child, "entityName")?,
            label: ChildText::read(xml, child, "label")?,
        })
    }

    pub(crate) fn entity_name(&self) -> Option<&str> {
        self.entity.value.as_deref()
    }
}

#[derive(Debug)]
struct ChildText {
    value: Option<String>,
    source: XmlAttributeSource,
}

impl ChildText {
    fn read(
        xml: &EntryXml<'_, '_, '_>,
        child: Root<'_>,
        field: &'static str,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            value: attribute(child, field).map(str::to_owned),
            source: xml.attribute_source(child, field)?,
        })
    }

    fn project(&self, sources: &SourceUses) -> Result<SourcedValue<String>, CatalogError> {
        let value = self
            .value
            .clone()
            .map_or(SourceValue::Absent, SourceValue::Present);
        let source = self.source.observe(SourceUseRole::Payload)?;
        Ok(sources.value(value, &[source], true))
    }
}

/// The authored names of the Child that declares an external occurrence.
pub(crate) struct ChildNames {
    pub(crate) label: SourcedValue<String>,
    pub(crate) entity_name: SourcedValue<String>,
}

impl ChildNames {
    pub(crate) fn from_route(
        route: Option<&PlacementStep>,
        root: &SourceUse,
        sources: &SourceUses,
    ) -> Result<Self, CatalogError> {
        let Some(step) = route else {
            let absent = sources.value(SourceValue::Absent, std::slice::from_ref(root), true);
            return Ok(Self {
                label: absent.clone(),
                entity_name: absent,
            });
        };
        Ok(Self {
            label: step.child.label.project(sources)?,
            entity_name: step.child.entity.project(sources)?,
        })
    }
}

#[derive(Debug)]
pub(crate) struct PlacementStep {
    pub(crate) preceding: Option<Arc<Self>>,
    pub(crate) via: Option<AuthoredEntryPath>,
    pub(crate) child: Arc<ChildOrigins>,
    pub(crate) has_record: bool,
}

impl PlacementStep {
    pub(crate) fn steps(&self) -> impl Iterator<Item = &Self> {
        iter::successors(Some(self), |step| step.preceding.as_deref())
    }

    pub(crate) fn child_index(&self) -> u32 {
        self.child.node.index()
    }

    pub(crate) fn selection(&self, uses: &mut Vec<SourceUse>) -> Result<bool, CatalogError> {
        uses.push(self.child.list.selected()?);
        uses.push(self.child.node.selected()?);
        if let Some(via) = &self.via {
            let (selectors, known) = via.selector_inputs()?;
            uses.extend(selectors);
            return Ok(known);
        }
        Ok(true)
    }

    pub(crate) fn position(&self, role: SourceUseRole) -> Result<SourceUse, CatalogError> {
        self.child.position.observe(role)
    }

    pub(crate) fn rotation(&self, role: SourceUseRole) -> Result<SourceUse, CatalogError> {
        self.child.rotation.observe(role)
    }

    pub(crate) fn position_uses(&self) -> Result<Vec<SourceUse>, CatalogError> {
        let mut uses = vec![self.position(SourceUseRole::Conversion)?];
        for step in self.steps().skip(1) {
            uses.push(step.position(SourceUseRole::Conversion)?);
            uses.push(step.rotation(SourceUseRole::Conversion)?);
        }
        Ok(uses)
    }
}

/// The actual root choice and ordered Child route of one placement occurrence.
#[derive(Debug, Clone)]
pub(crate) struct PlacementProof {
    pub(crate) root: Arc<[SourceUse]>,
    pub(crate) last: Arc<PlacementStep>,
}

#[derive(Debug)]
pub(crate) enum PlacementEvidence {
    Selected(PlacementProof),
    Ambiguous(Arc<[PlacementProof; 2]>),
}

impl PlacementEvidence {
    pub(crate) fn shared(&self, sources: &SourceUses) -> Result<SharedFieldOrigin, CatalogError> {
        let mut uses = Vec::new();
        let complete = match self {
            Self::Selected(proof) => proof.route_uses(&mut uses)?,
            Self::Ambiguous(pair) => {
                let mut complete = true;
                for proof in pair.iter() {
                    complete &= proof.route_uses(&mut uses)?;
                    uses.extend(proof.position_uses()?);
                }
                complete
            }
        };
        Ok(sources.shared_origin(&uses, complete))
    }

    pub(crate) fn uses(&self, field: PlacementField) -> Result<Vec<SourceUse>, CatalogError> {
        match self {
            Self::Selected(proof) => proof.uses(field),
            Self::Ambiguous(_) => Ok(Vec::new()),
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) enum PlacementField {
    System,
    Entity,
    Parent,
    Position,
}

impl PlacementProof {
    fn route_uses(&self, uses: &mut Vec<SourceUse>) -> Result<bool, CatalogError> {
        uses.extend_from_slice(&self.root);
        let mut complete = true;
        for step in self.last.steps() {
            complete &= step.selection(uses)?;
        }
        uses.push(self.last.child.record.observe(SourceUseRole::Selector)?);
        Ok(complete)
    }

    fn uses(&self, field: PlacementField) -> Result<Vec<SourceUse>, CatalogError> {
        match field {
            PlacementField::System => Ok(Vec::new()),
            PlacementField::Entity => Ok(vec![
                self.last
                    .child
                    .entity
                    .source
                    .observe(SourceUseRole::Payload)?,
            ]),
            PlacementField::Parent => self.parent_uses(),
            PlacementField::Position => self.position_uses(),
        }
    }

    fn parent_uses(&self) -> Result<Vec<SourceUse>, CatalogError> {
        let mut uses = Vec::new();
        for step in self.last.steps().skip(1) {
            let role = if step.has_record {
                SourceUseRole::Conversion
            } else {
                SourceUseRole::Selector
            };
            uses.push(step.child.record.observe(role)?);
            if step.has_record {
                break;
            }
        }
        Ok(uses)
    }

    fn position_uses(&self) -> Result<Vec<SourceUse>, CatalogError> {
        self.last.position_uses()
    }
}
