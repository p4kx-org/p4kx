// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use p4kx_assembly::{PortDefinition, PortDefinitionPosition, VehicleAttributeEvidence};
use p4kx_extraction::{SourceUse, SourceUseRole, SourceValue, SourcedValue, StructValue, Value};

use super::Capture;
use crate::catalog_read::original::{
    AuthoredEntryPath, EntryCapture, OriginalFlowFields, OriginalValueEdge, OriginalValueFact,
    OriginalValuePart,
};
use crate::{CatalogError, ExportAccountingError, Kind};

struct PortSizes {
    min: SizeFact,
    max: SizeFact,
}

#[derive(Clone)]
struct SizeFact {
    value: SourceValue<u32>,
    inputs: Vec<SourceUse>,
    complete: bool,
}

#[derive(Default)]
pub(super) struct RecordPortSizes {
    definitions: HashMap<PortDefinitionPosition, PortSizes>,
}

impl Capture<'_> {
    pub(super) fn record_port_sizes(
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
        let selectors = selection(fields, component, port)?;
        let sizes = PortSizes {
            min: record_size(fields, port, "MinSize", definition.min_size(), &selectors)?,
            max: record_size(fields, port, "MaxSize", definition.max_size(), &selectors)?,
        };
        let mut state = self.lock()?;
        if state
            .port_sizes
            .definitions
            .insert(position, sizes)
            .is_some()
        {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        drop(state);
        Ok(())
    }

    pub(crate) fn definition_sizes(
        &self,
        definition: &PortDefinition,
        name: Option<&AuthoredEntryPath>,
    ) -> Result<(SourcedValue<u32>, SourcedValue<u32>), CatalogError> {
        if self.input.is_none() {
            let unsourced = |value: Option<u32>| {
                self.name_sources.value(
                    value.map_or(SourceValue::Absent, SourceValue::Present),
                    &[],
                    false,
                )
            };
            return Ok((
                unsourced(definition.min_size()),
                unsourced(definition.max_size()),
            ));
        }
        let (min, max) = {
            let state = self.lock()?;
            let sizes = state
                .port_sizes
                .definitions
                .get(&definition.position())
                .ok_or(ExportAccountingError::UnobservedFailure)?;
            let result = (sizes.min.clone(), sizes.max.clone());
            drop(state);
            result
        };
        let (selection, complete) = name.map_or_else(
            || Ok((Vec::new(), false)),
            AuthoredEntryPath::selector_inputs,
        )?;
        let with_selection = |mut size: SizeFact| {
            size.inputs.extend(selection.iter().cloned());
            self.name_sources
                .value(size.value, &size.inputs, complete && size.complete)
        };
        Ok((with_selection(min), with_selection(max)))
    }

    pub(crate) fn vehicle_size(
        &self,
        (value, evidence): (&SourceValue<u32>, Option<&VehicleAttributeEvidence>),
        name: Option<&AuthoredEntryPath>,
    ) -> Result<SourcedValue<u32>, CatalogError> {
        let (Some(input), Some(evidence)) = (self.input, evidence) else {
            return Ok(self.name_sources.value(value.clone(), &[], false));
        };
        let entries = EntryCapture::for_input(input);
        let mut inputs = name.map_or_else(
            || Ok(Vec::new()),
            |name| name.selector_inputs().map(|(selection, _)| selection),
        )?;
        let role = match value {
            SourceValue::Absent => SourceUseRole::Selector,
            _ => SourceUseRole::Conversion,
        };
        let state = self.lock()?;
        for (site, role) in std::iter::once((evidence.original(), role)).chain(
            evidence
                .selectors()
                .iter()
                .map(|site| (site, SourceUseRole::Selector)),
        ) {
            if !state.files.contains(site.file()) {
                return Err(ExportAccountingError::UnobservedFailure.into());
            }
            if site.file().source().entry_origin().is_some() {
                let (_, source, _) = entries.vehicle_xml_site(site, role)?.into_parts();
                inputs.extend(source);
            }
        }
        drop(state);
        // The selected attribute does not account for the preceding XML Part searches.
        Ok(self.name_sources.value(value.clone(), &inputs, false))
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

fn record_size<'scope, 'read, 'source>(
    fields: &OriginalFlowFields<'scope, 'read, 'source>,
    port: &OriginalValueFact<'scope, 'read, 'source>,
    name: &'static str,
    accepted: Option<u32>,
    selectors: &[SourceUse],
) -> Result<SizeFact, CatalogError> {
    let mut inputs = selectors.to_vec();
    let fact = fields.observe_field_declaration(port, name, &mut |input| {
        inputs.push(input);
        Ok(())
    })?;
    let Some(fact) = fact else {
        if accepted.is_some() {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        return Ok(SizeFact {
            value: SourceValue::Absent,
            inputs,
            complete: true,
        });
    };
    let state = match (fact.value(), accepted) {
        (Value::Error(_), None) => SourceValue::Unreadable,
        (Value::Null, None) => SourceValue::Null,
        (_, Some(size)) => SourceValue::Present(size),
        (_, None) => SourceValue::Invalid,
    };
    let (value, original, complete) = fact
        .bind(
            OriginalValuePart::Value,
            state,
            SourceUseRole::Conversion,
            true,
        )?
        .into_parts();
    inputs.extend(original);
    Ok(SizeFact {
        value,
        inputs,
        complete,
    })
}
