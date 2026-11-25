// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use anyhow::Result;
use p4kx_assembly::EntityNode;
use p4kx_extraction::{
    DataForge, DataForgeOrigin, FieldOrigins, FieldPath, Record, SourceInventory, SourceUses,
};
use serde::Serialize;

use crate::scene_report_origins;

#[derive(Serialize)]
pub(super) struct Origins {
    #[serde(rename = "fieldOrigins")]
    fields: FieldOrigins,
    provenance: SourceInventory,
}

impl Origins {
    pub(super) fn capture(
        forge: &DataForge<'_>,
        record: &Record<'_>,
        original: &DataForgeOrigin,
        tree: &EntityNode,
        has_interior: bool,
    ) -> Result<Self> {
        let uses = SourceUses::with_compact_use_refs();
        let mut fields = FieldOrigins::new();
        scene_report_origins::entity_identity(&mut fields, &uses, forge, record, original, tree)?;
        for path in ["/archive", "/out", "/lod"] {
            fields.insert(FieldPath::parse(path)?, uses.origin(&[], true))?;
        }
        insert_counts(&mut fields, &uses, has_interior)?;
        Ok(Self {
            fields,
            provenance: uses.freeze()?,
        })
    }
}

fn insert_counts(fields: &mut FieldOrigins, uses: &SourceUses, has_interior: bool) -> Result<()> {
    let unknown = uses.origin(&[], false);
    for path in [
        "/parts",
        "/modelled_parts",
        "/hidden_parts",
        "/palettes",
        "/nodes",
        "/meshes",
        "/primitives",
        "/materials",
        "/textured_materials",
        "/textures",
        "/texture_pixels",
        "/default_material_ids",
        "/hidden_primitives",
        "/bones",
        "/skinned_meshes",
        "/lod_fallbacks",
        "/shared_meshes",
        "/shared_materials",
        "/shared_textures",
        "/vertices",
        "/triangles",
        "/bytes",
        "/skipped",
        "/unresolved_layers",
    ] {
        fields.insert(FieldPath::parse(path)?, unknown.clone())?;
    }
    if has_interior {
        for path in [
            "/interior/containers",
            "/interior/parts",
            "/interior/model_parts",
            "/interior/modelled_parts",
            "/interior/lights",
        ] {
            fields.insert(FieldPath::parse(path)?, unknown.clone())?;
        }
    }
    Ok(())
}
