// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_archive::{EntryOrigin, EntryOriginStep, SourceEncryption};
use p4kx_model::{Material, MaterialStatus, Model};
use serde::Serialize;
use std::fmt::Write as _;

use crate::material_source_info::{MaterialAuthoredInfo, MaterialLibrarySourceInfo};
use crate::{ArchiveDefaultMaterial, ArchiveDefaultMaterialState};

/// What reading a model's material library came to.
#[derive(Debug, Serialize)]
pub struct LibraryInfo<'a> {
    path: Option<&'a str>,
    target: Option<&'a str>,
    status: &'static str,
    problem: Option<String>,
    single: Option<bool>,
    default_palette: Option<&'a str>,
    authored: Option<MaterialLibrarySourceInfo<'a>>,
}

impl<'a> LibraryInfo<'a> {
    /// Describes the material library of `model`.
    #[must_use]
    pub fn of(model: &'a Model) -> Self {
        let library = model.materials.library();
        Self {
            path: model.material_library.as_deref(),
            target: model.materials.target(),
            status: match &model.materials {
                MaterialStatus::Missing { .. } => "missing",
                MaterialStatus::Unreadable { .. } => "unreadable",
                MaterialStatus::Loaded(_) => "loaded",
                _ => "unnamed",
            },
            problem: model.materials.problem(),
            single: library.map(|library| library.single),
            default_palette: library.and_then(|library| library.default_palette.as_deref()),
            authored: library.map(MaterialLibrarySourceInfo::of),
        }
    }
}

/// One material of a model's library.
#[derive(Debug, Serialize)]
pub struct MaterialInfo<'a> {
    id: Option<usize>,
    name: &'a str,
    flags: u32,
    shader: &'a str,
    surface_type: &'a str,
    string_gen_mask: &'a str,
    diffuse: [f32; 3],
    specular: [f32; 3],
    emissive: [f32; 3],
    shininess: f32,
    opacity: f32,
    alpha_test: f32,
    glow: f32,
    textures: Vec<TextureInfo<'a>>,
    authored: MaterialAuthoredInfo<'a>,
}

#[derive(Debug, Serialize)]
struct TextureInfo<'a> {
    map: &'a str,
    file: &'a str,
}

impl<'a> MaterialInfo<'a> {
    /// Describes the materials of the model's library, none when it was not read.
    #[must_use]
    pub fn of_model(model: &'a Model) -> Vec<Self> {
        let Some(library) = model.materials.library() else {
            return Vec::new();
        };
        library
            .materials
            .iter()
            .enumerate()
            .map(|(position, material)| Self::of(material, (!library.single).then_some(position)))
            .collect()
    }

    fn of(material: &'a Material, id: Option<usize>) -> Self {
        Self {
            id,
            name: &material.name,
            flags: material.flags.0,
            shader: &material.shader,
            surface_type: &material.surface_type,
            string_gen_mask: &material.string_gen_mask,
            diffuse: material.diffuse,
            specular: material.specular,
            emissive: material.emissive,
            shininess: material.shininess,
            opacity: material.opacity,
            alpha_test: material.alpha_test,
            glow: material.glow,
            authored: MaterialAuthoredInfo::of(material),
            textures: material
                .textures
                .iter()
                .map(|texture| TextureInfo {
                    map: &texture.map,
                    file: &texture.file,
                })
                .collect(),
        }
    }
}

/// Physical fallback lookup and its original source, separate from the authored library.
#[derive(Debug, Serialize)]
pub struct DefaultMaterialInfo<'a> {
    state: &'static str,
    source: Option<DefaultMaterialSource<'a>>,
    ambiguous_sources: Vec<DefaultMaterialSource<'a>>,
    material: Option<MaterialInfo<'a>>,
    decoded_sha256: Option<String>,
}

#[derive(Debug, Serialize)]
struct DefaultMaterialSource<'a> {
    entry_index: usize,
    entry_path: &'a str,
    containers: Vec<Option<DefaultMaterialContainer<'a>>>,
    contains_encrypted_source: Option<bool>,
}

#[derive(Debug, Serialize)]
struct DefaultMaterialContainer<'a> {
    entry_index: usize,
    entry_path: &'a str,
}

impl<'a> DefaultMaterialInfo<'a> {
    pub(crate) fn of(read: &'a ArchiveDefaultMaterial) -> Self {
        let (state, ambiguous_sources) = match read.state() {
            ArchiveDefaultMaterialState::Missing => ("missing", Vec::new()),
            ArchiveDefaultMaterialState::Ambiguous { witnesses } => (
                "ambiguous",
                witnesses.iter().map(DefaultMaterialSource::of).collect(),
            ),
            ArchiveDefaultMaterialState::LookupFailed(_) => ("lookup-failed", Vec::new()),
            ArchiveDefaultMaterialState::Unreadable(_) => ("unreadable", Vec::new()),
            ArchiveDefaultMaterialState::Malformed(_) => ("malformed", Vec::new()),
            ArchiveDefaultMaterialState::UnsupportedLibrary => ("unsupported-library", Vec::new()),
            ArchiveDefaultMaterialState::Resolved => ("resolved", Vec::new()),
        };
        Self {
            state,
            source: read.original().map(DefaultMaterialSource::of),
            ambiguous_sources,
            material: read
                .material()
                .map(|material| MaterialInfo::of(material, Some(0))),
            decoded_sha256: read.decoded_sha256().map(sha256_hex),
        }
    }
}

impl<'a> DefaultMaterialSource<'a> {
    fn of(origin: &'a EntryOrigin) -> Self {
        Self {
            entry_index: origin.entry().index(),
            entry_path: origin.entry().path(),
            containers: origin
                .ancestors()
                .map(|step| step.map(DefaultMaterialContainer::of))
                .collect(),
            contains_encrypted_source: match origin.encryption() {
                SourceEncryption::Encrypted => Some(true),
                SourceEncryption::Unencrypted => Some(false),
                _ => None,
            },
        }
    }
}

impl<'a> DefaultMaterialContainer<'a> {
    fn of(step: &'a EntryOriginStep) -> Self {
        Self {
            entry_index: step.index(),
            entry_path: step.path(),
        }
    }
}

fn sha256_hex(digest: &[u8; 32]) -> String {
    let mut hex = String::with_capacity(2 * digest.len());
    for byte in digest {
        let _ignored = write!(hex, "{byte:02x}");
    }
    hex
}
