// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use p4kx_extraction::model::{MaterialReadField, MaterialReadObservation, MaterialReadUse};
use p4kx_extraction::{InputScope, SourceUseRole};

use crate::origin_capture::OriginSite;
use crate::origin_contributions::{OriginContributions, OriginCut};
use crate::origin_input::MediaOriginInput;

use crate::{MediaError, MediaWriter};

struct StoredSite {
    indices: Vec<u64>,
    field: String,
    role: SourceUseRole,
    attribute: bool,
}

#[derive(Default)]
struct FieldRead {
    sites: Vec<StoredSite>,
    bytes: u64,
    omitted: Option<SourceUseRole>,
}

/// Neutral parser observations that grant no registered source use until parsing succeeds.
#[derive(Default)]
pub(crate) struct MaterialReadOrigins {
    fields: BTreeMap<(usize, MaterialReadField), FieldRead>,
    unknown: bool,
}

impl MaterialReadOrigins {
    pub(crate) fn observe(&mut self, observation: MaterialReadObservation<'_>, limit: u64) {
        let Some(role) = role(observation.use_kind) else {
            self.unknown = true;
            return;
        };
        let retained = self
            .fields
            .entry((observation.material, observation.field))
            .or_default();
        if retained.omitted.is_some() {
            return;
        }
        let Some((bytes, indices)) = admitted_site(observation, retained, limit) else {
            retained.omitted = Some(role);
            return;
        };
        retained.bytes = bytes;
        retained.sites.push(StoredSite {
            indices,
            field: observation.source_field.to_owned(),
            role,
            attribute: observation.attribute_ordinal.is_some(),
        });
    }

    pub(crate) fn take(
        &mut self,
        material: usize,
        field: MaterialReadField,
        input: &MediaOriginInput,
        writer: &MediaWriter<'_>,
    ) -> Result<OriginContributions, MediaError> {
        let Some(read) = self.fields.remove(&(material, field)) else {
            return crate::material_parts_origins::capture(
                input,
                InputScope::Payload,
                None,
                SourceUseRole::Selector,
                writer,
            );
        };
        let mut capture = writer.capture_origins();
        for site in read.sites {
            let namespace = if site.attribute {
                "materialXmlAttribute"
            } else {
                "materialXmlElement"
            };
            capture.add(
                input,
                InputScope::Payload,
                OriginSite::new(namespace, &site.indices, &site.field),
                site.role,
                writer,
            )?;
        }
        if let Some(role) = read.omitted {
            capture.add_opaque(
                input,
                InputScope::Payload,
                role,
                OriginCut::UseMetadata,
                writer,
            )?;
        }
        if self.unknown {
            capture.include_original(&OriginContributions::unknown(&writer.origins.owner))?;
        }
        Ok(capture.finish())
    }

    pub(crate) fn positions(&self, material: usize, field: MaterialReadField) -> Option<&[u64]> {
        let read = self.fields.get(&(material, field))?;
        read.sites
            .iter()
            .find(|site| !site.attribute)
            .map(|site| site.indices.as_slice())
    }
}
fn admitted_site(
    observation: MaterialReadObservation<'_>,
    retained: &mut FieldRead,
    limit: u64,
) -> Option<(u64, Vec<u64>)> {
    let count = observation
        .indices
        .len()
        .checked_add(usize::from(observation.attribute_ordinal.is_some()))?;
    let words = count.checked_mul(size_of::<u64>())?;
    let size = observation
        .source_field
        .len()
        .checked_add(words)?
        .checked_add(size_of::<StoredSite>())?;
    let bytes = retained.bytes.checked_add(u64::try_from(size).ok()?)?;
    if bytes > limit {
        return None;
    }
    let mut indices = Vec::new();
    indices.try_reserve_exact(count).ok()?;
    retained.sites.try_reserve(1).ok()?;
    for ordinal in observation
        .indices
        .iter()
        .copied()
        .chain(observation.attribute_ordinal)
    {
        indices.push(u64::try_from(ordinal).ok()?);
    }
    Some((bytes, indices))
}

const fn role(kind: MaterialReadUse) -> Option<SourceUseRole> {
    match kind {
        MaterialReadUse::Authored => Some(SourceUseRole::Payload),
        MaterialReadUse::Selected => Some(SourceUseRole::Selector),
        MaterialReadUse::Interpreted => Some(SourceUseRole::Conversion),
        _ => None,
    }
}
