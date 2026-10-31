// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, HashMap, HashSet};

use p4kx_assembly::ModelProvider;
use p4kx_gltf::{DrawnMaterial, drawn_materials, texture_path};
use p4kx_model::{LayerBudget, Material, MaterialLayer, MaterialLibrary, TintPalette};

use crate::TextureUse;
use crate::describe::{describe, layer_shares, library_path};
use crate::items::Item;
use crate::manifest::{Issue, IssueOrigin};
use crate::manifest_materials::MaterialDescription;
use crate::plan::MeshPlan;
use crate::roles::{classify_in, has_smoothness};
use crate::textures::Wanted;

/// How many material layers deep a layer's own layers are followed.
const MAX_LAYER_DEPTH: usize = 8;

/// A material description and, for each of its texture slots, the path of the texture it names.
#[derive(Debug)]
pub(crate) struct Described {
    pub(crate) description: MaterialDescription,
    pub(crate) paths: Vec<Option<String>>,
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
    pub(crate) materials: Vec<Vec<DrawnMaterial>>,
    pub(crate) described: Vec<Vec<Option<usize>>>,
    pub(crate) descriptions: Vec<Described>,
    pub(crate) palettes: Vec<TintPalette>,
    pub(crate) palette_of: Vec<Option<usize>>,
    pub(crate) wanted: Vec<Wanted>,
    pub(crate) refused: Vec<Refused>,
    pub(crate) issues: Vec<Issue>,
    index_of: HashMap<String, usize>,
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
    ) -> Self {
        let mut collected = Self {
            layer_budget: models.layer_budget(),
            ..Self::default()
        };
        for index in 0..plan.len() {
            let item = plan.first(index).and_then(|position| items.get(position));
            let drawn = item
                .and_then(|item| item.drawn)
                .map(|drawn| drawn_materials(drawn.model, drawn.materials, level))
                .unwrap_or_default();
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
            let described = drawn
                .iter()
                .map(|material| {
                    let position = material.library_index?;
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
        let adjusted = original.hair_cap_of_body(&library.path);
        let material = adjusted.as_ref().unwrap_or(original);
        let index = (!library.single).then_some(position);
        let path = library_path(&library.path);
        let identity = if path.contains('/') {
            path.to_ascii_lowercase()
        } else {
            format!("{}/{}", self.folder, path.to_ascii_lowercase())
        };
        let key = (identity, index);
        if let Some(&known) = self.keys.get(&key) {
            let shallower = self
                .descriptions
                .get(known)
                .is_some_and(|known| known.followed_at.is_none_or(|at| depth < at));
            if shallower && depth < MAX_LAYER_DEPTH {
                self.follow_layers(known, material, models, depth);
            }
            return Some(known);
        }
        let at = self.descriptions.len();
        let description = describe(
            id_of(at),
            &path,
            index,
            material,
            library.default_palette.as_deref(),
        );
        let paths = self.want_textures(material, drawn);
        self.keys.insert(key, at);
        self.descriptions.push(Described {
            description,
            paths,
            followed_at: None,
        });
        for cut in &material.truncated {
            self.issues.push(Issue {
                origin: IssueOrigin::Material,
                kind: "material".to_owned(),
                path: path.clone(),
                name: material.name.clone(),
                skipped: false,
                reason: format!("the material file is cut off: {cut}"),
            });
        }
        if depth < MAX_LAYER_DEPTH {
            self.follow_layers(at, material, models, depth);
        } else if !material.layers.is_empty() {
            self.cut.insert(at, (path, material.name.clone()));
        }
        Some(at)
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
        let path = layer.attribute("Path").map(str::trim).unwrap_or_default();
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
    ) -> Vec<Option<String>> {
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
                let role = drawn
                    .and_then(|drawn| gltf_role(drawn, &path))
                    .unwrap_or_else(|| classify_in(material, &slot.map, &slot.file));
                let gloss = role == TextureUse::Normal && has_smoothness(&slot.file);
                self.want(&path, &slot.file, role, gloss, &material.name);
                Some(path)
            })
            .collect()
    }

    fn want(&mut self, path: &str, file: &str, role: TextureUse, gloss: bool, material: &str) {
        let identity = path.to_ascii_lowercase();
        if self.index_of.contains_key(&identity) {
            return;
        }
        self.index_of.insert(identity, self.wanted.len());
        self.wanted.push(Wanted {
            path: path.to_owned(),
            file: file.to_owned(),
            material: material.to_owned(),
            role,
            gloss,
        });
    }

    /// Returns the position among the wanted textures of the texture at `path`.
    pub(crate) fn position_of(&self, path: &str) -> Option<usize> {
        self.index_of.get(&path.to_ascii_lowercase()).copied()
    }
}

/// Returns the id of the description at position `at`.
pub(crate) fn id_of(at: usize) -> String {
    format!("x{:04}", at + 1)
}

fn gltf_role(drawn: &DrawnMaterial, path: &str) -> Option<TextureUse> {
    [
        (&drawn.diffuse, TextureUse::Diffuse),
        (&drawn.normal, TextureUse::Normal),
        (&drawn.specular, TextureUse::Specular),
    ]
    .into_iter()
    .find_map(|(file, role)| {
        file.as_ref()
            .is_some_and(|file| file.path.eq_ignore_ascii_case(path))
            .then_some(role)
    })
}
