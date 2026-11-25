// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};

use anyhow::{Result, ensure};
use p4kx_assembly::Interior;
use p4kx_extraction::{
    Archive, FieldOrigin, FieldOrigins, FieldPath, InputScope, OriginArtifactBinding,
    OriginOutputAddress, SourceCoordinateOmission, SourceInput, SourceInventory, SourceLocation,
    SourceUse, SourceUseRole, SourceUses, write_origin_companion,
};
use serde::Serialize;

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
        ensure!(
            layers.len() == chain.len() + 1,
            "opened container path count differs"
        );
        let uses = SourceUses::with_compact_use_refs();
        let mut fields = FieldOrigins::new();
        let mut path_uses = Vec::with_capacity(chain.len());
        let mut artifact_uses = Vec::with_capacity(chain.len());
        let mut complete = true;
        for (index, (layer, path)) in layers.iter().skip(1).zip(chain).enumerate() {
            let origin = if let Some(bound) = observe_container(&uses, layer, path)? {
                let origin = uses.origin(std::slice::from_ref(&bound.path), bound.complete);
                complete &= bound.complete;
                path_uses.push(bound.path);
                artifact_uses.push(bound.conversion);
                origin
            } else {
                complete = false;
                uses.origin(&[], false)
            };
            fields.insert(FieldPath::parse(&format!("/containers/{index}"))?, origin)?;
        }
        fields.insert(
            FieldPath::parse("/containers")?,
            uses.origin(&path_uses, complete),
        )?;
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

struct ContainerUses {
    path: SourceUse,
    conversion: SourceUse,
    complete: bool,
}

fn observe_container(
    uses: &SourceUses,
    layer: &Archive,
    path: &str,
) -> Result<Option<ContainerUses>> {
    let Some(entry) = layer.container_input_origin() else {
        return Ok(None);
    };
    ensure!(
        entry.entry().path() == path,
        "opened container path differs from its original Entry"
    );
    let payload = uses.observe(SourceInput::Archive(&entry), InputScope::Payload)?;
    let path = payload.for_scope(InputScope::ArchiveDirectoryMetadata).at(
        SourceLocation::Metadata {
            field: "path".into(),
        },
        SourceUseRole::Payload,
    )?;
    let conversion = payload.at_opaque(
        SourceUseRole::Conversion,
        SourceCoordinateOmission::Unavailable,
    )?;
    Ok(Some(ContainerUses {
        path,
        conversion,
        complete: entry.ancestors().all(|ancestor| ancestor.is_some()),
    }))
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
