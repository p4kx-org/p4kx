// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::ops::Range;

use crate::catalog_read::original::{EntryCapture, EntryRead, EntrySoc, EntrySocEntity};
use crate::shop_inventory_name;
use crate::shop_registry;
use crate::shop_soc::{Anchors, RawFinding};
use p4kx_extraction::chunks::{Chunk, ChunkFile, ChunkType, Container};
use p4kx_extraction::cryxml::{
    Format, SourceAttribute, SourceContent, SourceElement, SourceElementPosition,
};

#[path = "shop_soc_structure_evidence.rs"]
mod evidence;
#[path = "shop_soc_invalid_shape.rs"]
mod invalid_shape;
use evidence::Scope;
pub(super) use evidence::{Evidence, InventoryValue, Metric, RawKind, Relation};
use invalid_shape::InvalidShape;

const ENTITY_CHUNK: ChunkType = ChunkType::new(Container::CrCh, 0x0004);
const SHOP_COMPONENT: &str = "EntityComponentShop";
const SHOP_PARAMETERS: &str = "SShopComponentParams";

pub(crate) struct Probe<'scope, 'archive> {
    pub(in super::super) capture: &'scope EntryCapture<'archive>,
    pub(in super::super) anchors: &'scope Anchors,
}

impl Probe<'_, '_> {
    pub(super) fn inspect(&self, original: &EntryRead, raw: &[RawFinding]) -> Evidence {
        let mut evidence = Evidence::default();
        for hit in raw {
            evidence.add(Metric::Raw(kind(hit), Relation::Total));
        }
        let Ok(Ok(soc)) = EntrySoc::parse(original, self.capture) else {
            evidence.add(Metric::UnparsedSoc);
            return evidence;
        };
        evidence.add(Metric::VerifiedSoc);
        let mut comparison = Comparison::new(original.bytes(), raw, self.anchors, &mut evidence);
        comparison.inspect(&soc);
        evidence
    }
}

struct Comparison<'source, 'evidence> {
    bytes: &'source [u8],
    anchors: &'source Anchors,
    values: BTreeMap<usize, &'source RawFinding>,
    properties: BTreeMap<usize, &'source RawFinding>,
    evidence: &'evidence mut Evidence,
}

impl<'source, 'evidence> Comparison<'source, 'evidence> {
    fn new(
        bytes: &'source [u8],
        raw: &'source [RawFinding],
        anchors: &'source Anchors,
        evidence: &'evidence mut Evidence,
    ) -> Self {
        let mut values = BTreeMap::new();
        let mut properties = BTreeMap::new();
        for hit in raw {
            values.insert(hit.value_offset, hit);
            if let Some(invalid) = &hit.invalid {
                properties.insert(invalid.property_offset, hit);
            }
        }
        Self {
            bytes,
            anchors,
            values,
            properties,
            evidence,
        }
    }

    fn inspect(&mut self, soc: &EntrySoc<'_>) {
        if let Ok(table) = ChunkFile::parse(self.bytes) {
            for chunk in table.chunks() {
                if chunk.chunk_type() != ENTITY_CHUNK {
                    self.evidence.add(Metric::UninspectedChunk);
                    self.observe_range(chunk, Relation::StartsInUninspectedChunk);
                }
            }
        }
        for chunk in soc.chunks() {
            let Ok(chunk) = chunk else {
                self.evidence.add(Metric::UnreadableEntityChunk);
                continue;
            };
            if chunk.is_lossy() {
                self.evidence.add(Metric::LossyEntityChunk);
                continue;
            }
            let original = chunk.original();
            let metric = match original.document().format() {
                Format::Text => Metric::ParsedTextEntityChunk,
                Format::CryXml(_) => Metric::ParsedCryXmlEntityChunk,
            };
            self.evidence.add(metric);
            self.observe_range(original.header(), Relation::StartsInLosslessEntityChunk);
            self.inspect_document(original.document().root());
            for owner in chunk.entities() {
                self.inspect_entity(&owner);
            }
        }
    }

    fn observe_range(&mut self, chunk: &Chunk, relation: Relation) {
        if let Some(range) = chunk_range(chunk) {
            for hit in self.values.range(range).map(|(_, hit)| *hit) {
                self.evidence.add(Metric::Raw(kind(hit), relation));
            }
        }
    }

    fn inspect_document(&mut self, root: SourceElement<'_>) {
        let mut pending = vec![root];
        while let Some(node) = pending.pop() {
            self.observe_source_string(node.name(), Relation::NodeName);
            for attribute in node.attributes() {
                self.observe_source_string(attribute.name(), Relation::AttributeName);
                self.observe_source_string(attribute.value(), Relation::AttributeValue);
            }
            if let Ok(syntax) = node.syntax()
                && let Ok(contents) = syntax.content()
            {
                for item in contents {
                    if let Ok(SourceContent::Text(text)) = item {
                        self.observe_source_string(text.value(), Relation::NodeContent);
                    }
                }
            }
            pending.extend(node.elements());
        }
    }

    fn observe_source_string(&mut self, text: &str, relation: Relation) {
        if let Some(offset) = source_offset(self.bytes, text)
            && let Some(hit) = self.values.get(&offset)
        {
            self.evidence.add(Metric::Raw(kind(hit), relation));
        }
    }

    fn inspect_entity(&mut self, owner: &EntrySocEntity<'_>) {
        self.evidence.add(Metric::DirectEntity);
        let entity = owner.original();
        let mut attributes = entity.guid_attributes();
        let guid = attributes
            .next()
            .filter(|attribute| {
                attributes.next().is_none()
                    && shop_registry::is_guid(attribute.value())
                    && owner.attribute_receipt(attribute).is_ok()
            })
            .map(|attribute| attribute.value().to_ascii_lowercase());
        let guid = guid.filter(|guid| self.anchors.guid_receipts(guid).len() == 1);
        let mut appropriate = Vec::new();
        for (scope, node) in components(entity.element()) {
            self.evidence.add(Metric::Component(scope));
            appropriate.push(node.position());
            self.inspect_attributes(owner, guid.as_deref(), node);
            for params in node.elements().filter(|node| named(*node, SHOP_PARAMETERS)) {
                self.evidence.add(Metric::ParameterNode);
                appropriate.push(params.position());
                self.inspect_attributes(owner, guid.as_deref(), params);
            }
        }
        self.inspect_other_fields(entity.element(), &appropriate);
    }

    fn inspect_attributes(
        &mut self,
        owner: &EntrySocEntity<'_>,
        guid: Option<&str>,
        node: SourceElement<'_>,
    ) {
        for attribute in node.attributes().filter(|attribute| {
            attribute.namespace().is_none() && inventory_field(attribute.name())
        }) {
            let value = inventory_value(attribute.value());
            if owner.attribute_receipt(&attribute).is_err() {
                self.evidence.add(Metric::AttributeWithoutReceipt);
                continue;
            }
            self.evidence.add(Metric::InventoryAttribute(value));
            if guid.is_some() {
                self.evidence
                    .add(Metric::InventoryAttributeWithUniqueMetadataGuid(value));
            }
            self.compare_attribute(&attribute, guid, value);
        }
    }

    fn compare_attribute(
        &mut self,
        attribute: &SourceAttribute<'_>,
        guid: Option<&str>,
        value: InventoryValue,
    ) {
        let value_offset = source_offset(self.bytes, attribute.value());
        if let Some(hit) = value_offset.and_then(|offset| self.values.get(&offset)) {
            self.evidence
                .add(Metric::Raw(kind(hit), Relation::ExactInventoryValue(value)));
            if guid.is_some() {
                self.evidence.add(Metric::Raw(
                    kind(hit),
                    Relation::ExactInventoryValueWithUniqueMetadataGuid(value),
                ));
            }
            if same_owner(hit, guid) {
                self.evidence.add(Metric::Raw(
                    kind(hit),
                    Relation::ExactInventoryValueWithSameAttachedGuid,
                ));
            }
        }
        let property_offset = source_offset(self.bytes, attribute.name());
        if let Some(hit) = property_offset.and_then(|offset| self.properties.get(&offset)) {
            self.evidence.add(Metric::Raw(
                kind(hit),
                Relation::ExactInventoryProperty(value),
            ));
            if same_owner(hit, guid) {
                self.evidence.add(Metric::Raw(
                    kind(hit),
                    Relation::ExactInventoryPropertyWithSameAttachedGuid(value),
                ));
            }
            if value_offset == Some(hit.value_offset) {
                self.evidence.add(Metric::Raw(
                    kind(hit),
                    Relation::ExactInventoryPropertyAndValue,
                ));
                if value == InventoryValue::Invalid && same_owner(hit, guid) {
                    self.evidence
                        .add(Metric::MatchingInvalidShape(InvalidShape::of(
                            attribute.name(),
                            attribute.value(),
                        )));
                }
            }
        }
    }

    fn inspect_other_fields(
        &mut self,
        root: SourceElement<'_>,
        appropriate: &[SourceElementPosition],
    ) {
        let mut pending = vec![root];
        while let Some(node) = pending.pop() {
            if inventory_field(node.name()) {
                self.evidence.add(Metric::InventoryElementInEntity);
            }
            if !appropriate.contains(&node.position()) {
                for _ in node
                    .attributes()
                    .filter(|attribute| inventory_field(attribute.name()))
                {
                    self.evidence
                        .add(Metric::InventoryPropertyInOtherEntityScope);
                }
            }
            pending.extend(node.elements());
        }
    }
}

fn kind(hit: &RawFinding) -> RawKind {
    match &hit.invalid {
        None => RawKind::UnattachedPath,
        Some(invalid) if invalid.owner.is_some() => RawKind::AttachedInvalid,
        Some(_) => RawKind::UnattachedInvalid,
    }
}

fn same_owner(hit: &RawFinding, guid: Option<&str>) -> bool {
    guid.is_some()
        && guid
            == hit
                .invalid
                .as_ref()
                .and_then(|invalid| invalid.owner.as_deref())
}

fn inventory_value(value: &str) -> InventoryValue {
    if value.trim().is_empty() {
        InventoryValue::Empty
    } else if shop_inventory_name::stem(value).is_some() {
        InventoryValue::File
    } else {
        InventoryValue::Invalid
    }
}

fn inventory_field(field: &str) -> bool {
    const PREFIX: &str = "offlineInventory";
    field
        .get(..PREFIX.len())
        .is_some_and(|head| head.eq_ignore_ascii_case(PREFIX))
}

fn named(element: SourceElement<'_>, name: &str) -> bool {
    element.namespace().is_none() && element.name().eq_ignore_ascii_case(name)
}

fn components(entity: SourceElement<'_>) -> impl Iterator<Item = (Scope, SourceElement<'_>)> {
    entity.elements().flat_map(|outer| {
        let direct = named(outer, SHOP_COMPONENT).then_some((Scope::Direct, outer));
        let wrapper = if named(outer, "PropertiesDataCore") {
            Some(Scope::Properties)
        } else if named(outer, "Components") {
            Some(Scope::Components)
        } else {
            None
        };
        direct
            .into_iter()
            .chain(wrapper.into_iter().flat_map(move |scope| {
                outer.elements().filter_map(move |component| {
                    named(component, SHOP_COMPONENT).then_some((scope, component))
                })
            }))
    })
}

fn source_offset(bytes: &[u8], text: &str) -> Option<usize> {
    if text.is_empty() {
        return None;
    }
    let offset = text.as_ptr().addr().checked_sub(bytes.as_ptr().addr())?;
    bytes
        .get(offset..offset.checked_add(text.len())?)
        .map(|_| offset)
}

fn chunk_range(chunk: &Chunk) -> Option<Range<usize>> {
    let start = usize::try_from(chunk.offset()).ok()?;
    let size = usize::try_from(chunk.size()).ok()?;
    Some(start..start.checked_add(size)?)
}
