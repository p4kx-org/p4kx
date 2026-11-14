// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use p4kx_archive::EntryOrigin;
use p4kx_chunks::{crch, ivo};
use p4kx_model::{
    ChunkUsage, Material, MaterialLibrary, MaterialReadField, MaterialReadObservation, Model,
    ModelFile,
};
use serde::Serialize;
use thiserror::Error;

use crate::material_source_info::MaterialSourceSite;
use crate::{
    ConversionError, FieldOrigins, FieldOriginsError, FieldPath, FieldPathError, InputScope,
    SourceEvidenceError, SourceInput, SourceInventory, SourceLocation, SourceScope, SourceUse,
    SourceUseRole, SourceUses,
};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MaterialFieldReceipt {
    material_field_origins: FieldOrigins,
    material_provenance: SourceInventory,
}

#[derive(Debug, Error)]
pub(crate) enum ReceiptError {
    #[error(transparent)]
    Evidence(#[from] SourceEvidenceError),
    #[error(transparent)]
    Path(#[from] FieldPathError),
    #[error(transparent)]
    Duplicate(#[from] FieldOriginsError),
}

type Sites = BTreeMap<(usize, MaterialReadField), Vec<MaterialSourceSite>>;

#[derive(Default)]
pub(crate) struct Observations(Sites);

impl Observations {
    pub(crate) fn retain(&mut self, observed: MaterialReadObservation<'_>) {
        if let Some(site) = MaterialSourceSite::from_observation(observed) {
            self.0
                .entry((observed.material, observed.field))
                .or_default()
                .push(site);
        }
    }

    pub(crate) fn project(
        &self,
        model: &Model,
        model_entry: Option<&EntryOrigin>,
        library_entry: Option<&EntryOrigin>,
    ) -> Result<Option<MaterialFieldReceipt>, ConversionError> {
        model
            .materials
            .library()
            .map(|library| {
                self.project_library(
                    library,
                    library_entry,
                    model_entry.zip(material_selector(model)),
                )
            })
            .transpose()
            .map_err(|source| ConversionError::MaterialSourceEvidence(Box::new(source)))
    }

    fn project_library(
        &self,
        library: &MaterialLibrary,
        library_entry: Option<&EntryOrigin>,
        locator: Option<(&EntryOrigin, usize)>,
    ) -> Result<MaterialFieldReceipt, ReceiptError> {
        use MaterialReadField::{LibraryDefaultPalette, LibrarySingle, LibraryTotal};

        let uses = SourceUses::with_compact_use_refs();
        let selector = locator
            .map(|(entry, index)| {
                uses.observe(SourceInput::Archive(entry), InputScope::Payload)?
                    .at(
                        SourceLocation::Subresource {
                            namespace: "modelChunk".into(),
                            indices: vec![index as u64],
                            field: "materialLibrary".into(),
                        },
                        SourceUseRole::Selector,
                    )
            })
            .transpose()?;
        let scope = library_entry
            .map(|entry| uses.observe(SourceInput::Archive(entry), InputScope::Payload))
            .transpose()?;
        let mut writer = Writer {
            sites: &self.0,
            uses: &uses,
            scope,
            selector,
            complete: library_entry.is_some_and(complete)
                && locator.is_some_and(|(entry, _)| complete(entry)),
            fields: FieldOrigins::new(),
        };
        writer.put("/library/single", 0, &[LibrarySingle])?;
        writer.put("/materials", 0, &[LibraryTotal])?;
        writer.put("/library/default_palette", 0, &[LibraryDefaultPalette])?;
        for (index, material) in library.materials.iter().enumerate() {
            writer.material(index, material)?;
        }
        Ok(MaterialFieldReceipt {
            material_field_origins: writer.fields,
            material_provenance: uses.freeze()?,
        })
    }
}

fn complete(entry: &EntryOrigin) -> bool {
    entry.ancestors().all(|item| item.is_some())
}

fn material_selector(model: &Model) -> Option<usize> {
    model
        .chunks
        .iter()
        .find(|chunk| {
            chunk.file == ModelFile::Model
                && chunk.usage == ChunkUsage::Read
                && [crch::MTL_NAME, ivo::MTL_NAME, ivo::MTL_NAME_320].contains(&chunk.chunk_type)
        })
        .map(|chunk| chunk.index)
}

struct Writer<'a> {
    sites: &'a Sites,
    uses: &'a SourceUses,
    scope: Option<SourceScope>,
    selector: Option<SourceUse>,
    complete: bool,
    fields: FieldOrigins,
}

impl Writer<'_> {
    fn put(
        &mut self,
        pointer: &str,
        material: usize,
        values: &[MaterialReadField],
    ) -> Result<(), ReceiptError> {
        let path = FieldPath::parse(pointer)?;
        let mut contributions: Vec<_> = self.selector.iter().cloned().collect();
        let mut lost = self.scope.is_none();
        if let Some(scope) = &self.scope {
            for &value in values {
                if let Some(sites) = self.sites.get(&(material, value)) {
                    for site in sites {
                        contributions.push(scope.at(site.location().clone(), site.role())?);
                    }
                } else {
                    lost = true;
                }
            }
        }
        self.fields.insert(
            path,
            self.uses.origin(&contributions, !lost && self.complete),
        )?;
        Ok(())
    }

    fn material(&mut self, index: usize, material: &Material) -> Result<(), ReceiptError> {
        use MaterialReadField::{
            AlphaTest, Diffuse, Emissive, Flags, Glow, Name, Opacity, Row, Shader, Shininess,
            Specular, StringGenMask, SurfaceType,
        };

        let parent = format!("/materials/{index}");
        self.put(&format!("{parent}/id"), index, &[Row])?;
        for (name, field) in [
            ("name", Name),
            ("flags", Flags),
            ("shader", Shader),
            ("surface_type", SurfaceType),
            ("string_gen_mask", StringGenMask),
            ("shininess", Shininess),
            ("opacity", Opacity),
            ("alpha_test", AlphaTest),
            ("glow", Glow),
        ] {
            self.put(&format!("{parent}/{name}"), index, &[Row, field])?;
        }
        for (name, field) in [
            ("diffuse", Diffuse),
            ("specular", Specular),
            ("emissive", Emissive),
        ] {
            self.put(&format!("{parent}/{name}"), index, &[Row, field])?;
            for channel in 0..3 {
                self.put(&format!("{parent}/{name}/{channel}"), index, &[Row, field])?;
            }
        }
        self.sources(&parent, index, material)
    }

    fn sources(
        &mut self,
        path: &str,
        index: usize,
        material: &Material,
    ) -> Result<(), ReceiptError> {
        use MaterialReadField::{Layers, PublicParams, Row, Textures};
        for scope in ["textures", "authored/textures"] {
            let textures = format!("{path}/{scope}");
            self.put(&textures, index, &[Row, Textures])?;
            for (position, _) in material.textures.iter().enumerate() {
                let slot = format!("{textures}/{position}");
                self.put(
                    &format!("{slot}/map"),
                    index,
                    &[Row, MaterialReadField::TextureMap { index: position }],
                )?;
                self.put(
                    &format!("{slot}/file"),
                    index,
                    &[Row, MaterialReadField::TextureFile { index: position }],
                )?;
            }
        }
        let parameters = format!("{path}/authored/public_params");
        self.put(&parameters, index, &[Row, PublicParams])?;
        for (position, _) in material.public_params.iter().enumerate() {
            let field = MaterialReadField::PublicParam { index: position };
            for name in ["name", "value"] {
                self.put(
                    &format!("{parameters}/{position}/{name}"),
                    index,
                    &[Row, field],
                )?;
            }
        }
        let layer_path = format!("{path}/authored/layers");
        self.put(&layer_path, index, &[Row, Layers])?;
        for (position, layer) in material.layers.iter().enumerate() {
            let path = format!("{layer_path}/{position}/attributes");
            self.put(
                &path,
                index,
                &[Row, MaterialReadField::Layer { index: position }],
            )?;
            for (attribute, _) in layer.attributes.iter().enumerate() {
                let field = MaterialReadField::LayerAttribute {
                    index: position,
                    attribute,
                };
                for name in ["name", "value"] {
                    self.put(&format!("{path}/{attribute}/{name}"), index, &[Row, field])?;
                }
            }
        }
        Ok(())
    }
}
