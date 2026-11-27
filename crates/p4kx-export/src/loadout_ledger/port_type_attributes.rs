// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::PortDefinition;
use p4kx_extraction::{
    SharedFieldOrigin, SourceUse, SourceUseRole, SourceUses, SourceValue, SourcedValue, Value,
};

use super::port_attributes::{FieldFact, read_record_field, text};
use crate::CatalogError;
use crate::catalog_read::original::{OriginalFlowFields, OriginalValueFact, OriginalValuePart};
use crate::contract_loadout::{PortSubTypes, PortType};

pub(super) struct RecordPortTypes(FieldFact<Vec<RecordPortType>>);

struct RecordPortType {
    position: SourceUse,
    name: FieldFact<String>,
    sub_types: FieldFact<Vec<RecordSubType>>,
}

struct RecordSubType {
    value: String,
    origin: SourceUse,
}

impl RecordPortTypes {
    #[cfg(test)]
    pub(super) fn test_absent(declaration: SourceUse) -> Self {
        Self(FieldFact {
            value: SourceValue::Absent,
            inputs: vec![declaration],
            complete: true,
        })
    }

    pub(super) fn read<'scope, 'read, 'source>(
        fields: &OriginalFlowFields<'scope, 'read, 'source>,
        port: &OriginalValueFact<'scope, 'read, 'source>,
    ) -> Result<Self, CatalogError> {
        collection(fields, port, "Types", RecordPortType::read).map(Self)
    }

    pub(super) fn bind(
        &self,
        sources: &SourceUses,
        port: &SharedFieldOrigin,
    ) -> Result<SourcedValue<Vec<PortType>>, CatalogError> {
        self.0.bind_map(sources, port, |items| {
            items.iter().map(|item| item.bind(sources, port)).collect()
        })
    }

    pub(super) fn unbound(
        definition: &PortDefinition,
        sources: &SourceUses,
    ) -> SourcedValue<Vec<PortType>> {
        let items = definition
            .types()
            .iter()
            .map(|kind| PortType {
                r#type: sources.value(SourceValue::Present(kind.name().to_owned()), &[], false),
                sub_types: sources.value(
                    SourceValue::Present(PortSubTypes::shared(
                        kind.sub_types().to_vec(),
                        sources.value(SourceValue::Present(()), &[], false),
                    )),
                    &[],
                    false,
                ),
            })
            .collect();
        sources.value(SourceValue::Present(items), &[], false)
    }
}

impl RecordPortType {
    fn read<'scope, 'read, 'source>(
        fields: &OriginalFlowFields<'scope, 'read, 'source>,
        item: &OriginalValueFact<'scope, 'read, 'source>,
    ) -> Result<Option<Self>, CatalogError> {
        if !matches!(item.value(), Value::Struct(_)) {
            return Ok(None);
        }
        let position = item.observe(OriginalValuePart::Value, SourceUseRole::Selector)?;
        let name = read_record_field(fields, item, "Type", SourceUseRole::Payload, |raw| {
            text(raw).map(str::to_owned)
        })?;
        let sub_types = collection(fields, item, "SubTypes", |_, subtype| {
            let Some(value) = text(subtype.value()).filter(|text| !text.is_empty()) else {
                return Ok(None);
            };
            Ok(Some(RecordSubType {
                value: value.to_owned(),
                origin: subtype.observe(OriginalValuePart::Value, SourceUseRole::Payload)?,
            }))
        })?;
        Ok(Some(Self {
            position,
            name,
            sub_types,
        }))
    }

    fn bind(
        &self,
        sources: &SourceUses,
        port: &SharedFieldOrigin,
    ) -> Result<PortType, CatalogError> {
        let position = sources.origin(std::slice::from_ref(&self.position), true);
        let sub_types = self.sub_types.bind_map(sources, port, |subtypes| {
            PortSubTypes::separate(
                subtypes
                    .iter()
                    .map(|subtype| self.bind_subtype(subtype, sources, port)),
            )
        })?;
        Ok(PortType {
            r#type: sources.add_value_origins(self.name.bind(sources, port)?, &[&position])?,
            sub_types: sources.add_value_origins(sub_types, &[&position])?,
        })
    }

    fn bind_subtype(
        &self,
        subtype: &RecordSubType,
        sources: &SourceUses,
        port: &SharedFieldOrigin,
    ) -> Result<(String, SourcedValue<()>), CatalogError> {
        let origin = sources.value_with_shared_origin(
            SourceValue::Present(()),
            port,
            &[self.position.clone(), subtype.origin.clone()],
            true,
        )?;
        Ok((subtype.value.clone(), origin))
    }
}

fn collection<'scope, 'read, 'source, T>(
    fields: &OriginalFlowFields<'scope, 'read, 'source>,
    owner: &OriginalValueFact<'scope, 'read, 'source>,
    name: &'static str,
    mut read: impl FnMut(
        &OriginalFlowFields<'scope, 'read, 'source>,
        &OriginalValueFact<'scope, 'read, 'source>,
    ) -> Result<Option<T>, CatalogError>,
) -> Result<FieldFact<Vec<T>>, CatalogError> {
    let mut inputs = Vec::new();
    let fact = fields.observe_field_selection(owner, name, &mut |input| {
        inputs.push(input);
        Ok(())
    })?;
    let mut complete = true;
    let value = match fact.map(|fact| (fact, fact.value())) {
        Some((array, Value::Array(_))) => {
            let mut items = Vec::new();
            for item in fields.elements(array)? {
                let role = if matches!(item.value(), Value::Error(_)) {
                    complete = false;
                    continue;
                } else if let Some(item) = read(fields, item)? {
                    items.push(item);
                    SourceUseRole::Payload
                } else {
                    SourceUseRole::Selector
                };
                inputs.push(item.observe(OriginalValuePart::Value, role)?);
            }
            SourceValue::Present(items)
        }
        Some((_, Value::Null)) => SourceValue::Null,
        Some((_, Value::Error(_))) => SourceValue::Unreadable,
        Some(_) => SourceValue::Unsupported,
        None => SourceValue::Absent,
    };
    Ok(FieldFact {
        value,
        inputs,
        complete,
    })
}
