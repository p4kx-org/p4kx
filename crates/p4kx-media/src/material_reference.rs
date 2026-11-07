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
    writer: &mut MediaWriter<'_>,
) -> Result<Vec<MaterialReference>, MediaError> {
    let mut references = Vec::new();
    if let Some(palette) = &library.default_palette {
        references.push(declaration(
            MaterialReferenceRole::DefaultPalette,
            None,
            None,
            None,
            palette,
            context,
            writer,
        )?);
    }
    for (material_index, material) in library.materials.iter().enumerate() {
        append_material(&mut references, material_index, material, context, writer)?;
    }
    Ok(references)
}

fn append_material(
    references: &mut Vec<MaterialReference>,
    material_index: usize,
    material: &Material,
    context: &SourceContext<'_>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    for (layer_index, layer) in material.layers.iter().enumerate() {
        writer.checkpoint()?;
        if let Some(path) = layer.attribute("Path") {
            references.push(declaration(
                MaterialReferenceRole::LayerLibrary,
                Some(material_index),
                Some(layer_index),
                layer.attribute("Submtl"),
                path,
                context,
                writer,
            )?);
        }
    }
    for (texture_index, texture) in material.textures.iter().enumerate() {
        writer.checkpoint()?;
        let role = if texture.file.trim_start().starts_with('$') {
            MaterialReferenceRole::EnginePlaceholder
        } else {
            MaterialReferenceRole::Texture
        };
        references.push(declaration(
            role,
            Some(material_index),
            Some(texture_index),
            None,
            &texture.file,
            context,
            writer,
        )?);
    }
    Ok(())
}
fn declaration(
    role: MaterialReferenceRole,
    material: Option<usize>,
    slot: Option<usize>,
    submaterial: Option<&str>,
    token: &str,
    context: &SourceContext<'_>,
    writer: &mut MediaWriter<'_>,
) -> Result<MaterialReference, MediaError> {
    let original = if matches!(&role, MaterialReferenceRole::EnginePlaceholder) {
        OriginalIdentifier::try_new_with_text("p4kx.material.engine-placeholder", token)?
    } else {
        OriginalIdentifier::new_with_path(token)
    };
    let reference = resolve(&role, token, original, context, writer)?;
    Ok(MaterialReference {
        role,
        material,
        slot,
        submaterial: submaterial.map(str::to_owned),
        reference,
    })
}

fn resolve(
    role: &MaterialReferenceRole,
    token: &str,
    original: OriginalIdentifier,
    context: &SourceContext<'_>,
    writer: &mut MediaWriter<'_>,
) -> Result<Reference, MediaError> {
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
            return writer.resolve_reference(
                original,
                std::iter::empty(),
                CandidateInventory::Unavailable(ReferenceUnavailable::TargetNotRead),
            );
        }
    };
    match path {
        Some(path) => context
            .directory
            .resolve_path(original, &path, kind, writer),
        None => writer.resolve_reference(
            original,
            std::iter::empty(),
            CandidateInventory::Unavailable(ReferenceUnavailable::InvalidIdentifier),
        ),
    }
}
