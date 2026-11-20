// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;

use p4kx_assembly::WornMountOrigins;
use p4kx_extraction::{FieldOrigins, OriginArtifactDigest, SourceInventory};
use schemars::JsonSchema;
use serde::Serialize;

use crate::manifest::Manifest;

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WornMountProvenance {
    glb: String,
    artifact: OriginArtifactDigest,
    source_inventory: SourceInventory,
    field_origins: FieldOrigins,
}

pub(crate) fn bind(
    manifest: &mut Manifest,
    glb: &str,
    digest: &OriginArtifactDigest,
    observed: Option<WornMountOrigins>,
) -> io::Result<()> {
    let Some(observed) = observed.filter(|origins| !origins.is_empty()) else {
        return Ok(());
    };
    let rig = manifest
        .meshes
        .first_mut()
        .and_then(|mesh| mesh.rig.as_mut())
        .ok_or_else(|| io::Error::other("worn mount provenance has no owning rig"))?;
    let (source_inventory, field_origins) =
        observed.into_field_origins().map_err(io::Error::other)?;
    let proof = WornMountProvenance {
        glb: glb.to_owned(),
        artifact: digest.clone(),
        source_inventory,
        field_origins,
    };
    rig.worn_mount_provenance = Some(serde_json::to_value(proof)?);
    Ok(())
}
