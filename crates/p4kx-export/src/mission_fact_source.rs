// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Guid, SourceUseRole, SourceUses, StructValue, Value};

use super::choice::{Choice, Evidence, Selected};
use crate::catalog_read::original::{OriginalFlowFields, OriginalValueFact};
use crate::contract_facts::FactLayer;
use crate::mission_layers::items_value;
use crate::record_fields::{
    boolean_value, integer_value, record_reference, single_value, text_value,
};
use crate::{CatalogError, ExportAccountingError};

type Fact<'fields, 'scope, 'read, 'source> = &'fields OriginalValueFact<'scope, 'read, 'source>;
type Decode<T> = fn(&Value<'_>) -> Option<T>;

pub(crate) struct Field<T> {
    name: &'static str,
    overrides: &'static str,
    parameter: Decode<T>,
    default: Decode<T>,
    parameter_role: SourceUseRole,
}

impl Field<bool> {
    pub(crate) const fn flag(name: &'static str) -> Self {
        Self {
            name,
            overrides: "boolParamOverrides",
            parameter: boolean_value,
            default: boolean_value,
            parameter_role: SourceUseRole::Payload,
        }
    }
}

impl Field<i64> {
    pub(super) const fn integer(name: &'static str) -> Self {
        Self {
            name,
            overrides: "intParamOverrides",
            parameter: integer_value,
            default: integer_value,
            parameter_role: SourceUseRole::Payload,
        }
    }
}

impl Field<f32> {
    pub(crate) const fn duration(name: &'static str) -> Self {
        Self {
            name,
            overrides: "intParamOverrides",
            parameter: duration_parameter,
            default: single_value,
            parameter_role: SourceUseRole::Conversion,
        }
    }
}

#[expect(
    clippy::cast_precision_loss,
    reason = "The cooldown projection converts authored integer overrides to f32."
)]
fn duration_parameter(value: &Value<'_>) -> Option<f32> {
    integer_value(value).map(|value| value as f32)
}

pub(crate) struct Layer<'fields, 'scope, 'read, 'source> {
    parameters: Option<Fact<'fields, 'scope, 'read, 'source>>,
    kind: FactLayer,
    evidence: Evidence,
}

/// Holds a layer until an unresolved field actually reaches it.
struct DeferredLayer<'owner, 'fields, 'scope, 'read, 'source> {
    owner: &'owner StructValue<'source>,
    name: &'static str,
    kind: FactLayer,
    retained: Option<Layer<'fields, 'scope, 'read, 'source>>,
}

impl<'owner, 'fields, 'scope, 'read, 'source>
    DeferredLayer<'owner, 'fields, 'scope, 'read, 'source>
{
    const fn new(owner: &'owner StructValue<'source>, name: &'static str, kind: FactLayer) -> Self {
        Self {
            owner,
            name,
            kind,
            retained: None,
        }
    }

    fn read(
        &mut self,
        reader: &Reader<'fields, 'scope, 'read, 'source>,
    ) -> Result<&Layer<'fields, 'scope, 'read, 'source>, CatalogError> {
        match &mut self.retained {
            Some(layer) => Ok(layer),
            empty => Ok(empty.insert(reader.layer(self.owner, self.name, self.kind)?)),
        }
    }
}

/// The contract and handler layers, read only when a fact reaches them.
pub(crate) struct ParentLayers<'owner, 'fields, 'scope, 'read, 'source> {
    overrides: [DeferredLayer<'owner, 'fields, 'scope, 'read, 'source>; 2],
    defaults: DeferredLayer<'owner, 'fields, 'scope, 'read, 'source>,
}

impl<'owner, 'source> ParentLayers<'owner, '_, '_, '_, 'source> {
    pub(crate) const fn new(
        handler: &'owner StructValue<'source>,
        contract: &'owner StructValue<'source>,
    ) -> Self {
        Self {
            overrides: [
                DeferredLayer::new(contract, "paramOverrides", FactLayer::Contract),
                DeferredLayer::new(handler, "contractParams", FactLayer::Handler),
            ],
            defaults: DeferredLayer::new(handler, "defaultAvailability", FactLayer::Handler),
        }
    }
}

pub(super) struct TemplateCaller {
    pub(super) guid: Option<Guid>,
    pub(super) evidence: Evidence,
}

pub(crate) struct Reader<'fields, 'scope, 'read, 'source> {
    fields: &'fields OriginalFlowFields<'scope, 'read, 'source>,
}

impl<'fields, 'scope, 'read, 'source> Reader<'fields, 'scope, 'read, 'source> {
    pub(crate) const fn new(fields: &'fields OriginalFlowFields<'scope, 'read, 'source>) -> Self {
        Self { fields }
    }

    pub(crate) const fn sources(&self) -> &SourceUses {
        self.fields.sources()
    }

    pub(crate) fn sub(
        &self,
        source: &StructValue<'source>,
    ) -> Result<Layer<'fields, 'scope, 'read, 'source>, CatalogError> {
        self.layer(source, "paramOverrides", FactLayer::SubContract)
    }

    fn layer(
        &self,
        source: &StructValue<'source>,
        name: &str,
        kind: FactLayer,
    ) -> Result<Layer<'fields, 'scope, 'read, 'source>, CatalogError> {
        let owner = self
            .fields
            .owner_of(source)?
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        let mut evidence = Evidence::default();
        let parameters = if let Some(container) = self.field(owner, name, &mut evidence)? {
            self.first_struct(container, &mut evidence)?
        } else {
            None
        };
        let parameters = parameters.or_else(|| (kind == FactLayer::SubContract).then_some(owner));
        Ok(Layer {
            parameters,
            kind,
            evidence,
        })
    }

    pub(crate) fn parent<T: Copy>(
        &self,
        field: &Field<T>,
        layers: &mut ParentLayers<'_, 'fields, 'scope, 'read, 'source>,
    ) -> Result<Choice<T>, CatalogError> {
        let ParentLayers {
            overrides,
            defaults,
        } = layers;
        self.with_fallback(field, overrides, |evidence| {
            self.default_value(field, defaults.read(self)?, evidence)
        })
    }

    pub(super) fn with_template(
        &self,
        field: &Field<bool>,
        layers: &mut ParentLayers<'_, 'fields, 'scope, 'read, 'source>,
        fallback: impl FnOnce(&mut Evidence) -> Result<Option<Selected<bool>>, CatalogError>,
    ) -> Result<Choice<bool>, CatalogError> {
        self.with_fallback(field, &mut layers.overrides, fallback)
    }

    fn with_fallback<T: Copy>(
        &self,
        field: &Field<T>,
        overrides: &mut [DeferredLayer<'_, 'fields, 'scope, 'read, 'source>],
        fallback: impl FnOnce(&mut Evidence) -> Result<Option<Selected<T>>, CatalogError>,
    ) -> Result<Choice<T>, CatalogError> {
        let mut evidence = Evidence::default();
        for layer in overrides {
            if let Some(selected) = self.override_value(field, layer.read(self)?, &mut evidence)? {
                return Ok(Choice::new(Some(selected), evidence));
            }
        }
        let selected = fallback(&mut evidence)?;
        Ok(Choice::new(selected, evidence))
    }

    pub(super) fn template_caller(
        &self,
        contract: &StructValue<'source>,
    ) -> Result<TemplateCaller, CatalogError> {
        let owner = self
            .fields
            .owner_of(contract)?
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        let mut evidence = Evidence::default();
        let guid = self
            .field(owner, "template", &mut evidence)?
            .and_then(|fact| record_reference(fact.value()))
            .map(|guid| guid.0);
        Ok(TemplateCaller { guid, evidence })
    }

    pub(crate) fn child<T: Copy>(
        &self,
        field: &Field<T>,
        layer: &Layer<'fields, 'scope, 'read, 'source>,
        parent: &Choice<T>,
    ) -> Result<Choice<T>, CatalogError> {
        let mut evidence = Evidence::default();
        let selected = self.override_value(field, layer, &mut evidence)?;
        Choice::inherit(selected, evidence, parent)
    }

    fn first_struct(
        &self,
        container: Fact<'fields, 'scope, 'read, 'source>,
        evidence: &mut Evidence,
    ) -> Result<Option<Fact<'fields, 'scope, 'read, 'source>>, CatalogError> {
        self.fields.first_struct_observed(container, |value| {
            evidence.consult(self.fields.fact(value)?)
        })
    }

    fn field(
        &self,
        parent: Fact<'fields, 'scope, 'read, 'source>,
        name: &str,
        evidence: &mut Evidence,
    ) -> Result<Option<Fact<'fields, 'scope, 'read, 'source>>, CatalogError> {
        if let Some((_, fact)) = self.fields.field(Some(parent), name)? {
            evidence.consult(fact)?;
            return Ok(Some(fact));
        }
        evidence.retain(self.fields.survey(Some(parent))?)?;
        Ok(None)
    }

    fn override_value<T>(
        &self,
        field: &Field<T>,
        layer: &Layer<'fields, 'scope, 'read, 'source>,
        evidence: &mut Evidence,
    ) -> Result<Option<Selected<T>>, CatalogError> {
        evidence.extend(&layer.evidence)?;
        let Some(parameters) = layer.parameters else {
            return Ok(None);
        };
        let Some(container) = self.field(parameters, field.overrides, evidence)? else {
            return Ok(None);
        };
        for value in items_value(container.value()) {
            let candidate = self.fields.fact(value)?;
            if !matches!(value, Value::Struct(_)) {
                evidence.consult(candidate)?;
                continue;
            }
            let name = self
                .field(candidate, "param", evidence)?
                .and_then(|fact| text_value(fact.value()));
            if !name.is_some_and(|name| name.eq_ignore_ascii_case(field.name)) {
                continue;
            }
            if let Some(fact) = self.field(candidate, "value", evidence)?
                && let Some(value) = (field.parameter)(fact.value())
            {
                return Selected::read(value, layer.kind, fact, field.parameter_role).map(Some);
            }
        }
        Ok(None)
    }

    fn default_value<T>(
        &self,
        field: &Field<T>,
        defaults: &Layer<'fields, 'scope, 'read, 'source>,
        evidence: &mut Evidence,
    ) -> Result<Option<Selected<T>>, CatalogError> {
        evidence.extend(&defaults.evidence)?;
        if let Some(availability) = defaults.parameters
            && let Some(fact) = self.field(availability, field.name, evidence)?
            && let Some(value) = (field.default)(fact.value())
        {
            return Selected::read(value, defaults.kind, fact, SourceUseRole::Payload).map(Some);
        }
        Ok(None)
    }
}
