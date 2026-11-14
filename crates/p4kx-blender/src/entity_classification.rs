// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::Part;
use p4kx_extraction::{
    DataForgeOrigin, InputScope, OriginReaderLimits, SourceInput, SourceLocation, SourceScope,
    SourceUseRole, SourceUses, SourceValue, SourcedValue, SourcedValueSeed,
};
use schemars::JsonSchema;
use serde::de::DeserializeSeed as _;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

use crate::{Manifest, PackageError, PackageResult};

/// The authored classification of an Entity node and its immediate origins.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EntityClassification {
    /// Original `AttachDef` Type.
    #[serde(deserialize_with = "read_text")]
    pub item_type: SourcedValue<String>,
    /// Original `AttachDef` `SubType`.
    #[serde(deserialize_with = "read_text")]
    pub subtype: SourcedValue<String>,
}

pub(crate) struct EntityClassifications {
    nodes: Vec<Option<EntityClassification>>,
    provenance: Option<Value>,
}

impl EntityClassifications {
    pub(crate) fn capture<'a>(
        parts: impl IntoIterator<Item = &'a Part>,
        origin: Option<&DataForgeOrigin>,
    ) -> PackageResult<Self> {
        let uses = SourceUses::with_compact_use_refs();
        let scope = match origin {
            Some(DataForgeOrigin::Archive(entry)) => Some(SourceInput::Archive(entry)),
            Some(DataForgeOrigin::File(file)) => Some(SourceInput::File(file)),
            _ => None,
        }
        .map(|source| uses.observe(source, InputScope::Payload))
        .transpose()
        .map_err(evidence)?;
        let nodes = parts
            .into_iter()
            .map(|part| {
                part.classification()
                    .map(|selected| {
                        Ok(EntityClassification {
                            item_type: bind(
                                &uses,
                                scope.as_ref(),
                                selected.item_type_location(),
                                selected.item_type(),
                            )?,
                            subtype: bind(
                                &uses,
                                scope.as_ref(),
                                selected.subtype_location(),
                                selected.subtype(),
                            )?,
                        })
                    })
                    .transpose()
            })
            .collect::<PackageResult<Vec<_>>>()?;
        let provenance = nodes
            .iter()
            .any(Option::is_some)
            .then(|| serde_json::to_value(uses.freeze().map_err(evidence)?).map_err(evidence))
            .transpose()?;
        Ok(Self { nodes, provenance })
    }

    pub(crate) fn apply(self, manifest: &mut Manifest) {
        for (node, classification) in manifest.nodes.iter_mut().zip(self.nodes) {
            if let Some(entity) = &mut node.entity {
                entity.classification = classification;
            }
        }
        manifest.entity_provenance = self.provenance;
    }
}

fn bind(
    uses: &SourceUses,
    scope: Option<&SourceScope>,
    location: Option<SourceLocation>,
    value: &SourceValue<String>,
) -> PackageResult<SourcedValue<String>> {
    let role = match value {
        SourceValue::Present(_) => SourceUseRole::Payload,
        SourceValue::Unreadable => SourceUseRole::Conversion,
        _ => SourceUseRole::Selector,
    };
    let contribution = scope
        .zip(location)
        .map(|(source, location)| source.at(location, role))
        .transpose()
        .map_err(evidence)?;
    let complete = contribution.is_some() && !matches!(value, SourceValue::Unreadable);
    Ok(uses.value(value.clone(), contribution.as_slice(), complete))
}

fn evidence(error: impl std::error::Error + Send + Sync + 'static) -> PackageError {
    PackageError::EntityEvidence(Box::new(error))
}

fn read_text<'de, D: Deserializer<'de>>(input: D) -> Result<SourcedValue<String>, D::Error> {
    SourcedValueSeed::new(OriginReaderLimits {
        fields: 1,
        metadata_bytes: 16 * 1024,
        path_bytes: 256,
        field_uses: 8,
        reference_bytes: 64,
    })
    .deserialize(input)
}
