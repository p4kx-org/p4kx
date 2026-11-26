// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::sync::Arc;

use p4kx_assembly::{PortDefinition, PortDefinitionPosition, VehiclePortSelection};
use p4kx_extraction::{
    Archive, SharedFieldOrigin, SourceUse, SourceUseRole, SourceUses, SourceValue, SourcedValue,
    StructValue, Value,
};

use super::Capture;
use super::port_type_attributes::RecordPortTypes;
use super::port_vehicle_sources::VehiclePortSources;
use crate::catalog_read::original::{
    AuthoredEntryPath, OriginalFlowFields, OriginalValueEdge, OriginalValueFact, OriginalValuePart,
};
use crate::contract_loadout::PortType;
use crate::{CatalogError, ExportAccountingError, Kind};

struct DefinitionFacts {
    selectors: Vec<SourceUse>,
    min: FieldFact<u32>,
    max: FieldFact<u32>,
    flags: FieldFact<Vec<String>>,
    port_tags: FieldFact<Vec<String>>,
    required_tags: FieldFact<Vec<String>>,
    display_name_key: FieldFact<String>,
    types: RecordPortTypes,
}

pub(super) struct FieldFact<T> {
    pub(super) value: SourceValue<T>,
    pub(super) inputs: Vec<SourceUse>,
    pub(super) complete: bool,
}

#[derive(Default)]
pub(super) struct RecordPortAttributes {
    definitions: HashMap<PortDefinitionPosition, Arc<DefinitionFacts>>,
}

pub(crate) struct PortAttributes {
    pub(crate) min_size: SourcedValue<u32>,
    pub(crate) max_size: SourcedValue<u32>,
    pub(crate) flags: SourcedValue<Vec<String>>,
    pub(crate) port_tags: SourcedValue<Vec<String>>,
    pub(crate) required_tags: SourcedValue<Vec<String>>,
    pub(crate) display_name_key: SourcedValue<String>,
    pub(crate) types: SourcedValue<Vec<PortType>>,
    pub(crate) visibility: Option<SourcedValue<&'static str>>,
}

impl Capture<'_> {
    pub(super) fn record_port_attributes(
        &self,
        fields: &OriginalFlowFields<'_, '_, '_>,
        definition: &PortDefinition,
        original: &StructValue<'_>,
    ) -> Result<(), CatalogError> {
        if self.kind != Kind::Loadouts {
            return Ok(());
        }
        let position = definition.position();
        let (component, port) = definition_site(fields, position, original)?;
        let attributes = DefinitionFacts {
            selectors: selection(fields, component, port)?,
            min: record_field(fields, port, "MinSize", |_| definition.min_size())?,
            max: record_field(fields, port, "MaxSize", |_| definition.max_size())?,
            flags: record_field(fields, port, "Flags", |raw| text(raw).map(flag_words))?,
            port_tags: record_field(fields, port, "PortTags", |raw| text(raw).map(tag_words))?,
            required_tags: record_field(fields, port, "RequiredPortTags", |raw| {
                text(raw).map(tag_words)
            })?,
            display_name_key: read_record_field(
                fields,
                port,
                "DisplayName",
                SourceUseRole::Payload,
                |raw| text(raw).map(str::to_owned),
            )?,
            types: RecordPortTypes::read(fields, port)?,
        };
        let mut state = self.lock()?;
        if state
            .port_attributes
            .definitions
            .insert(position, Arc::new(attributes))
            .is_some()
        {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        drop(state);
        Ok(())
    }

    pub(crate) fn definition_attributes(
        &self,
        definition: &PortDefinition,
        name: Option<&AuthoredEntryPath>,
    ) -> Result<PortAttributes, CatalogError> {
        if self.input.is_none() {
            let unsourced = |value: Option<u32>| {
                self.name_sources.value(
                    value.map_or(SourceValue::Absent, SourceValue::Present),
                    &[],
                    false,
                )
            };
            let words = |value| {
                self.name_sources
                    .value(SourceValue::Present(value), &[], false)
            };
            return Ok(PortAttributes {
                min_size: unsourced(definition.min_size()),
                max_size: unsourced(definition.max_size()),
                flags: words(flag_words(definition.flags())),
                port_tags: words(tag_words(definition.port_tags())),
                required_tags: words(tag_words(definition.required_tags())),
                display_name_key: self.name_sources.value(
                    SourceValue::Present(definition.display_name().to_owned()),
                    &[],
                    false,
                ),
                types: RecordPortTypes::unbound(definition, &self.name_sources),
                visibility: None,
            });
        }
        let facts = {
            let state = self.lock()?;
            Arc::clone(
                state
                    .port_attributes
                    .definitions
                    .get(&definition.position())
                    .ok_or(ExportAccountingError::UnobservedFailure)?,
            )
        };
        let (selectors, complete) = name.map_or_else(
            || Ok((Vec::new(), false)),
            AuthoredEntryPath::selector_inputs,
        )?;
        facts.bind(&self.name_sources, selectors, complete)
    }

    pub(crate) fn vehicle_attributes(
        &self,
        selection: &VehiclePortSelection<'_>,
        name: Option<&AuthoredEntryPath>,
        archive: Option<&Archive>,
        record_index: Option<u32>,
    ) -> Result<PortAttributes, CatalogError> {
        let port = selection.port();
        let (min, min_site) = port.min_size_source();
        let (max, max_site) = port.max_size_source();
        let (flags, flag_site) = port.flags_source();
        let (tags, tag_site) = port.port_tags_source();
        let (required, required_site) = port.required_tags_source();
        let (display_name, display_name_site) = port.display_name_source();
        let state = self.lock()?;
        let source =
            VehiclePortSources::read(self, &state, selection, name, archive, record_index)?;
        let attributes = PortAttributes {
            min_size: source.value(min.clone(), min_site, SourceUseRole::Conversion)?,
            max_size: source.value(max.clone(), max_site, SourceUseRole::Conversion)?,
            flags: source.value(
                flags.map_or(SourceValue::Absent, |value| {
                    SourceValue::Present(flag_words(value))
                }),
                flag_site,
                SourceUseRole::Conversion,
            )?,
            port_tags: source.value(
                tags.map_or(SourceValue::Absent, |value| {
                    SourceValue::Present(tag_words(value))
                }),
                tag_site,
                SourceUseRole::Conversion,
            )?,
            required_tags: source.value(
                required.map_or(SourceValue::Absent, |value| {
                    SourceValue::Present(tag_words(value))
                }),
                required_site,
                SourceUseRole::Conversion,
            )?,
            display_name_key: source.value(
                display_name.map_or(SourceValue::Absent, |name| {
                    SourceValue::Present(name.to_owned())
                }),
                display_name_site,
                SourceUseRole::Payload,
            )?,
            types: source.item_types(port.item_types(), selection.position())?,
            visibility: Some(source.value(
                SourceValue::Present(port.visibility().code()),
                flag_site,
                SourceUseRole::Conversion,
            )?),
        };
        drop(state);
        Ok(attributes)
    }
}

impl DefinitionFacts {
    fn bind(
        &self,
        sources: &SourceUses,
        mut selectors: Vec<SourceUse>,
        complete: bool,
    ) -> Result<PortAttributes, CatalogError> {
        selectors.extend_from_slice(&self.selectors);
        let shared = sources.shared_origin(&selectors, complete);
        Ok(PortAttributes {
            min_size: self.min.bind(sources, &shared)?,
            max_size: self.max.bind(sources, &shared)?,
            flags: self.flags.bind(sources, &shared)?,
            port_tags: self.port_tags.bind(sources, &shared)?,
            required_tags: self.required_tags.bind(sources, &shared)?,
            display_name_key: self.display_name_key.bind(sources, &shared)?,
            types: self.types.bind(sources, &shared)?,
            visibility: None,
        })
    }
}

impl<T> FieldFact<T> {
    pub(super) fn bind(
        &self,
        sources: &SourceUses,
        shared: &SharedFieldOrigin,
    ) -> Result<SourcedValue<T>, CatalogError>
    where
        T: Clone,
    {
        Ok(sources.value_with_shared_origin(
            self.value.clone(),
            shared,
            &self.inputs,
            self.complete,
        )?)
    }

    pub(super) fn bind_map<U>(
        &self,
        sources: &SourceUses,
        shared: &SharedFieldOrigin,
        map: impl FnOnce(&T) -> Result<U, CatalogError>,
    ) -> Result<SourcedValue<U>, CatalogError> {
        let value = match &self.value {
            SourceValue::Present(value) => SourceValue::Present(map(value)?),
            SourceValue::Absent => SourceValue::Absent,
            SourceValue::Null => SourceValue::Null,
            SourceValue::Unreadable => SourceValue::Unreadable,
            SourceValue::Invalid => SourceValue::Invalid,
            SourceValue::Unsupported => SourceValue::Unsupported,
            _ => SourceValue::Unsupported,
        };
        Ok(sources.value_with_shared_origin(value, shared, &self.inputs, self.complete)?)
    }
}

fn definition_site<'fields, 'scope, 'read, 'source>(
    fields: &'fields OriginalFlowFields<'scope, 'read, 'source>,
    position: PortDefinitionPosition,
    original: &StructValue<'_>,
) -> Result<
    (
        &'fields OriginalValueFact<'scope, 'read, 'source>,
        &'fields OriginalValueFact<'scope, 'read, 'source>,
    ),
    CatalogError,
> {
    if position.record_index() != fields.record_index() {
        return Err(ExportAccountingError::ConflictingSourceDatum.into());
    }
    let (_, components) = fields
        .field(None, "Components")?
        .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
    let component = fields.child(
        Some(components.ordinal()),
        element(position.component_ordinal())?,
    )?;
    let (_, ports) = fields
        .field(Some(component), "Ports")?
        .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
    let port = fields.child(Some(ports.ordinal()), element(position.port_ordinal())?)?;
    if !matches!(port.value(), Value::Struct(source) if std::ptr::addr_eq(source, original)) {
        return Err(ExportAccountingError::ConflictingSourceDatum.into());
    }
    Ok((component, port))
}

fn element(ordinal: usize) -> Result<OriginalValueEdge, ExportAccountingError> {
    Ok(OriginalValueEdge::Element {
        ordinal: u64::try_from(ordinal).map_err(|_| ExportAccountingError::CounterOverflow)?,
    })
}

fn selection<'scope, 'read, 'source>(
    fields: &OriginalFlowFields<'scope, 'read, 'source>,
    component: &OriginalValueFact<'scope, 'read, 'source>,
    port: &OriginalValueFact<'scope, 'read, 'source>,
) -> Result<Vec<SourceUse>, CatalogError> {
    let mut selectors = vec![
        component.observe(OriginalValuePart::ConcreteType, SourceUseRole::Selector)?,
        port.observe(OriginalValuePart::Value, SourceUseRole::Selector)?,
    ];
    fields.observe_field_selection(port, "Name", &mut |input| {
        selectors.push(input);
        Ok(())
    })?;
    Ok(selectors)
}

fn record_field<'scope, 'read, 'source, T>(
    fields: &OriginalFlowFields<'scope, 'read, 'source>,
    port: &OriginalValueFact<'scope, 'read, 'source>,
    name: &'static str,
    parse: impl FnOnce(&Value<'_>) -> Option<T>,
) -> Result<FieldFact<T>, CatalogError> {
    read_record_field(fields, port, name, SourceUseRole::Conversion, parse)
}

pub(super) fn read_record_field<'scope, 'read, 'source, T>(
    fields: &OriginalFlowFields<'scope, 'read, 'source>,
    port: &OriginalValueFact<'scope, 'read, 'source>,
    name: &'static str,
    role: SourceUseRole,
    parse: impl FnOnce(&Value<'_>) -> Option<T>,
) -> Result<FieldFact<T>, CatalogError> {
    let mut inputs = Vec::new();
    let fact = fields.observe_field_declaration(port, name, &mut |input| {
        inputs.push(input);
        Ok(())
    })?;
    let Some(fact) = fact else {
        return Ok(FieldFact {
            value: SourceValue::Absent,
            inputs,
            complete: true,
        });
    };
    let state = match fact.value() {
        Value::Error(_) => SourceValue::Unreadable,
        Value::Null => SourceValue::Null,
        value => parse(value).map_or(SourceValue::Invalid, SourceValue::Present),
    };
    let (value, original, complete) = fact
        .bind(OriginalValuePart::Value, state, role, true)?
        .into_parts();
    inputs.extend(original);
    Ok(FieldFact {
        value,
        inputs,
        complete,
    })
}

pub(super) const fn text<'source>(value: &Value<'source>) -> Option<&'source str> {
    match value {
        Value::String(text) | Value::Locale(text) | Value::Enum(text) => Some(text),
        _ => None,
    }
}

fn tag_words(value: &str) -> Vec<String> {
    value.split_whitespace().map(str::to_owned).collect()
}

fn flag_words(value: &str) -> Vec<String> {
    value
        .split_whitespace()
        .map(|word| word.trim_start_matches('$'))
        .filter(|word| !word.is_empty())
        .map(str::to_owned)
        .collect()
}

#[cfg(test)]
#[path = "port_attribute_tests.rs"]
mod tests;
