// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use p4kx_assembly::{PortDefinition, PortDefinitionPosition, VehiclePortSelection};
use p4kx_extraction::{
    Archive, SourceUse, SourceUseRole, SourceValue, SourcedValue, StructValue, Value,
};

use super::Capture;
use super::port_vehicle_sources::VehiclePortSources;
use crate::catalog_read::original::{
    AuthoredEntryPath, OriginalFlowFields, OriginalValueEdge, OriginalValueFact, OriginalValuePart,
};
use crate::{CatalogError, ExportAccountingError, Kind};

#[derive(Clone)]
struct DefinitionFacts {
    selectors: Vec<SourceUse>,
    min: FieldFact<u32>,
    max: FieldFact<u32>,
    flags: FieldFact<Vec<String>>,
    port_tags: FieldFact<Vec<String>>,
    required_tags: FieldFact<Vec<String>>,
    display_name_key: FieldFact<String>,
}

#[derive(Clone)]
struct FieldFact<T> {
    value: SourceValue<T>,
    inputs: Vec<SourceUse>,
    complete: bool,
}

#[derive(Default)]
pub(super) struct RecordPortAttributes {
    definitions: HashMap<PortDefinitionPosition, DefinitionFacts>,
}

pub(crate) struct PortAttributes {
    pub(crate) min_size: SourcedValue<u32>,
    pub(crate) max_size: SourcedValue<u32>,
    pub(crate) flags: SourcedValue<Vec<String>>,
    pub(crate) port_tags: SourcedValue<Vec<String>>,
    pub(crate) required_tags: SourcedValue<Vec<String>>,
    pub(crate) display_name_key: SourcedValue<String>,
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
        };
        let mut state = self.lock()?;
        if state
            .port_attributes
            .definitions
            .insert(position, attributes)
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
            });
        }
        let facts = {
            let state = self.lock()?;
            let facts = state
                .port_attributes
                .definitions
                .get(&definition.position())
                .ok_or(ExportAccountingError::UnobservedFailure)?
                .clone();
            drop(state);
            facts
        };
        let (selection, complete) = name.map_or_else(
            || Ok((Vec::new(), false)),
            AuthoredEntryPath::selector_inputs,
        )?;
        let mut selectors = facts.selectors;
        selectors.extend(selection);
        Ok(PortAttributes {
            min_size: self.definition_field(facts.min, &selectors, complete),
            max_size: self.definition_field(facts.max, &selectors, complete),
            flags: self.definition_field(facts.flags, &selectors, complete),
            port_tags: self.definition_field(facts.port_tags, &selectors, complete),
            required_tags: self.definition_field(facts.required_tags, &selectors, complete),
            display_name_key: self.definition_field(facts.display_name_key, &selectors, complete),
        })
    }

    fn definition_field<T>(
        &self,
        mut fact: FieldFact<T>,
        selectors: &[SourceUse],
        complete: bool,
    ) -> SourcedValue<T> {
        fact.inputs.extend_from_slice(selectors);
        self.name_sources
            .value(fact.value, &fact.inputs, complete && fact.complete)
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
        };
        drop(state);
        Ok(attributes)
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

fn read_record_field<'scope, 'read, 'source, T>(
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

const fn text<'source>(value: &Value<'source>) -> Option<&'source str> {
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
