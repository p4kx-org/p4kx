// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::Issue as AssemblyIssue;
use p4kx_gltf::{DrawnMaterial, GlbOptions, GlbSummary, TextureFile};

use crate::PackageInfo;
use crate::collect::{Collected, id_of as material_id};
use crate::describe::settle_stencil;
use crate::items::Item;
use crate::manifest::{
    Basis, Coordinates, EntityRef, FORMAT_VERSION, Format, Generator, Issue, IssueOrigin,
    MESHES_FOLDER, Manifest, MaterialTextures, Mesh, MeshMaterial, Node, Options, Source,
    TEXTURES_FOLDER, Texture, TextureFormat, Unit,
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
    pub(crate) meshes: &'a [GlbSummary],
}

/// Builds the manifest of a package whose files are written.
pub(crate) fn manifest(inputs: &Inputs<'_>) -> Manifest {
    Manifest {
        format: Format::P4kxBlenderPackage,
        version: FORMAT_VERSION.to_owned(),
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
        materials: materials(inputs),
        palettes: palettes(inputs),
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
        .map(|(index, summary)| {
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
                    .map(|drawn| {
                        drawn
                            .iter()
                            .enumerate()
                            .map(|(position, material)| {
                                mesh_material(inputs, mesh, position, material)
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
                rig: None,
            }
        })
        .collect()
}

fn written_path(inputs: &Inputs<'_>, path: &str) -> Option<String> {
    let position = inputs.collected.position_of(path)?;
    matches!(inputs.outcomes.get(position), Some(Outcome::Written { .. })).then(|| id_of(position))
}

fn written_id(inputs: &Inputs<'_>, file: Option<&TextureFile>) -> Option<String> {
    written_path(inputs, &file?.path)
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
    let normal = written_id(inputs, material.normal.as_ref());
    MeshMaterial {
        index,
        name: material.name.clone(),
        material: inputs
            .collected
            .described
            .get(mesh)
            .and_then(|described| described.get(index))
            .copied()
            .flatten()
            .map(material_id),
        textures: MaterialTextures {
            diffuse: written_id(inputs, material.diffuse.as_ref()),
            normal,
            specular: written_id(inputs, material.specular.as_ref()),
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
            for (texture, path) in description.textures.iter_mut().zip(&described.paths) {
                texture.texture = path.as_deref().and_then(|path| written_path(inputs, path));
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
                alpha,
                ..
            } => {
                let id = id_of(position);
                Some(Texture {
                    file: format!("{TEXTURES_FOLDER}/{id}.png"),
                    id,
                    source: wanted.path.clone(),
                    format: TextureFormat::Png,
                    bit_depth: 8,
                    colour_space: *colour_space,
                    alpha: *alpha,
                    width: *width,
                    height: *height,
                })
            }
            Outcome::Failed(_) => None,
        })
        .collect()
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
        if let Outcome::Failed(reason) = outcome {
            issues.push(Issue {
                origin: IssueOrigin::Texture,
                kind: "texture".to_owned(),
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
