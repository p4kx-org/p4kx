// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, HashMap, HashSet};

use p4kx_assembly::ModelProvider;
use p4kx_extraction::{ArchiveDefaultMaterial, EntryOrigin};
use p4kx_gltf::{DrawnMaterial, drawn_materials_with_fallback};
use p4kx_model::{LayerBudget, Material, MaterialLibrary, TintPalette};

use crate::describe::{describe, library_path};
use crate::items::Item;
use crate::manifest::{Issue, IssueOrigin};
use crate::manifest_materials::MaterialDescription;
use crate::material_library_sources::MaterialLibrarySources;
use crate::plan::MeshPlan;
use crate::texture_encoding::TextureMode;
use crate::textures::Wanted;
use crate::{PackageResult, PackageWriteContext};

#[path = "collect_layers.rs"]
mod layers;
#[path = "collect_textures.rs"]
mod textures;

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

#[derive(Clone, Copy)]
struct LibraryInput<'a> {
    library: &'a MaterialLibrary,
    entry: Option<&'a EntryOrigin>,
}

#[derive(Debug, Clone, Copy)]
struct MaterialOccurrence {
    library: usize,
    description: usize,
}

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
    keys: HashMap<MaterialKey, Vec<MaterialOccurrence>>,
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
        fallback: Option<&ArchiveDefaultMaterial>,
        context: &PackageWriteContext<'_>,
    ) -> PackageResult<Self> {
        let fallback = fallback.and_then(|fallback| {
            Some(LibraryInput {
                library: fallback.library()?,
                entry: fallback.original(),
            })
        });
        let mut collected = Self {
            layer_budget: models.layer_budget(),
            ..Self::default()
        };
        for index in 0..plan.len() {
            context.check()?;
            let item = plan.first(index).and_then(|position| items.get(position));
            collected.add_mesh(item, level, models, fallback)?;
        }
        collected.cut_issues();
        collected.library_sources.finish(context)?;
        Ok(collected)
    }

    fn add_mesh(
        &mut self,
        item: Option<&Item<'_>>,
        level: u32,
        models: &dyn ModelProvider,
        fallback: Option<LibraryInput<'_>>,
    ) -> PackageResult<()> {
        let drawn = item.and_then(|item| item.drawn).map_or_default(|drawn| {
            drawn_materials_with_fallback(
                drawn.model,
                drawn.materials,
                level,
                fallback.and_then(|fallback| fallback.library.materials.first()),
            )
        });
        let palette = item
            .and_then(|item| item.drawn?.palette)
            .map(|palette| self.palette_index(palette));
        self.palette_of.push(palette);
        self.folder = item
            .and_then(|item| item.geometry)
            .and_then(|geometry| {
                geometry
                    .replace('\\', "/")
                    .rsplit_once('/')
                    .map(|(folder, _)| folder.to_ascii_lowercase())
            })
            .unwrap_or_default();
        let library = item.and_then(|item| {
            let drawn = item.drawn?;
            Some(LibraryInput {
                library: drawn.materials.library()?,
                entry: drawn.material_entry,
            })
        });
        let authored = self.library_sources.capture_fallback(
            library.map(|library| library.library),
            library.and_then(|library| library.entry),
            &drawn,
            &mut self.issues,
        );
        self.authored_libraries.push(authored);
        let described = drawn
            .iter()
            .map(|material| {
                let (library, position) = material
                    .library_index
                    .map_or((fallback, 0), |position| (library, position));
                library.map_or(Ok(None), |library| {
                    self.describe(library, position, Some(material), models, 0)
                })
            })
            .collect::<PackageResult<_>>()?;
        self.described.push(described);
        self.materials.push(drawn);
        Ok(())
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
        input: LibraryInput<'_>,
        position: usize,
        drawn: Option<&DrawnMaterial>,
        models: &dyn ModelProvider,
        depth: usize,
    ) -> PackageResult<Option<usize>> {
        let LibraryInput { library, entry } = input;
        let Some(original) = library.materials.get(position) else {
            return Ok(None);
        };
        let source_library = self
            .library_sources
            .capture(library, entry, &mut self.issues);
        let adjusted = original.hair_cap_of_body(&library.path);
        let material = adjusted.as_ref().unwrap_or(original);
        let index = (!library.single).then_some(position);
        let (path, identity) = self.library_identity(library);
        let key = (identity, index);
        let occurrences = self.keys.entry(key).or_default();
        if let Some(known) = occurrences
            .iter()
            .find(|occurrence| occurrence.library == source_library)
            .map(|occurrence| occurrence.description)
        {
            let shallower = self
                .descriptions
                .get(known)
                .is_some_and(|known| known.followed_at.is_none_or(|at| depth < at));
            if shallower && depth < MAX_LAYER_DEPTH {
                self.follow_layers(known, material, models, depth)?;
            }
            if let Some(drawn) = drawn {
                self.want_drawn(drawn);
            }
            return Ok(Some(known));
        }
        let at = self.descriptions.len();
        let first = occurrences.first().map(|occurrence| occurrence.library);
        occurrences.push(MaterialOccurrence {
            library: source_library,
            description: at,
        });
        if let Some(issue) =
            self.library_sources
                .conflict(first, source_library, &path, &material.name)
        {
            self.issues.push(issue);
        }
        let mut description = describe(
            id_of(at),
            &path,
            index,
            material,
            library.default_palette.as_deref(),
        );
        description.source_library = Some(source_library);
        self.add_description(description, input, position, material, drawn)?;
        if depth < MAX_LAYER_DEPTH {
            self.follow_layers(at, material, models, depth)?;
        } else if !material.layers.is_empty() {
            self.cut.insert(at, (path, material.name.clone()));
        }
        Ok(Some(at))
    }

    fn library_identity(&self, library: &MaterialLibrary) -> (String, String) {
        let relative = p4kx_model::material_library_path(&library.path);
        let mapped_root = relative.is_none() && library.target.is_some();
        let path = relative
            .unwrap_or_else(|| library_path(library.target.as_deref().unwrap_or(&library.path)));
        let identity = if mapped_root || path.contains('/') {
            path.to_ascii_lowercase()
        } else {
            format!("{}/{}", self.folder, path.to_ascii_lowercase())
        };
        (path, identity)
    }

    fn add_description(
        &mut self,
        mut description: MaterialDescription,
        input: LibraryInput<'_>,
        position: usize,
        material: &Material,
        drawn: Option<&DrawnMaterial>,
    ) -> PackageResult<()> {
        if let Some(source) = input.library.emission_source(position)
            && crate::package_material_emission::relevant(&source)
            && let Some(source_library) = description.source_library
        {
            let evidence = self.library_sources.evidence(source_library, input.entry)?;
            description.emission_source = Some(crate::PackageMaterialEmission::of(
                &source,
                source_library,
                material,
                evidence,
            )?);
        }
        let texture_positions =
            self.want_textures(material, drawn, description.emission_source.as_ref());
        if let Some(drawn) = drawn {
            self.want_drawn(drawn);
        }
        self.note_truncation(&description.library, material);
        self.descriptions.push(Described {
            description,
            texture_positions,
            followed_at: None,
        });
        Ok(())
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
