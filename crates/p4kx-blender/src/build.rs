// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::Issue as AssemblyIssue;
use p4kx_gltf::{DrawnMaterial, GlbOptions, TextureFile};

use crate::PackageInfo;
use crate::collect::{Collected, id_of as material_id};
use crate::describe::settle_stencil;
use crate::items::Item;
use crate::manifest::{
    Basis, ColourSpace, Coordinates, EntityRef, FORMAT_VERSION, Format, Generator, Issue,
    IssueOrigin, MESHES_FOLDER, Manifest, MaterialTextures, Mesh, MeshMaterial, Node, Options,
    Source, TEXTURES_FOLDER, Texture, Unit,
};
use crate::manifest_light::Light;
use crate::manifest_materials::{MaterialDescription, Palette, PaletteColours};
use crate::plan::{MeshPlan, mesh_id};
use crate::textures::{Outcome, id_of};

/// Everything the manifest is made of.
pub(crate) struct Inputs<'a> {
    pub(crate) entity: EntityRef,
    pub(crate) assembly_issues: &'a [AssemblyIssue],
    pub(crate) items: &'a [Item<'a>],
    pub(crate) info: &'a PackageInfo,
    pub(crate) options: &'a GlbOptions,
    pub(crate) plan: &'a MeshPlan,
    pub(crate) collected: &'a Collected,
    pub(crate) outcomes: &'a [Outcome],
    pub(crate) meshes: &'a [crate::meshes::WrittenMesh],
}

/// Builds the manifest of a package whose files are written.
pub(crate) fn manifest(inputs: &Inputs<'_>) -> Manifest {
    let materials = materials(inputs);
    let version = if materials
        .iter()
        .any(|material| material.emission_source.is_some())
    {
        crate::manifest::AUTHORED_HELPER_FORMAT_VERSION
    } else {
        FORMAT_VERSION
    };
    Manifest {
        format: Format::P4kxBlenderPackage,
        version: version.to_owned(),
        generator: generator(),
        source: Source {
            channel: inputs.info.channel.clone(),
            build: inputs.info.build.clone(),
            entity: inputs.entity.clone(),
            character: None,
        },
        coordinates: coordinates(),
        options: Options {
            lod: inputs.options.lod,
            texture_mip: inputs.options.texture_mip,
            vertex_colours: inputs.options.writes_vertex_colours(),
            exclude: inputs.info.exclude.clone(),
            replace: inputs.info.replace.clone(),
        },
        nodes: nodes(inputs),
        meshes: meshes(inputs),
        textures: textures(inputs),
        materials,
        material_libraries: inputs
            .collected
            .library_sources
            .descriptions()
            .cloned()
            .collect(),
        palettes: palettes(inputs),
        authored_soc: None,
        collision_helpers: None,
        default_material: None,
        entity_provenance: None,
        light_placements: None,
        entity_glow: None,
        issues: issues(inputs),
    }
}

/// The program that writes the package.
pub(crate) fn generator() -> Generator {
    Generator {
        name: "p4kx".to_owned(),
        version: env!("CARGO_PKG_VERSION").to_owned(),
    }
}

/// The axes and unit of every package.
pub(crate) const fn coordinates() -> Coordinates {
    Coordinates {
        basis: Basis::GltfYUp,
        unit: Unit::Metre,
    }
}

fn nodes(inputs: &Inputs<'_>) -> Vec<Node> {
    inputs
        .items
        .iter()
        .enumerate()
        .map(|(position, item)| Node {
            kind: item.kind,
            name: item.name.to_owned(),
            path: item.path.clone(),
            parent: item.parent,
            entity: item.entity.clone(),
            light: item.light.map(Light::of),
            mesh: inputs.plan.drawn_by(position).map(mesh_id),
            translation: item.transform.translation,
            rotation: item.transform.rotation,
            scale: item.transform.scale,
        })
        .collect()
}

fn meshes(inputs: &Inputs<'_>) -> Vec<Mesh> {
    inputs
        .meshes
        .iter()
        .enumerate()
        .map(|(index, written)| {
            let summary = &written.summary;
            let mesh = index;
            let first = inputs
                .plan
                .first(index)
                .and_then(|position| inputs.items.get(position));
            let id = mesh_id(index);
            Mesh {
                file: format!("{MESHES_FOLDER}/{id}.glb"),
                id,
                geometry: first.and_then(|item| item.geometry).map(str::to_owned),
                palette: first
                    .and_then(|item| item.drawn?.palette)
                    .map(|palette| palette.name.clone()),
                palette_id: inputs
                    .collected
                    .palette_of
                    .get(index)
                    .copied()
                    .flatten()
                    .map(palette_id),
                instances: inputs.plan.instances(index),
                bytes: summary.bytes,
                vertices: summary.vertices,
                triangles: summary.triangles,
                materials: inputs
                    .collected
                    .materials
                    .get(index)
                    .map_or_default(|drawn| {
                        drawn
                            .iter()
                            .enumerate()
                            .map(|(position, material)| {
                                mesh_material(inputs, mesh, position, material)
                            })
                            .collect()
                    }),
                rig: None,
            }
        })
        .collect()
}

fn written_position(inputs: &Inputs<'_>, position: usize) -> Option<String> {
    matches!(inputs.outcomes.get(position), Some(Outcome::Written { .. })).then(|| id_of(position))
}

fn written_id(
    inputs: &Inputs<'_>,
    file: Option<&TextureFile>,
    role: crate::TextureUse,
) -> Option<String> {
    let file = file?;
    let mode = crate::texture_encoding::TextureMode::drawn(file, role);
    written_position(inputs, inputs.collected.position_of(&file.path, mode)?)
}

fn palette_id(position: usize) -> String {
    format!("p{:04}", position + 1)
}

fn mesh_material(
    inputs: &Inputs<'_>,
    mesh: usize,
    index: usize,
    material: &DrawnMaterial,
) -> MeshMaterial {
    let normal = written_id(inputs, material.normal.as_ref(), crate::TextureUse::Normal);
    let authored = inputs
        .plan
        .first(mesh)
        .and_then(|item| inputs.items.get(item)?.drawn);
    MeshMaterial {
        index,
        name: material.name.clone(),
        source_material_ids: material.source_material_ids.clone(),
        archive_default: material.archive_default,
        authored_library: inputs
            .collected
            .authored_libraries
            .get(mesh)
            .copied()
            .flatten()
            .filter(|_| material.library_index.is_none()),
        fallback_slots: authored.map_or_default(|drawn| {
            crate::package_default_material::fallback_slots(material, drawn.materials)
        }),
        material: inputs
            .collected
            .described
            .get(mesh)
            .and_then(|described| described.get(index))
            .copied()
            .flatten()
            .map(material_id),
        textures: MaterialTextures {
            diffuse: written_id(
                inputs,
                material.diffuse.as_ref(),
                crate::TextureUse::Diffuse,
            ),
            normal,
            specular: written_id(
                inputs,
                material.specular.as_ref(),
                crate::TextureUse::Specular,
            ),
        },
    }
}

fn materials(inputs: &Inputs<'_>) -> Vec<MaterialDescription> {
    inputs
        .collected
        .descriptions
        .iter()
        .map(|described| {
            let mut description = described.description.clone();
            for (texture, position) in description
                .textures
                .iter_mut()
                .zip(&described.texture_positions)
            {
                texture.texture = position.and_then(|position| written_position(inputs, position));
            }
            settle_stencil(&mut description);
            description
        })
        .collect()
}

fn palettes(inputs: &Inputs<'_>) -> Vec<Palette> {
    let colours = |entry: &p4kx_model::TintEntry| PaletteColours {
        tint: entry.tint,
        specular: entry.specular,
        glossiness: entry.glossiness,
    };
    inputs
        .collected
        .palettes
        .iter()
        .enumerate()
        .map(|(position, palette)| Palette {
            id: palette_id(position),
            name: palette.name.clone(),
            primary: colours(&palette.primary),
            secondary: colours(&palette.secondary),
            tertiary: colours(&palette.tertiary),
            glass: palette.glass,
            decal: palette.decal,
        })
        .collect()
}

fn textures(inputs: &Inputs<'_>) -> Vec<Texture> {
    inputs
        .collected
        .wanted
        .iter()
        .zip(inputs.outcomes)
        .enumerate()
        .filter_map(|(position, (wanted, outcome))| match outcome {
            Outcome::Written {
                width,
                height,
                colour_space,
                encoding,
                alpha,
                source_metadata,
                format,
                dds_asset,
                dds_fallback,
                smoothness,
                ..
            } => {
                let id = id_of(position);
                Some(Texture {
                    file: format!("{TEXTURES_FOLDER}/{id}.{}", format.extension()),
                    id,
                    source: wanted.path.clone(),
                    format: *format,
                    bit_depth: 8,
                    colour_space: *colour_space,
                    encoding: Some(*encoding),
                    alpha: *alpha,
                    ddna_pair: smoothness
                        .as_ref()
                        .map(|value| Box::new(value.pairing.clone())),
                    source_metadata: source_metadata.clone(),
                    dds_asset: dds_asset.clone(),
                    dds_fallback: *dds_fallback,
                    width: *width,
                    height: *height,
                })
            }
            Outcome::Failed { .. } => None,
        })
        .chain(inputs.outcomes.iter().filter_map(companion_texture))
        .collect()
}

fn companion_texture(outcome: &Outcome) -> Option<Texture> {
    let Outcome::Written {
        width,
        height,
        smoothness: Some(value),
        ..
    } = outcome
    else {
        return None;
    };
    Some(Texture {
        id: value.id.clone(),
        file: format!("{TEXTURES_FOLDER}/{}.dds", value.id),
        source: value.source.clone(),
        format: crate::TextureFormat::Dds,
        bit_depth: 8,
        colour_space: ColourSpace::Linear,
        encoding: Some(crate::TextureEncoding::Data),
        alpha: crate::AlphaMeaning::None,
        ddna_pair: None,
        source_metadata: None,
        dds_asset: Some(Box::new(value.asset.clone())),
        dds_fallback: None,
        width: *width,
        height: *height,
    })
}

/// Returns the manifest's issue for an issue of the Assembly.
pub(crate) fn assembly_issue(issue: &AssemblyIssue) -> Issue {
    Issue {
        origin: IssueOrigin::Assembly,
        kind: issue.kind().code().to_owned(),
        path: issue.path().to_owned(),
        name: issue.name().to_owned(),
        skipped: issue.kind().skips(),
        reason: issue.kind().to_string(),
    }
}

fn issues(inputs: &Inputs<'_>) -> Vec<Issue> {
    let mut issues: Vec<Issue> = inputs.assembly_issues.iter().map(assembly_issue).collect();
    for (wanted, outcome) in inputs.collected.wanted.iter().zip(inputs.outcomes) {
        if let Outcome::Failed { reason, dds } = outcome {
            issues.push(Issue {
                origin: IssueOrigin::Texture,
                kind: dds
                    .map_or("texture", crate::DdsTextureFallback::code)
                    .to_owned(),
                path: wanted.file.clone(),
                name: wanted.material.clone(),
                skipped: false,
                reason: reason.clone(),
            });
        }
    }
    let mut refused: Vec<(&str, &str)> = Vec::new();
    for found in &inputs.collected.refused {
        if !refused.contains(&(found.file.as_str(), found.material.as_str())) {
            refused.push((found.file.as_str(), found.material.as_str()));
            issues.push(Issue {
                origin: IssueOrigin::Texture,
                kind: "texture_path".to_owned(),
                path: found.file.clone(),
                name: found.material.clone(),
                skipped: false,
                reason: "the texture file name is not a safe path".to_owned(),
            });
        }
    }
    issues.extend(inputs.collected.issues.iter().cloned());
    issues
}
