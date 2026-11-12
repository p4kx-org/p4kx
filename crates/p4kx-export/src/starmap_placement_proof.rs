// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::iter;
use std::sync::Arc;

use p4kx_extraction::{SharedFieldOrigin, SourceUse, SourceUseRole, SourceUses};

use crate::CatalogError;
use crate::catalog_read::original::{
    AuthoredEntryPath, EntryXml, XmlAttributeSource, XmlNodeSource,
};
use crate::starmap_xml::Root;

#[derive(Debug)]
pub(crate) struct ChildOrigins {
    node: XmlNodeSource,
    list: XmlNodeSource,
    position: XmlAttributeSource,
    rotation: XmlAttributeSource,
    record: XmlAttributeSource,
    entity: XmlAttributeSource,
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
            entity: xml.attribute_source(child, "entityName")?,
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
        for step in self.steps() {
            uses.push(step.child.list.selected()?);
            uses.push(step.child.node.selected()?);
            if let Some(via) = &step.via {
                let (selectors, known) = via.selector_inputs()?;
                uses.extend(selectors);
                complete &= known;
            }
        }
        uses.push(self.last.child.record.observe(SourceUseRole::Selector)?);
        Ok(complete)
    }

    fn uses(&self, field: PlacementField) -> Result<Vec<SourceUse>, CatalogError> {
        match field {
            PlacementField::System => Ok(Vec::new()),
            PlacementField::Entity => Ok(vec![
                self.last.child.entity.observe(SourceUseRole::Payload)?,
            ]),
            PlacementField::Parent => self.parent_uses(),
            PlacementField::Position => self.position_uses(),
        }
    }

    fn steps(&self) -> impl Iterator<Item = &PlacementStep> {
        iter::successors(Some(self.last.as_ref()), |step| step.preceding.as_deref())
    }

    fn parent_uses(&self) -> Result<Vec<SourceUse>, CatalogError> {
        let mut uses = Vec::new();
        for step in self.steps().skip(1) {
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
        let mut uses = vec![
            self.last
                .child
                .position
                .observe(SourceUseRole::Conversion)?,
        ];
        for step in self.steps().skip(1) {
            uses.push(step.child.position.observe(SourceUseRole::Conversion)?);
            uses.push(step.child.rotation.observe(SourceUseRole::Conversion)?);
        }
        Ok(uses)
    }
}
