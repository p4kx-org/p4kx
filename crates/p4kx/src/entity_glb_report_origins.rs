// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use anyhow::Result;
use p4kx_assembly::EntityNode;
use p4kx_extraction::{
    DataForge, DataForgeOrigin, FieldOrigins, FieldPath, InputScope, Record, RecordJsonLocation,
    RecordText, SourceInput, SourceInventory, SourceLocation, SourceUseRole, SourceUses,
};
use serde::Serialize;

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
        if record.is_from(forge)
            && Some(record.index()) == tree.record_index()
            && record.guid() == tree.guid()
        {
            root_identity(&mut fields, &uses, forge, record, original)?;
        } else {
            for path in ["/entity", "/guid"] {
                fields.insert(FieldPath::parse(path)?, uses.origin(&[], false))?;
            }
        }
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

fn root_identity(
    fields: &mut FieldOrigins,
    uses: &SourceUses,
    forge: &DataForge<'_>,
    record: &Record<'_>,
    original: &DataForgeOrigin,
) -> Result<()> {
    let (source, complete) = match original {
        DataForgeOrigin::Archive(entry) => (
            SourceInput::Archive(entry),
            entry.ancestors().all(|ancestor| ancestor.is_some()),
        ),
        DataForgeOrigin::File(file) => (SourceInput::File(file), true),
        _ => (SourceInput::Unknown, false),
    };
    let scope = uses.observe(source, InputScope::Payload)?;
    let header = |field: &str, role| {
        scope.at(
            SourceLocation::DataForge {
                record_index: record.index(),
                field: field.into(),
            },
            role,
        )
    };
    let guid = header("guid", SourceUseRole::Payload)?;
    fields.insert(FieldPath::parse("/guid")?, uses.origin(&[guid], complete))?;

    let mut name = vec![header("recordName", SourceUseRole::Conversion)?];
    let name_complete = match record.stored_name() {
        Ok(RecordText::Present(text)) => {
            let original = RecordJsonLocation::borrowed_text(forge, text)
                .map(|location| scope.at(location, SourceUseRole::Conversion))
                .transpose()?;
            let complete = original.is_some();
            name.extend(original);
            complete
        }
        Ok(RecordText::Absent | RecordText::Null) => true,
        _ => false,
    };
    fields.insert(
        FieldPath::parse("/entity")?,
        uses.origin(&name, complete && name_complete),
    )?;
    Ok(())
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
