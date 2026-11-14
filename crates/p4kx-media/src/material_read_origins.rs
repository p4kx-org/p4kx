// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use p4kx_extraction::model::{MaterialReadField, MaterialReadObservation, MaterialReadUse};
use p4kx_extraction::{FieldPath, InputScope, SourceUseRole};

use crate::origin_capture::OriginSite;
use crate::origin_contributions::{OriginContributions, OriginCut};
use crate::origin_input::MediaOriginInput;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::{MediaError, MediaWriter, origin_error};

struct StoredSite {
    indices: Vec<u64>,
    field: String,
    role: SourceUseRole,
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
    fields: BTreeMap<(usize, &'static str), FieldRead>,
    unknown: bool,
}

impl MaterialReadOrigins {
    pub(crate) fn observe(&mut self, observation: MaterialReadObservation<'_>, limit: u64) {
        let field = match surface_field(observation.field) {
            SurfaceField::Reported(name) => name,
            SurfaceField::Other => return,
            SurfaceField::Unknown => {
                self.unknown = true;
                return;
            }
        };
        let Some(role) = role(observation.use_kind) else {
            self.unknown = true;
            return;
        };
        let retained = self
            .fields
            .entry((observation.material, field))
            .or_default();
        if retained.omitted.is_some() {
            return;
        }
        let bytes = u64::try_from(observation.source_field.len())
            .ok()
            .and_then(|bytes| {
                u64::try_from(observation.indices.len())
                    .ok()
                    .and_then(|count| count.checked_mul(size_of::<u64>() as u64))
                    .and_then(|indices| bytes.checked_add(indices))
            })
            .and_then(|bytes| bytes.checked_add(size_of::<StoredSite>() as u64))
            .and_then(|bytes| retained.bytes.checked_add(bytes));
        let Some(bytes) = bytes.filter(|bytes| *bytes <= limit) else {
            retained.omitted = Some(role);
            return;
        };
        let mut indices = Vec::new();
        if indices
            .try_reserve_exact(observation.indices.len())
            .is_err()
            || retained.sites.try_reserve(1).is_err()
        {
            retained.omitted = Some(role);
            return;
        }
        for ordinal in observation.indices {
            let Ok(ordinal) = u64::try_from(*ordinal) else {
                retained.omitted = Some(role);
                return;
            };
            indices.push(ordinal);
        }
        retained.bytes = bytes;
        retained.sites.push(StoredSite {
            indices,
            field: observation.source_field.to_owned(),
            role,
        });
    }

    pub(crate) fn finish(
        self,
        input: &MediaOriginInput,
        writer: &MediaWriter<'_>,
    ) -> Result<PreparedRecordOrigins, MediaError> {
        let mut prepared = writer.record_origins();
        for ((material, field), read) in self.fields {
            let mut capture = writer.capture_origins();
            for site in read.sites {
                capture.add(
                    input,
                    InputScope::Payload,
                    OriginSite::new("material.xml", &site.indices, &site.field),
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
            prepared.insert_subtree(&field_path(material, field)?, capture.finish(), writer)?;
        }
        if self.unknown {
            let path = FieldPath::parse("/data/payload/materials")
                .map_err(|cause| origin_error::address(&cause))?;
            prepared.insert(
                &path,
                OriginContributions::unknown(&writer.origins.owner),
                writer,
            )?;
        }
        Ok(prepared)
    }
}

fn field_path(material: usize, field: &str) -> Result<FieldPath, MediaError> {
    let mut path = FieldPath::parse("/data/payload/materials")
        .map_err(|cause| origin_error::address(&cause))?;
    path.push_index(material)
        .map_err(|cause| origin_error::address(&cause))?;
    for member in field.split('/') {
        path.push_field(member)
            .map_err(|cause| origin_error::address(&cause))?;
    }
    Ok(path)
}

enum SurfaceField {
    Reported(&'static str),
    Other,
    Unknown,
}

const fn surface_field(field: MaterialReadField) -> SurfaceField {
    match field {
        MaterialReadField::Name => SurfaceField::Reported("name"),
        MaterialReadField::Flags => SurfaceField::Reported("interpreted_surface/flags"),
        MaterialReadField::Shader => SurfaceField::Reported("interpreted_surface/shader"),
        MaterialReadField::SurfaceType => {
            SurfaceField::Reported("interpreted_surface/surface_type")
        }
        MaterialReadField::StringGenMask => {
            SurfaceField::Reported("interpreted_surface/string_gen_mask")
        }
        MaterialReadField::Diffuse => SurfaceField::Reported("interpreted_surface/diffuse"),
        MaterialReadField::Specular => SurfaceField::Reported("interpreted_surface/specular"),
        MaterialReadField::Emissive => SurfaceField::Reported("interpreted_surface/emissive"),
        MaterialReadField::Shininess => SurfaceField::Reported("interpreted_surface/shininess"),
        MaterialReadField::Opacity => SurfaceField::Reported("interpreted_surface/opacity"),
        MaterialReadField::AlphaTest => SurfaceField::Reported("interpreted_surface/alpha_test"),
        MaterialReadField::Glow => SurfaceField::Reported("interpreted_surface/glow"),
        MaterialReadField::Palette => SurfaceField::Reported("interpreted_surface/palette"),
        MaterialReadField::LibrarySingle
        | MaterialReadField::LibraryDefaultPalette
        | MaterialReadField::LibraryTotal
        | MaterialReadField::LibraryUnknownCount
        | MaterialReadField::Row
        | MaterialReadField::Textures
        | MaterialReadField::Texture { .. }
        | MaterialReadField::TextureMap { .. }
        | MaterialReadField::TextureFile { .. }
        | MaterialReadField::PublicParams
        | MaterialReadField::PublicParam { .. }
        | MaterialReadField::Layers
        | MaterialReadField::Layer { .. }
        | MaterialReadField::LayerAttribute { .. }
        | MaterialReadField::LayerName { .. }
        | MaterialReadField::LayerPath { .. }
        | MaterialReadField::LayerPaletteTint { .. } => SurfaceField::Other,
        _ => SurfaceField::Unknown,
    }
}

const fn role(kind: MaterialReadUse) -> Option<SourceUseRole> {
    match kind {
        MaterialReadUse::Authored => Some(SourceUseRole::Payload),
        MaterialReadUse::Selected => Some(SourceUseRole::Selector),
        MaterialReadUse::Interpreted => Some(SourceUseRole::Conversion),
        _ => None,
    }
}
