// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};

use anyhow::Result;
use p4kx_assembly::Interior;
use p4kx_extraction::{
    Archive, FieldOrigin, FieldOrigins, FieldPath, OriginArtifactBinding, OriginOutputAddress,
    SourceCoordinateOmission, SourceInventory, SourceUseRole, SourceUses, write_origin_companion,
};
use serde::Serialize;

use crate::scene_report_origins;

#[derive(Serialize)]
pub(super) struct Origins {
    #[serde(rename = "fieldOrigins")]
    fields: FieldOrigins,
    provenance: SourceInventory,
    #[serde(skip)]
    artifact: FieldOrigin,
}

impl Origins {
    pub(super) fn capture(
        layers: &[Archive],
        chain: &[String],
        interior: &Interior,
        lod: u32,
    ) -> Result<Self> {
        let uses = SourceUses::with_compact_use_refs();
        let mut fields = FieldOrigins::new();
        let mut artifact_uses = Vec::with_capacity(chain.len());
        scene_report_origins::container_paths(&uses, &mut fields, layers, chain, |payload| {
            artifact_uses.push(payload.at_opaque(
                SourceUseRole::Conversion,
                SourceCoordinateOmission::Unavailable,
            )?);
            Ok(())
        })?;
        unknown_report_fields(&uses, &mut fields, interior, lod)?;
        let artifact = uses.origin(&artifact_uses, false);
        Ok(Self {
            fields,
            provenance: uses.freeze()?,
            artifact,
        })
    }

    pub(super) fn write_companion(
        &self,
        writer: &mut dyn Write,
        binding: &OriginArtifactBinding,
    ) -> io::Result<()> {
        write_origin_companion(
            writer,
            binding,
            &self.provenance,
            [(&OriginOutputAddress::OpaquePayload, &self.artifact)],
        )
    }
}

fn unknown_report_fields(
    uses: &SourceUses,
    fields: &mut FieldOrigins,
    interior: &Interior,
    lod: u32,
) -> Result<()> {
    let unknown = uses.origin(&[], false);
    for path in [
        "/parts",
        "/containers_opened",
        "/model_parts",
        "/modelled_parts",
        "/props",
        "/lights",
        "/nodes",
        "/meshes",
        "/primitives",
        "/materials",
        "/default_material_ids",
        "/hidden_primitives",
        "/lod_fallbacks",
        "/shared_meshes",
        "/shared_materials",
        "/shared_textures",
        "/vertices",
        "/triangles",
        "/bytes",
        "/skipped",
        "/unresolved_layers",
        "/issues",
    ] {
        fields.insert(FieldPath::parse(path)?, unknown.clone())?;
    }
    if interior.default_material(lod).is_some() {
        fields.insert(FieldPath::parse("/default_material")?, unknown.clone())?;
    }
    if interior.unresolved_layers().first().is_some() {
        fields.insert(
            FieldPath::parse("/first_unresolved_layer")?,
            unknown.clone(),
        )?;
    }
    for (index, _) in interior.issues().iter().enumerate() {
        for field in ["kind", "name", "path", "reason", "skipped"] {
            let path = FieldPath::parse(&format!("/issues/{index}/{field}"))?;
            fields.insert(path, unknown.clone())?;
        }
    }
    Ok(())
}
