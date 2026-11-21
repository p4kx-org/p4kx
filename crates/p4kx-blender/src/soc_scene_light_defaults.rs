// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use p4kx_extraction::cryxml::SourceElement;
use p4kx_extraction::{
    Location, SourceLocation, SourceScope, SourceUse, SourceUseRole, SourceUses, SourceValue,
    SourcedValue, Step, StructValue,
};
use schemars::JsonSchema;
use serde::Serialize;

use crate::soc_scene::SocSceneError;
use crate::soc_scene_light_override::{self, Override};
use crate::soc_scene_light_value::LightValue;
use crate::soc_scene_origin::XmlOrigin;

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DefaultComponent {
    ordinal: usize,
    fields: Vec<DefaultField>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct DefaultField {
    ordinal: usize,
    path: String,
    #[serde(skip)]
    #[schemars(skip)]
    xml_path: Option<Vec<String>>,
    value: SourcedValue<LightValue>,
    #[serde(skip)]
    #[schemars(skip)]
    use_at: SourceUse,
}

impl DefaultComponent {
    pub(crate) fn read(
        record: u32,
        ordinal: usize,
        component: &StructValue<'_>,
        scope: &SourceScope,
        sources: &SourceUses,
    ) -> Result<Self, SocSceneError> {
        let mut fields = Vec::new();
        component.visit_owned_values(|visit| {
            let Some(value) = LightValue::class(visit.value()) else {
                return;
            };
            let names = visit
                .steps()
                .iter()
                .map(|step| match step {
                    Step::Field(name) => Some((*name).to_owned()),
                    Step::Element(_) => None,
                })
                .collect::<Option<Vec<_>>>();
            let coordinate = Location::new(visit.steps());
            let position = visit.ordinal();
            let result = scope
                .at(
                    SourceLocation::Subresource {
                        namespace: "dataForgeLightClass".to_owned(),
                        indices: vec![u64::from(record), ordinal as u64, position as u64],
                        field: coordinate.as_str().to_owned(),
                    },
                    SourceUseRole::Payload,
                )
                .map(|used| {
                    let complete = !matches!(value, SourceValue::Unreadable);
                    DefaultField {
                        ordinal: position,
                        path: coordinate.as_str().to_owned(),
                        xml_path: names,
                        value: sources.value(value, std::slice::from_ref(&used), complete),
                        use_at: used,
                    }
                });
            fields.push(result);
        });
        Ok(Self {
            ordinal,
            fields: fields
                .into_iter()
                .collect::<Result<_, _>>()
                .map_err(SocSceneError::from)?,
        })
    }

    pub(crate) fn resolve(
        &self,
        light: SourceElement<'_>,
        origin: &XmlOrigin<'_>,
        selector: &[SourceUse],
    ) -> Result<Vec<EffectiveField>, SocSceneError> {
        let mut by_path = BTreeMap::<_, Vec<_>>::new();
        for field in &self.fields {
            if let Some(path) = &field.xml_path {
                by_path.entry(path.as_slice()).or_default().push(field);
            }
        }
        by_path
            .into_iter()
            .map(|(path, fields)| {
                if let [field] = fields.as_slice() {
                    return field.resolve(path, light, origin, selector);
                }
                let mut uses = selector.to_vec();
                uses.extend(fields.iter().map(|field| field.use_at.clone()));
                Ok(EffectiveField {
                    path: path.to_vec(),
                    selection: FieldSelection::ConflictingDefault,
                    value: origin.sources.value(SourceValue::Invalid, &uses, true),
                })
            })
            .collect()
    }
}

#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct EffectiveField {
    path: Vec<String>,
    selection: FieldSelection,
    value: SourcedValue<LightValue>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
enum FieldSelection {
    Authored,
    Inherited,
    ConflictingOverride,
    ConflictingDefault,
}

impl DefaultField {
    fn resolve(
        &self,
        path: &[String],
        light: SourceElement<'_>,
        origin: &XmlOrigin<'_>,
        selector: &[SourceUse],
    ) -> Result<EffectiveField, SocSceneError> {
        let mut uses = selector.to_vec();
        uses.push(self.use_at.clone());
        let (value, selection) = match soc_scene_light_override::find(light, path) {
            Some(Override::Value(attribute)) => {
                uses.push(origin.attribute(&attribute, SourceUseRole::Payload)?.1);
                let parsed = self
                    .value
                    .value()
                    .and_then(|value| value.parse(attribute.value()));
                (
                    parsed.map_or(SourceValue::Invalid, SourceValue::Present),
                    FieldSelection::Authored,
                )
            }
            Some(Override::RepeatedNodes(nodes)) => {
                for node in nodes {
                    uses.push(origin.node(node, SourceUseRole::Payload)?);
                }
                (SourceValue::Invalid, FieldSelection::ConflictingOverride)
            }
            Some(Override::RepeatedAttributes(attributes)) => {
                for attribute in attributes {
                    uses.push(origin.attribute(&attribute, SourceUseRole::Payload)?.1);
                }
                (SourceValue::Invalid, FieldSelection::ConflictingOverride)
            }
            Some(Override::Absent(parent)) => {
                uses.push(origin.node(parent, SourceUseRole::Selector)?);
                (source_state(&self.value), FieldSelection::Inherited)
            }
            None => return Err(SocSceneError::Position),
        };
        let complete = !matches!(value, SourceValue::Unreadable);
        Ok(EffectiveField {
            path: path.to_vec(),
            selection,
            value: origin.sources.value(value, &uses, complete),
        })
    }
}

fn source_state(value: &SourcedValue<LightValue>) -> SourceValue<LightValue> {
    use p4kx_extraction::SourceValueState;
    match value.state() {
        SourceValueState::Present => value
            .value()
            .cloned()
            .map_or(SourceValue::Invalid, SourceValue::Present),
        SourceValueState::Absent => SourceValue::Absent,
        SourceValueState::Null => SourceValue::Null,
        SourceValueState::Unreadable => SourceValue::Unreadable,
        SourceValueState::Unsupported => SourceValue::Unsupported,
        _ => SourceValue::Invalid,
    }
}
