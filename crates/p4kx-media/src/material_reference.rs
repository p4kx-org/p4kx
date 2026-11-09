// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::model::{Material, MaterialLibrary};
use schemars::JsonSchema;
use serde::Serialize;

use crate::{
    CandidateInventory, MediaError, MediaKind, MediaWriter, OriginalIdentifier, Reference,
    ReferenceUnavailable, SourceContext,
};

/// Role of an original material declaration, without inferred target existence.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum MaterialReferenceRole {
    /// A layer's `Path` with a separate original `Submtl` token.
    LayerLibrary,
    /// A file-backed texture declaration.
    Texture,
    /// A runtime engine texture token beginning with `$`.
    EnginePlaceholder,
    /// A default-palette Record-path declaration.
    DefaultPalette,
}

/// One ordered declaration with separate original tokens and central resolution.
#[derive(Debug, Serialize, JsonSchema)]
#[schemars(extend("allOf" = [{"required": ["material", "slot", "submaterial"]}]))]
#[non_exhaustive]
pub struct MaterialReference {
    /// Source declaration role.
    pub role: MaterialReferenceRole,
    /// Original material index, absent for a library-root declaration.
    pub material: Option<usize>,
    /// Position in the retained typed layer/texture list; original XML ordinals remain in authored syntax.
    pub slot: Option<usize>,
    /// Original `Submtl` name/token, without treating it as a numeric index.
    pub submaterial: Option<String>,
    /// Actual common classification; absent target evidence remains unavailable.
    pub reference: Reference,
}

pub(crate) fn project(
    library: &MaterialLibrary,
    context: &SourceContext<'_>,
    origins: &mut crate::origin_prepared::PreparedRecordOrigins,
    writer: &mut MediaWriter<'_>,
) -> Result<Vec<MaterialReference>, MediaError> {
    let mut references = Vec::new();
    if let Some(palette) = &library.default_palette {
        append(
            &mut references,
            Declaration {
                role: MaterialReferenceRole::DefaultPalette,
                material: None,
                slot: None,
                submaterial: None,
                token: palette,
            },
            context,
            origins,
            writer,
        )?;
    }
    for (material_index, material) in library.materials.iter().enumerate() {
        append_material(
            &mut references,
            material_index,
            material,
            context,
            origins,
            writer,
        )?;
    }
    Ok(references)
}

fn append_material(
    references: &mut Vec<MaterialReference>,
    material_index: usize,
    material: &Material,
    context: &SourceContext<'_>,
    origins: &mut crate::origin_prepared::PreparedRecordOrigins,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    for (layer_index, layer) in material.layers.iter().enumerate() {
        writer.checkpoint()?;
        if let Some(path) = layer.attribute("Path") {
            append(
                references,
                Declaration {
                    role: MaterialReferenceRole::LayerLibrary,
                    material: Some(material_index),
                    slot: Some(layer_index),
                    submaterial: layer.attribute("Submtl"),
                    token: path,
                },
                context,
                origins,
                writer,
            )?;
        }
    }
    for (texture_index, texture) in material.textures.iter().enumerate() {
        writer.checkpoint()?;
        let role = if texture.file.trim_start().starts_with('$') {
            MaterialReferenceRole::EnginePlaceholder
        } else {
            MaterialReferenceRole::Texture
        };
        append(
            references,
            Declaration {
                role,
                material: Some(material_index),
                slot: Some(texture_index),
                submaterial: None,
                token: &texture.file,
            },
            context,
            origins,
            writer,
        )?;
    }
    Ok(())
}

struct Declaration<'a> {
    role: MaterialReferenceRole,
    material: Option<usize>,
    slot: Option<usize>,
    submaterial: Option<&'a str>,
    token: &'a str,
}

fn append(
    references: &mut Vec<MaterialReference>,
    declaration: Declaration<'_>,
    context: &SourceContext<'_>,
    origins: &mut crate::origin_prepared::PreparedRecordOrigins,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let original = if matches!(declaration.role, MaterialReferenceRole::EnginePlaceholder) {
        OriginalIdentifier::try_new_with_text(
            "p4kx.material.engine-placeholder",
            declaration.token,
        )?
    } else {
        OriginalIdentifier::new_with_path(declaration.token)
    };
    let resolution = resolve(
        &declaration.role,
        declaration.token,
        original,
        context,
        writer,
    )?;
    let reference = MaterialReference {
        role: declaration.role,
        material: declaration.material,
        slot: declaration.slot,
        submaterial: declaration.submaterial.map(str::to_owned),
        reference: resolution.reference,
    };
    crate::material_reference_origins::write(
        references.len(),
        &reference,
        &resolution.origins,
        context,
        origins,
        writer,
    )?;
    references.push(reference);
    Ok(())
}

fn resolve(
    role: &MaterialReferenceRole,
    token: &str,
    original: OriginalIdentifier,
    context: &SourceContext<'_>,
    writer: &mut MediaWriter<'_>,
) -> Result<crate::material_reference_origins::MaterialResolution, MediaError> {
    let (path, kind) = match role {
        MaterialReferenceRole::LayerLibrary => (
            p4kx_extraction::model::material_library_path(token)
                .map(|path| format!("Data\\{}.mtl", path.replace('/', "\\"))),
            Some(MediaKind::Material),
        ),
        MaterialReferenceRole::Texture => (
            p4kx_extraction::texture_path(token)
                .map(|path| format!("Data\\{}", path.replace('/', "\\"))),
            None,
        ),
        MaterialReferenceRole::EnginePlaceholder | MaterialReferenceRole::DefaultPalette => {
            return unavailable(original, ReferenceUnavailable::TargetNotRead, writer);
        }
    };
    let Some(path) = path else {
        return unavailable(original, ReferenceUnavailable::InvalidIdentifier, writer);
    };
    let observed = context
        .directory
        .lookup_input_observed(original, &path, kind, writer)?;
    Ok(crate::material_reference_origins::MaterialResolution {
        reference: observed.lookup.reference,
        origins: crate::material_reference_origins::MaterialReferenceProof {
            evidence: observed.evidence,
            inventory: observed.inventory,
        },
    })
}

fn unavailable(
    original: OriginalIdentifier,
    reason: ReferenceUnavailable,
    writer: &mut MediaWriter<'_>,
) -> Result<crate::material_reference_origins::MaterialResolution, MediaError> {
    let (reference, evidence) = writer
        .try_resolve_reference_observed::<crate::origin_input::MediaOriginInput>(
            original,
            std::iter::empty(),
            CandidateInventory::Unavailable(reason),
        )?;
    Ok(crate::material_reference_origins::MaterialResolution {
        reference,
        origins: crate::material_reference_origins::MaterialReferenceProof {
            evidence,
            inventory: crate::origin_contributions::OriginContributions::independent(
                &writer.origins.owner,
            ),
        },
    })
}
