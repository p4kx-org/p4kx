// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, HashMap, HashSet};

use p4kx_assembly::ModelProvider;
use p4kx_gltf::{DrawnMaterial, drawn_materials_with_fallback, texture_path};
use p4kx_model::{LayerBudget, Material, MaterialLayer, MaterialLibrary, TintPalette};

use crate::TextureUse;
use crate::describe::{describe, layer_shares, library_path};
use crate::items::Item;
use crate::manifest::{Issue, IssueOrigin};
use crate::manifest_materials::MaterialDescription;
use crate::material_library_sources::MaterialLibrarySources;
use crate::plan::MeshPlan;
use crate::roles::{classify_in, has_smoothness};
use crate::texture_encoding::TextureMode;
use crate::textures::Wanted;

/// How many material layers deep a layer's own layers are followed.
const MAX_LAYER_DEPTH: usize = 8;

/// A material description and the selected output texture of each slot.
#[derive(Debug)]
pub(crate) struct Described {
    pub(crate) description: MaterialDescription,
    pub(crate) texture_positions: Vec<Option<usize>>,
    followed_at: Option<usize>,
}

/// A texture file that has no safe path, and the material that names it.
#[derive(Debug)]
pub(crate) struct Refused {
    pub(crate) file: String,
    pub(crate) material: String,
}

type MaterialKey = (String, Option<usize>);

/// The materials of every distinct mesh, their descriptions, their palettes and the distinct
/// texture files they name.
#[derive(Debug, Default)]
pub(crate) struct Collected {
    pub(crate) library_sources: MaterialLibrarySources,
    pub(crate) materials: Vec<Vec<DrawnMaterial>>,
    pub(crate) described: Vec<Vec<Option<usize>>>,
    pub(crate) authored_libraries: Vec<Option<usize>>,
    pub(crate) descriptions: Vec<Described>,
    pub(crate) palettes: Vec<TintPalette>,
    pub(crate) palette_of: Vec<Option<usize>>,
    pub(crate) wanted: Vec<Wanted>,
    pub(crate) refused: Vec<Refused>,
    pub(crate) issues: Vec<Issue>,
    index_of: HashMap<(String, TextureMode), usize>,
    keys: HashMap<MaterialKey, usize>,
    layer_budget: LayerBudget,
    layer_issues: HashSet<(String, String, String)>,
    cut: BTreeMap<usize, (String, String)>,
    folder: String,
}

impl Collected {
    /// Reads the materials of each distinct mesh of `plan` at `level`.
    pub(crate) fn of(
        items: &[Item<'_>],
        plan: &MeshPlan,
        level: u32,
        models: &dyn ModelProvider,
        fallback: Option<&MaterialLibrary>,
    ) -> Self {
        let mut collected = Self {
            layer_budget: models.layer_budget(),
            ..Self::default()
        };
        for index in 0..plan.len() {
            let item = plan.first(index).and_then(|position| items.get(position));
            let drawn = item.and_then(|item| item.drawn).map_or_default(|drawn| {
                drawn_materials_with_fallback(
                    drawn.model,
                    drawn.materials,
                    level,
                    fallback.and_then(|library| library.materials.first()),
                )
            });
            let palette = item
                .and_then(|item| item.drawn?.palette)
                .map(|palette| collected.palette_index(palette));
            collected.palette_of.push(palette);
            collected.folder = item
                .and_then(|item| item.geometry)
                .and_then(|geometry| {
                    geometry
                        .replace('\\', "/")
                        .rsplit_once('/')
                        .map(|(folder, _)| folder.to_ascii_lowercase())
                })
                .unwrap_or_default();
            let library = item.and_then(|item| item.drawn?.materials.library());
            let authored =
                collected
                    .library_sources
                    .capture_fallback(library, &drawn, &mut collected.issues);
            collected.authored_libraries.push(authored);
            let described = drawn
                .iter()
                .map(|material| {
                    let (library, position) = material
                        .library_index
                        .map_or((fallback, 0), |position| (library, position));
                    collected.describe(library?, position, Some(material), models, 0)
                })
                .collect();
            collected.described.push(described);
            collected.materials.push(drawn);
        }
        collected.cut_issues();
        collected
    }

    fn palette_index(&mut self, palette: &TintPalette) -> usize {
        self.palettes
            .iter()
            .position(|known| known == palette)
            .unwrap_or_else(|| {
                self.palettes.push(palette.clone());
                self.palettes.len() - 1
            })
    }

    fn describe(
        &mut self,
        library: &MaterialLibrary,
        position: usize,
        drawn: Option<&DrawnMaterial>,
        models: &dyn ModelProvider,
        depth: usize,
    ) -> Option<usize> {
        let original = library.materials.get(position)?;
        let source_library = self.library_sources.capture(library, &mut self.issues);
        let adjusted = original.hair_cap_of_body(&library.path);
        let material = adjusted.as_ref().unwrap_or(original);
        let index = (!library.single).then_some(position);
        let relative = p4kx_model::material_library_path(&library.path);
        let mapped_root = relative.is_none() && library.target.is_some();
        let path = relative
            .unwrap_or_else(|| library_path(library.target.as_deref().unwrap_or(&library.path)));
        let identity = if mapped_root || path.contains('/') {
            path.to_ascii_lowercase()
        } else {
            format!("{}/{}", self.folder, path.to_ascii_lowercase())
        };
        let key = (identity, index);
        if let Some(&known) = self.keys.get(&key) {
            self.check_source(known, source_library, &path, &material.name);
            let shallower = self
                .descriptions
                .get(known)
                .is_some_and(|known| known.followed_at.is_none_or(|at| depth < at));
            if shallower && depth < MAX_LAYER_DEPTH {
                self.follow_layers(known, material, models, depth);
            }
            if let Some(drawn) = drawn {
                self.want_drawn(drawn);
            }
            return Some(known);
        }
        let at = self.descriptions.len();
        let mut description = describe(
            id_of(at),
            &path,
            index,
            material,
            library.default_palette.as_deref(),
        );
        description.source_library = Some(source_library);
        let texture_positions = self.want_textures(material, drawn);
        if let Some(drawn) = drawn {
            self.want_drawn(drawn);
        }
        self.keys.insert(key, at);
        self.descriptions.push(Described {
            description,
            texture_positions,
            followed_at: None,
        });
        self.note_truncation(&path, material);
        if depth < MAX_LAYER_DEPTH {
            self.follow_layers(at, material, models, depth);
        } else if !material.layers.is_empty() {
            self.cut.insert(at, (path, material.name.clone()));
        }
        Some(at)
    }

    fn note_truncation(&mut self, path: &str, material: &Material) {
        for cut in &material.truncated {
            self.issues.push(Issue {
                origin: IssueOrigin::Material,
                kind: "material".to_owned(),
                path: path.to_owned(),
                name: material.name.clone(),
                skipped: false,
                reason: format!("the material file is cut off: {cut}"),
            });
        }
    }

    fn check_source(&mut self, known: usize, actual: usize, path: &str, name: &str) {
        let previous = self
            .descriptions
            .get(known)
            .and_then(|known| known.description.source_library);
        if let Some(issue) = self.library_sources.conflict(previous, actual, path, name) {
            self.issues.push(issue);
        }
    }

    fn follow_layers(
        &mut self,
        at: usize,
        material: &Material,
        models: &dyn ModelProvider,
        depth: usize,
    ) {
        if let Some(parent) = self.descriptions.get_mut(at) {
            parent.followed_at = Some(depth);
        }
        self.cut.remove(&at);
        let mut resolved = vec![false; material.layers.len()];
        for (number, layer) in material.layers.iter().enumerate() {
            let found = self.layer_material(material, layer, models, depth);
            if let Some(found) = found
                && let Some(slot) = self
                    .descriptions
                    .get_mut(at)
                    .and_then(|parent| parent.description.layers.get_mut(number))
            {
                slot.material = Some(id_of(found));
                if let Some(flag) = resolved.get_mut(number) {
                    *flag = true;
                }
            }
        }
        if let Some(parent) = self.descriptions.get_mut(at) {
            parent.description.layer_shares = layer_shares(material, |number| {
                resolved.get(number).copied().unwrap_or(false)
            });
        }
    }

    fn cut_issues(&mut self) {
        for (path, name) in std::mem::take(&mut self.cut).into_values() {
            self.issues.push(Issue {
                origin: IssueOrigin::Material,
                kind: "layer_material".to_owned(),
                path,
                name,
                skipped: false,
                reason: format!("layers are followed {MAX_LAYER_DEPTH} files deep"),
            });
        }
    }

    fn layer_material(
        &mut self,
        parent: &Material,
        layer: &MaterialLayer,
        models: &dyn ModelProvider,
        depth: usize,
    ) -> Option<usize> {
        let path = layer.attribute("Path").map_or_default(str::trim);
        if path.is_empty() {
            return None;
        }
        let status = models.layer_library(path, &mut self.layer_budget);
        let Some(library) = status.library() else {
            let problem = status.problem().unwrap_or_default();
            self.layer_issue(parent, path, &problem);
            return None;
        };
        let Some(position) = library.layer_position(layer.attribute("Submtl").unwrap_or_default())
        else {
            self.layer_issue(parent, path, "the layer file has no such sub-material");
            return None;
        };
        self.describe(library, position, None, models, depth + 1)
    }

    fn layer_issue(&mut self, parent: &Material, path: &str, reason: &str) {
        let first =
            self.layer_issues
                .insert((path.to_owned(), parent.name.clone(), reason.to_owned()));
        if !first {
            return;
        }
        self.issues.push(Issue {
            origin: IssueOrigin::Material,
            kind: "layer_material".to_owned(),
            path: path.to_owned(),
            name: parent.name.clone(),
            skipped: false,
            reason: reason.to_owned(),
        });
    }

    fn want_textures(
        &mut self,
        material: &Material,
        drawn: Option<&DrawnMaterial>,
    ) -> Vec<Option<usize>> {
        material
            .textures
            .iter()
            .map(|slot| {
                let Some(path) = texture_path(&slot.file) else {
                    if !slot.file.trim_start().starts_with('$') {
                        self.refused.push(Refused {
                            file: slot.file.clone(),
                            material: material.name.clone(),
                        });
                    }
                    return None;
                };
                let role = classify_in(material, &slot.map, &slot.file);
                let mode = drawn
                    .and_then(|drawn| drawn_mode(drawn, &path, role))
                    .unwrap_or_else(|| TextureMode::of(role, has_smoothness(&slot.file)));
                Some(self.want(&path, &slot.file, mode, &material.name))
            })
            .collect()
    }

    fn want_drawn(&mut self, drawn: &DrawnMaterial) {
        for (texture, role) in [
            (drawn.diffuse.as_ref(), TextureUse::Diffuse),
            (drawn.normal.as_ref(), TextureUse::Normal),
            (drawn.specular.as_ref(), TextureUse::Specular),
        ] {
            if let Some(file) = texture {
                self.want(
                    &file.path,
                    &file.file,
                    TextureMode::drawn(file, role),
                    &drawn.name,
                );
            }
        }
    }

    fn want(&mut self, path: &str, file: &str, mode: TextureMode, material: &str) -> usize {
        let identity = (path.to_ascii_lowercase(), mode);
        if let Some(&position) = self.index_of.get(&identity) {
            return position;
        }
        let position = self.wanted.len();
        self.index_of.insert(identity, position);
        self.wanted.push(Wanted {
            path: path.to_owned(),
            file: file.to_owned(),
            material: material.to_owned(),
            mode,
        });
        position
    }

    pub(crate) fn position_of(&self, path: &str, mode: TextureMode) -> Option<usize> {
        self.index_of
            .get(&(path.to_ascii_lowercase(), mode))
            .copied()
    }
}

#[cfg(test)]
mod tests;

/// Returns the id of the description at position `at`.
pub(crate) fn id_of(at: usize) -> String {
    format!("x{:04}", at + 1)
}

fn drawn_mode(drawn: &DrawnMaterial, path: &str, role: TextureUse) -> Option<TextureMode> {
    let role_for = |file: &p4kx_gltf::TextureFile, role| {
        file.path
            .eq_ignore_ascii_case(path)
            .then(|| TextureMode::drawn(file, role))
    };
    match role {
        TextureUse::Diffuse | TextureUse::Mask | TextureUse::Stencil => drawn
            .diffuse
            .as_ref()
            .and_then(|file| role_for(file, TextureUse::Diffuse)),
        TextureUse::Normal => drawn
            .normal
            .as_ref()
            .and_then(|file| role_for(file, TextureUse::Normal)),
        TextureUse::Specular => drawn
            .specular
            .as_ref()
            .and_then(|file| role_for(file, TextureUse::Specular)),
        _ => None,
    }
}
