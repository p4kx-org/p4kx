// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    FieldOrigin, FieldOrigins, FieldPath, SourceUse, SourceUseRole, StructValue, Value,
};

use crate::catalog_read::original::{OriginalFlowFields, OriginalValueFact, OriginalValuePart};
use crate::export_field_error;
use crate::record_fields;
use crate::{CatalogError, ExportAccountingError};

type Fact<'scope, 'tree, 'source> = OriginalValueFact<'scope, 'tree, 'source>;

#[derive(Clone, Default)]
pub(crate) struct Inputs {
    uses: Vec<SourceUse>,
    incomplete: bool,
}

impl Inputs {
    fn select(&mut self, fact: &Fact<'_, '_, '_>) -> Result<(), CatalogError> {
        self.observe(fact, SourceUseRole::Selector)
    }

    fn observe(
        &mut self,
        fact: &Fact<'_, '_, '_>,
        role: SourceUseRole,
    ) -> Result<(), CatalogError> {
        let role = if matches!(fact.value(), Value::Error(error) if export_field_error::blocks_projection(*error))
        {
            self.incomplete = true;
            SourceUseRole::Attempted
        } else {
            role
        };
        self.uses
            .push(fact.observe(OriginalValuePart::Value, role)?);
        Ok(())
    }

    pub(crate) fn extend(&mut self, other: &Self) {
        self.uses.extend_from_slice(&other.uses);
        self.incomplete |= other.incomplete;
    }
}

pub(crate) struct Object<'tree, 'source> {
    pub(crate) structure: Option<&'tree StructValue<'source>>,
    pub(crate) inputs: Inputs,
}

pub(crate) struct Projected<T> {
    pub(crate) value: Option<T>,
    pub(crate) inputs: Inputs,
}

pub(crate) struct Projection<'facts, 'scope, 'tree, 'source> {
    fields: &'facts OriginalFlowFields<'scope, 'tree, 'source>,
    output: FieldPath,
    origins: FieldOrigins,
}

impl<'facts, 'scope, 'tree, 'source> Projection<'facts, 'scope, 'tree, 'source> {
    pub(crate) const fn new(fields: &'facts OriginalFlowFields<'scope, 'tree, 'source>) -> Self {
        Self {
            fields,
            output: FieldPath::root(),
            origins: FieldOrigins::new(),
        }
    }

    pub(crate) const fn root(&self) -> Object<'tree, 'source> {
        Object {
            structure: Some(self.fields.root()),
            inputs: Inputs {
                uses: Vec::new(),
                incomplete: false,
            },
        }
    }

    /// Retains the actual component selector prefix through its first matching concrete type.
    pub(crate) fn first_component(
        &self,
        type_name: &str,
    ) -> Result<Object<'tree, 'source>, CatalogError> {
        let (array, mut inputs) = self.array(&self.root(), "Components")?;
        for fact in array {
            inputs.select(fact)?;
            if let Value::Struct(structure) = fact.value() {
                inputs.uses.push(self.fields.observe_type(structure)?);
                if structure.type_name() == type_name {
                    return Ok(Object {
                        structure: Some(structure),
                        inputs,
                    });
                }
            }
        }
        Ok(Object {
            structure: None,
            inputs,
        })
    }

    pub(crate) fn finish(self) -> FieldOrigins {
        self.origins
    }

    pub(crate) fn object(
        &self,
        owner: &Object<'tree, 'source>,
        field: &str,
    ) -> Result<Object<'tree, 'source>, CatalogError> {
        let (fact, mut inputs) = self.field(owner, field)?;
        let structure = if let Some(fact) = fact {
            inputs.select(fact)?;
            match fact.value() {
                Value::Struct(structure) => Some(structure),
                _ => None,
            }
        } else {
            None
        };
        Ok(Object { structure, inputs })
    }

    pub(crate) fn scalar<T>(
        &mut self,
        owner: &Object<'tree, 'source>,
        field: &str,
        output: &str,
        convert: impl FnOnce(&Value<'source>) -> Option<T>,
    ) -> Result<Option<T>, CatalogError> {
        let value = self.value(owner, field, convert)?;
        self.named_origin(output, &value.inputs)?;
        Ok(value.value)
    }

    pub(crate) fn value<T>(
        &self,
        owner: &Object<'tree, 'source>,
        field: &str,
        convert: impl FnOnce(&Value<'source>) -> Option<T>,
    ) -> Result<Projected<T>, CatalogError> {
        let (fact, inputs) = self.field(owner, field)?;
        project_value(fact, inputs, convert, |original| match original {
            Value::Enum(_) => SourceUseRole::Conversion,
            _ => SourceUseRole::Payload,
        })
    }

    pub(crate) fn number(
        &self,
        owner: &Object<'tree, 'source>,
        field: &str,
    ) -> Result<Projected<f32>, CatalogError> {
        let (fact, inputs) = self.field(owner, field)?;
        project_value(fact, inputs, record_fields::number_value, |original| {
            if matches!(original, Value::Single(_)) {
                SourceUseRole::Payload
            } else {
                SourceUseRole::Conversion
            }
        })
    }

    pub(crate) fn structures<T>(
        &mut self,
        owner: &Object<'tree, 'source>,
        field: &str,
        output: &str,
        mut project: impl FnMut(&mut Self, &Object<'tree, 'source>) -> Result<T, CatalogError>,
    ) -> Result<Vec<T>, CatalogError> {
        let (array, base) = self.array(owner, field)?;
        self.in_field(output, |this| {
            let mut rendered = Vec::new();
            let mut membership = base;
            for fact in array {
                membership.select(fact)?;
                if let Value::Struct(structure) = fact.value() {
                    let object = Object {
                        structure: Some(structure),
                        inputs: membership.clone(),
                    };
                    rendered.push(this.in_index(rendered.len(), |this| project(this, &object))?);
                }
            }
            this.insert(this.origin(&membership))?;
            Ok(rendered)
        })
    }

    pub(crate) fn references(
        &mut self,
        owner: &Object<'tree, 'source>,
        field: &str,
        output: &str,
        shape: ReferenceElements,
    ) -> Result<Vec<crate::guid_text::CatalogGuid>, CatalogError> {
        let (array, mut membership) = self.array(owner, field)?;
        self.in_field(output, |this| {
            let mut rendered = Vec::new();
            for fact in array {
                let mut selected = Inputs::default();
                selected.select(fact)?;
                let (candidate, mut route) = match shape {
                    ReferenceElements::Direct => (Some(fact), selected),
                    ReferenceElements::ObjectField(tag) => {
                        let Value::Struct(structure) = fact.value() else {
                            membership.extend(&selected);
                            continue;
                        };
                        let child = Object {
                            structure: Some(structure),
                            inputs: selected,
                        };
                        this.field(&child, tag)?
                    }
                };
                if let Some(candidate) = candidate {
                    route.select(candidate)?;
                }
                membership.extend(&route);
                if let Some((candidate, guid)) = candidate.and_then(|candidate| {
                    record_fields::record_reference(candidate.value()).map(|guid| (candidate, guid))
                }) {
                    let mut inputs = membership.clone();
                    inputs.observe(candidate, SourceUseRole::Payload)?;
                    this.in_index(rendered.len(), |this| this.insert(this.origin(&inputs)))?;
                    rendered.push(guid);
                }
            }
            this.insert(this.origin(&membership))?;
            Ok(rendered)
        })
    }

    pub(crate) fn in_field<T>(
        &mut self,
        name: &str,
        project: impl FnOnce(&mut Self) -> Result<T, CatalogError>,
    ) -> Result<T, CatalogError> {
        self.output.push_field(name).map_err(path_error)?;
        let result = project(self);
        self.output.pop();
        result
    }

    fn in_index<T>(
        &mut self,
        index: usize,
        project: impl FnOnce(&mut Self) -> Result<T, CatalogError>,
    ) -> Result<T, CatalogError> {
        self.output.push_index(index).map_err(path_error)?;
        let result = project(self);
        self.output.pop();
        result
    }

    pub(crate) fn named_origin(
        &mut self,
        field: &str,
        inputs: &Inputs,
    ) -> Result<(), CatalogError> {
        self.named(field, self.origin(inputs))
    }

    pub(crate) fn named(&mut self, field: &str, origin: FieldOrigin) -> Result<(), CatalogError> {
        self.in_field(field, |this| this.insert(origin))
    }

    pub(crate) fn origin(&self, inputs: &Inputs) -> FieldOrigin {
        self.fields
            .sources()
            .origin(&inputs.uses, !inputs.incomplete)
    }

    fn insert(&mut self, origin: FieldOrigin) -> Result<(), CatalogError> {
        self.origins
            .insert(self.output.clone(), origin)
            .map_err(|_| ExportAccountingError::ConflictingSourceDatum.into())
    }

    fn field(
        &self,
        owner: &Object<'tree, 'source>,
        field: &str,
    ) -> Result<(Option<&'facts Fact<'scope, 'tree, 'source>>, Inputs), CatalogError> {
        let mut inputs = owner.inputs.clone();
        let Some(structure) = owner.structure else {
            return Ok((None, inputs));
        };
        let parent = self.fields.owner_of(structure)?;
        let Some((_, fact)) = self.fields.field(parent, field)? else {
            inputs.uses.push(self.fields.survey(parent)?);
            return Ok((None, inputs));
        };
        inputs
            .uses
            .push(fact.observe(OriginalValuePart::FieldName, SourceUseRole::Selector)?);
        Ok((Some(fact), inputs))
    }

    fn array(
        &self,
        owner: &Object<'tree, 'source>,
        field: &str,
    ) -> Result<(Vec<&'facts Fact<'scope, 'tree, 'source>>, Inputs), CatalogError> {
        let (fact, mut inputs) = self.field(owner, field)?;
        let array = if let Some(fact) = fact {
            inputs.select(fact)?;
            self.fields.elements(fact)?
        } else {
            Vec::new()
        };
        Ok((array, inputs))
    }
}

#[derive(Clone, Copy)]
pub(crate) enum ReferenceElements {
    Direct,
    ObjectField(&'static str),
}

fn project_value<'source, T>(
    fact: Option<&Fact<'_, '_, 'source>>,
    mut inputs: Inputs,
    convert: impl FnOnce(&Value<'source>) -> Option<T>,
    role: impl FnOnce(&Value<'source>) -> SourceUseRole,
) -> Result<Projected<T>, CatalogError> {
    let value = if let Some(fact) = fact {
        let original = fact.value();
        let value = convert(original);
        let contribution = if value.is_some() {
            role(original)
        } else {
            SourceUseRole::Selector
        };
        inputs.observe(fact, contribution)?;
        value
    } else {
        None
    };
    Ok(Projected { value, inputs })
}

fn path_error(error: p4kx_extraction::FieldPathError) -> CatalogError {
    match error {
        p4kx_extraction::FieldPathError::Allocation(source) => {
            CatalogError::JsonSourceAllocation(source)
        }
        _ => ExportAccountingError::CounterOverflow.into(),
    }
}
