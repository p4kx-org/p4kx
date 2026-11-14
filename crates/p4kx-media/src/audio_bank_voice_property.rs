// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourceUseRole;
use p4kx_extraction::bank::{BankHircField, BankNodePropertyRow, BankNodePropertyValue};
use p4kx_extraction::bank::{VoiceProperty, VoicePropertyName};
use schemars::JsonSchema;
use serde::Serialize;

use crate::MediaError;
use crate::audio_bank_node_origins::NodeSites;
use crate::audio_bank_origins::BankOrigins;
use crate::origin_capture::OriginSite;

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct PropertyInterpretation {
    property: VoicePropertyName,
    #[serde(skip_serializing_if = "Option::is_none")]
    scalar: Option<PropertyValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    minimum: Option<PropertyValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    maximum: Option<PropertyValue>,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct PropertyValue {
    value: Option<f32>,
}

pub(crate) fn project(
    sites: &NodeSites,
    prepared: &mut BankOrigins<'_, '_, '_>,
    row: &BankNodePropertyRow,
) -> Result<Option<PropertyInterpretation>, MediaError> {
    let Some(property) = VoiceProperty::from_node_property(sites.version(), row.id.value) else {
        return Ok(None);
    };
    let Some(name) = VoicePropertyName::from_property(property) else {
        return Ok(None);
    };
    let id = [row.id.range.start, row.id.range.end];
    let selector = (
        OriginSite::new("bnkHircNodeProperties", &id, "nodePropertyId"),
        SourceUseRole::Selector,
    );
    sites.converted(
        prepared,
        "interpretation/property",
        &row.id.range,
        "nodePropertyType",
    )?;
    let mut observe = |name, word: Option<&BankHircField<u32>>| {
        word.map(|word| {
            let extent = [word.range.start, word.range.end];
            let used = (
                OriginSite::new("bnkHircNodeProperties", &extent, "nodePropertyWord"),
                SourceUseRole::Conversion,
            );
            let origin =
                prepared.capture(sites.property_context().into_iter().chain([selector, used]))?;
            prepared.subtree(name, origin)?;
            Ok::<_, MediaError>(PropertyValue {
                value: property.finite_value(word.value),
            })
        })
        .transpose()
    };
    let (scalar, minimum, maximum) = match &row.value {
        BankNodePropertyValue::Scalar(bits) => (
            observe("/data/payload/interpretation/scalar", bits.as_ref())?,
            None,
            None,
        ),
        BankNodePropertyValue::Ranged {
            minimum, maximum, ..
        } => (
            None,
            observe("/data/payload/interpretation/minimum", minimum.as_ref())?,
            observe("/data/payload/interpretation/maximum", maximum.as_ref())?,
        ),
        _ => {
            return Err(MediaError::Contract(
                "unexpected original property layout".into(),
            ));
        }
    };
    Ok(Some(PropertyInterpretation {
        property: name,
        scalar,
        minimum,
        maximum,
    }))
}
