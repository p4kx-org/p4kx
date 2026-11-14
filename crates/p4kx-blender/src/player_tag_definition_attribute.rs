// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::cryxml::SourceElement;
use p4kx_extraction::{SourceUse, SourceUseRole, SourceValue, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use crate::soc_scene::SocSceneError;
use crate::soc_scene_origin::{SocXmlPosition, XmlOrigin};

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(in super::super) struct Attribute {
    value: SourcedValue<String>,
    occurrences: Vec<AttributeOccurrence>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct AttributeOccurrence {
    position: SocXmlPosition,
    value: SourcedValue<String>,
}

#[derive(Clone)]
pub(in super::super) struct PendingAttribute {
    field: &'static str,
    values: Vec<(SocXmlPosition, String)>,
}

impl Attribute {
    pub(in super::super) const fn value(&self) -> &SourcedValue<String> {
        &self.value
    }
}

impl PendingAttribute {
    pub(in super::super) const fn is_declared(&self) -> bool {
        !self.values.is_empty()
    }

    pub(in super::super) fn read(
        node: SourceElement<'_>,
        field: &'static str,
    ) -> Result<Self, SocSceneError> {
        let values = node
            .attributes()
            .filter(|a| a.namespace().is_none() && a.name() == field)
            .map(|a| {
                Ok((
                    SocXmlPosition::attribute(a.position())?,
                    a.value().to_owned(),
                ))
            })
            .collect::<Result<_, SocSceneError>>()?;
        Ok(Self { field, values })
    }

    pub(in super::super) fn bind(
        &self,
        origin: &XmlOrigin<'_>,
        owner: &SourceUse,
        role: SourceUseRole,
    ) -> Result<(Attribute, Vec<SourceUse>), SocSceneError> {
        let mut uses = vec![owner.clone()];
        let mut occurrences = Vec::with_capacity(self.values.len());
        for (position, value) in &self.values {
            let evidence = origin.at(position, self.field, role)?;
            occurrences.push(AttributeOccurrence {
                position: position.clone(),
                value: origin.sources.value(
                    SourceValue::Present(value.clone()),
                    std::slice::from_ref(&evidence),
                    true,
                ),
            });
            uses.push(evidence);
        }
        let value = origin.sources.value(self.value(), &uses, true);
        Ok((Attribute { value, occurrences }, uses))
    }

    pub(in super::super) fn value(&self) -> SourceValue<String> {
        match self.values.as_slice() {
            [] => SourceValue::Absent,
            [(_, value)] => SourceValue::Present(value.clone()),
            _ => SourceValue::Invalid,
        }
    }
}
