// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    Guid, SourceUse, SourceUseRole, SourceUses, SourceValue, SourceValueState, SourcedValue,
};
use schemars::JsonSchema;
use serde::Serialize;

use crate::CatalogError;
use crate::catalog_read::original::{EntryCapture, EntryXml};
use crate::export_ledger::ExportLedger;
use crate::guid_text::CatalogGuid;
use crate::schema_fields::every_field_required;
use crate::starmap_parse::entxml::{EntXmlIndex, EntXmlLink, EntXmlRequest};
use crate::starmap_parse::hcd::{HcdLink, HcdLookup, HcdReader};
use crate::starmap_xml::{Node, Root, attribute};

const LOCATION_TYPE: &str = "locationType";

#[path = "starmap_location_metadata.rs"]
mod metadata;

pub use metadata::LocationMetadataElement;

/// The authored fields and component declarations of a typed location Child.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct ChildLocation {
    location_type: SourcedValue<u32>,
    streaming_root: SourcedValue<bool>,
    guid: SourcedValue<CatalogGuid>,
    class: SourcedValue<String>,
    class_guid: SourcedValue<CatalogGuid>,
    #[schemars(extend("x-gameText" = true))]
    label: SourcedValue<String>,
    entity_name: SourcedValue<String>,
    tags_text: SourcedValue<String>,
    tags: SourcedValue<Vec<SourcedValue<LocationTag>>>,
    has_hierarchical_context_data: SourcedValue<bool>,
    hierarchical_context: Option<HcdLink>,
    #[serde(skip)]
    #[schemars(skip)]
    entity_request: Option<EntXmlRequest>,
    ent_xml: Option<EntXmlLink>,
    invalid_attributes: Vec<LocationAttribute>,
    instance_metadata: SourcedValue<Vec<LocationMetadataElement>>,
}

impl ChildLocation {
    /// Returns the authored numeric location type.
    #[must_use]
    pub const fn location_type(&self) -> &SourcedValue<u32> {
        &self.location_type
    }

    /// Returns the authored streaming root flag.
    #[must_use]
    pub const fn streaming_root(&self) -> &SourcedValue<bool> {
        &self.streaming_root
    }

    /// Returns the authored Child GUID.
    #[must_use]
    pub const fn guid(&self) -> &SourcedValue<CatalogGuid> {
        &self.guid
    }

    /// Returns the authored Child class name.
    #[must_use]
    pub const fn class(&self) -> &SourcedValue<String> {
        &self.class
    }

    /// Returns the independently authored class GUID.
    #[must_use]
    pub const fn class_guid(&self) -> &SourcedValue<CatalogGuid> {
        &self.class_guid
    }

    /// Returns the authored display label.
    #[must_use]
    pub const fn label(&self) -> &SourcedValue<String> {
        &self.label
    }

    /// Returns the authored entity name.
    #[must_use]
    pub const fn entity_name(&self) -> &SourcedValue<String> {
        &self.entity_name
    }

    /// Returns the original tag text.
    #[must_use]
    pub const fn tags_text(&self) -> &SourcedValue<String> {
        &self.tags_text
    }

    /// Returns the ordered original tag tokens.
    #[must_use]
    pub const fn tags(&self) -> &SourcedValue<Vec<SourcedValue<LocationTag>>> {
        &self.tags
    }

    /// Returns the authored hierarchical context flag.
    #[must_use]
    pub const fn has_hierarchical_context_data(&self) -> &SourcedValue<bool> {
        &self.has_hierarchical_context_data
    }

    /// Returns the same-container hierarchical context lookup.
    #[must_use]
    pub const fn hierarchical_context(&self) -> Option<&HcdLink> {
        self.hierarchical_context.as_ref()
    }

    /// Returns the same-container entity lookup.
    #[must_use]
    pub const fn ent_xml(&self) -> Option<&EntXmlLink> {
        self.ent_xml.as_ref()
    }

    /// Returns the original text of invalid typed attributes.
    #[must_use]
    pub const fn invalid_attributes(&self) -> &[LocationAttribute] {
        self.invalid_attributes.as_slice()
    }

    /// Returns the ordered instance metadata elements.
    #[must_use]
    pub const fn instance_metadata(&self) -> &SourcedValue<Vec<LocationMetadataElement>> {
        &self.instance_metadata
    }

    /// Retains a location declaration and its exact source uses from its owning Child.
    pub(crate) fn read(
        xml: &EntryXml<'_, '_, '_>,
        child: Root<'_>,
        capture: &EntryCapture<'_>,
    ) -> Result<Option<Self>, CatalogError> {
        if attribute(child, LOCATION_TYPE).is_none() {
            return Ok(None);
        }
        let mut fields = LocationFields {
            xml,
            child,
            sources: capture.source_uses(),
            complete: !xml.is_lossy(),
            invalid_attributes: Vec::new(),
        };
        let location_type = fields.typed(LOCATION_TYPE, |raw| raw.trim().parse().ok())?;
        let streaming_root = fields.typed("streamingRoot", boolean)?;
        let guid = fields.typed("guid", parse_guid)?;
        let class = fields.text("class")?;
        let class_guid = fields.typed("classGuid", parse_guid)?;
        let has_hierarchical_context_data = fields.typed("hasHierarchicalContextData", boolean)?;
        let tags = fields.tags()?;
        Ok(Some(Self {
            location_type,
            streaming_root,
            guid,
            class,
            class_guid,
            label: fields.text("label")?,
            entity_name: fields.text("entityName")?,
            tags_text: tags.text,
            tags: tags.tokens,
            has_hierarchical_context_data,
            hierarchical_context: None,
            entity_request: None,
            ent_xml: None,
            instance_metadata: metadata::read(xml, child, capture)?,
            invalid_attributes: fields.invalid_attributes,
        }))
    }

    pub(crate) fn request_entity(
        &mut self,
        xml: &EntryXml<'_, '_, '_>,
        child: Root<'_>,
    ) -> Result<Option<Guid>, CatalogError> {
        let request = EntXmlRequest::capture(xml, child, self.guid.value().map(|guid| guid.0))?;
        let guid = request.guid();
        self.entity_request = Some(request);
        Ok(guid)
    }

    pub(crate) fn attach_entity(&mut self, index: &EntXmlIndex, sources: &SourceUses) {
        if let Some(request) = self.entity_request.take() {
            self.ent_xml = Some(index.project(&request, sources));
        }
    }
    pub(crate) fn read_context(
        &mut self,
        hcd: &mut HcdReader<'_, '_, '_>,
        xml: &EntryXml<'_, '_, '_>,
        child: Root<'_>,
        ledger: &mut ExportLedger,
    ) -> Result<(), CatalogError> {
        let flag = &self.has_hierarchical_context_data;
        let target = match (flag.state(), flag.value()) {
            (SourceValueState::Invalid, _) => HcdLookup::InvalidFlag,
            (_, Some(true)) => self.guid.value().map_or(HcdLookup::InvalidGuid, |guid| {
                if guid.0 == Guid::default() {
                    HcdLookup::InvalidGuid
                } else {
                    HcdLookup::Guid(guid.0)
                }
            }),
            _ => return Ok(()),
        };
        self.hierarchical_context = Some(hcd.read(xml, child, target, ledger)?);
        Ok(())
    }
}

struct LocationFields<'xml, 'read, 'capture, 'input, 'node> {
    xml: &'xml EntryXml<'read, 'capture, 'input>,
    child: Root<'node>,
    sources: &'xml SourceUses,
    complete: bool,
    invalid_attributes: Vec<LocationAttribute>,
}

impl LocationFields<'_, '_, '_, '_, '_> {
    fn text(&self, name: &'static str) -> Result<SourcedValue<String>, CatalogError> {
        let raw = attribute(self.child, name).map(str::to_owned);
        let source = self.origin(name, SourceUseRole::Payload)?;
        Ok(self.sources.value(state(raw), &[source], self.complete))
    }

    fn typed<T>(
        &mut self,
        field_name: &'static str,
        parse: impl FnOnce(&str) -> Option<T>,
    ) -> Result<SourcedValue<T>, CatalogError> {
        let source = self.origin(field_name, SourceUseRole::Conversion)?;
        let original = self
            .child
            .attributes()
            .find(|(field, _)| field.eq_ignore_ascii_case(field_name));
        let value = match original {
            None => SourceValue::Absent,
            Some((name, raw)) => {
                if let Some(parsed) = parse(raw) {
                    SourceValue::Present(parsed)
                } else {
                    let name_use = self.origin(field_name, SourceUseRole::Selector)?;
                    let value_use = self.origin(field_name, SourceUseRole::Payload)?;
                    self.invalid_attributes.push(LocationAttribute::new(
                        self.sources,
                        name,
                        raw.to_owned(),
                        name_use,
                        value_use,
                        self.complete,
                    ));
                    SourceValue::Invalid
                }
            }
        };
        Ok(self.sources.value(value, &[source], self.complete))
    }

    fn origin(&self, name: &'static str, role: SourceUseRole) -> Result<SourceUse, CatalogError> {
        self.xml.attribute_source(self.child, name)?.observe(role)
    }

    fn tags(&self) -> Result<LocationTags, CatalogError> {
        let text = self.text("tags")?;
        let source = self.origin("tags", SourceUseRole::Conversion)?;
        let selection = std::slice::from_ref(&source);
        let parsed = attribute(self.child, "tags").map_or(SourceValue::Absent, |raw| {
            let mut tags = Vec::new();
            if !raw.is_empty() {
                for token in raw.split(',') {
                    let raw = self.sources.value(
                        SourceValue::Present(token.to_owned()),
                        selection,
                        self.complete,
                    );
                    let parsed =
                        parse_guid(token).map_or(SourceValue::Invalid, SourceValue::Present);
                    let guid = self.sources.value(parsed, selection, self.complete);
                    tags.push(self.sources.value(
                        SourceValue::Present(LocationTag { raw, guid }),
                        selection,
                        self.complete,
                    ));
                }
            }
            SourceValue::Present(tags)
        });
        Ok(LocationTags {
            text,
            tokens: self.sources.value(parsed, selection, self.complete),
        })
    }
}

struct LocationTags {
    text: SourcedValue<String>,
    tokens: SourcedValue<Vec<SourcedValue<LocationTag>>>,
}

fn state<T>(raw: Option<T>) -> SourceValue<T> {
    raw.map_or(SourceValue::Absent, SourceValue::Present)
}

fn parse_guid(raw: &str) -> Option<CatalogGuid> {
    raw.trim().parse::<Guid>().ok().map(CatalogGuid)
}

fn boolean(raw: &str) -> Option<bool> {
    match raw.trim() {
        "0" | "false" => Some(false),
        "1" | "true" => Some(true),
        _ => None,
    }
}

/// An original token in the Child location tags.
#[derive(Clone, Debug, Serialize, JsonSchema)]
#[schemars(transform = every_field_required)]
pub struct LocationTag {
    raw: SourcedValue<String>,
    guid: SourcedValue<CatalogGuid>,
}

impl LocationTag {
    /// Returns the authored tag token.
    #[must_use]
    pub const fn raw(&self) -> &SourcedValue<String> {
        &self.raw
    }

    /// Returns the parsed tag GUID.
    #[must_use]
    pub const fn guid(&self) -> &SourcedValue<CatalogGuid> {
        &self.guid
    }
}

/// An original location element attribute.
#[derive(Clone, Debug, Serialize, JsonSchema)]
#[schemars(transform = every_field_required)]
pub struct LocationAttribute {
    name: SourcedValue<String>,
    #[schemars(extend("x-gameText" = true))]
    value: SourcedValue<String>,
}

impl LocationAttribute {
    /// Returns the authored attribute name.
    #[must_use]
    pub const fn name(&self) -> &SourcedValue<String> {
        &self.name
    }

    /// Returns the authored attribute value.
    #[must_use]
    pub const fn value(&self) -> &SourcedValue<String> {
        &self.value
    }

    fn new(
        sources: &SourceUses,
        name: &str,
        value: String,
        name_use: SourceUse,
        value_use: SourceUse,
        complete: bool,
    ) -> Self {
        Self {
            name: sources.value(SourceValue::Present(name.to_owned()), &[name_use], complete),
            value: sources.value(SourceValue::Present(value), &[value_use], complete),
        }
    }
}
