// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use p4kx_extraction::{ArchiveMaterialRead, ModelArchiveSource};
use p4kx_model::{MaterialStatus, Model, Transform};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};

use crate::assembly::ModelDefinition;
use crate::build_attachments::attached;
use crate::build_gear::load_models;
use crate::build_hidden::hide_worn;
use crate::build_library_name::{self, LibraryName};
use crate::build_models::{Loaded, choose_models};
use crate::build_palettes::Palettes;
use crate::build_place::{place, under_skin};
use crate::build_zones::BodyCut;
use crate::creature_rig;
use crate::flatten::{Draft, flatten};
use crate::geometry::GeometrySelectors;
use crate::placement::{is_character_item, is_unskinned};
use crate::{
    Assembly, AssemblyError, AssemblyOptions, EntityNode, Issue, IssueKind, ModelProvider,
    ObjectContainerRef, PaletteProvider, Part, SkeletonProblem,
};

pub(crate) type Library = Arc<ArchiveMaterialRead>;
pub(crate) type Libraries = HashMap<String, Library>;

impl Assembly {
    /// Builds the Assembly of the Loadout tree `tree`: the Entity's geometry and, for every
    /// attached Entity, its geometry placed at its port.
    ///
    /// The models are read at the level of detail in `options`, each distinct file once and in
    /// parallel; a material library the Entity's geometry names replaces the one the model names,
    /// and each library is read once. A part whose Entity or geometry cannot be resolved is left
    /// out or kept without a model, and the [`Issue`] says why. The number of parts, their depth
    /// and the vertices of their models are limited by [`MAX_ASSEMBLY_PARTS`](crate::MAX_ASSEMBLY_PARTS),
    /// [`MAX_ASSEMBLY_DEPTH`](crate::MAX_ASSEMBLY_DEPTH) and [`MAX_ASSEMBLY_VERTICES`](crate::MAX_ASSEMBLY_VERTICES).
    ///
    /// # Errors
    ///
    /// Returns [`AssemblyError::Provider`] when `models` fails for every file, e.g. because an
    /// Archive key is missing.
    pub fn build(
        tree: &EntityNode,
        models: &dyn ModelProvider,
        options: &AssemblyOptions,
    ) -> Result<Self, AssemblyError> {
        Self::build_inner(tree, models, None, options)
    }

    /// Builds the Assembly like [`Self::build`] and resolves the tint palette of each part with
    /// `palettes`: the one its Entity's geometry names, else the one of its nearest ancestor, else
    /// the default of its material library. Each distinct palette is read once and shared by the
    /// parts that use it.
    ///
    /// A palette that cannot be resolved is an [`Issue`] of the first part that names it; the
    /// part falls back to the palette of its nearest ancestor, else the library's default
    /// palette, else it keeps the colours of its library.
    ///
    /// # Errors
    ///
    /// As [`Self::build`].
    pub fn build_with_palettes(
        tree: &EntityNode,
        models: &dyn ModelProvider,
        palettes: &dyn PaletteProvider,
        options: &AssemblyOptions,
    ) -> Result<Self, AssemblyError> {
        Self::build_inner(tree, models, Some(palettes), options)
    }

    fn build_inner(
        tree: &EntityNode,
        models: &dyn ModelProvider,
        palettes: Option<&dyn PaletteProvider>,
        options: &AssemblyOptions,
    ) -> Result<Self, AssemblyError> {
        let mut flat = flatten(tree, options.max_parts);
        hide_worn(&mut flat);
        let mut issues = flat.issues;
        let mut drafts = flat.drafts;
        let loaded = load_models(&mut drafts, models, options, &mut issues)?;
        let chosen = choose_models(&drafts, &loaded, options.max_vertices, &mut issues);
        let libraries = read_libraries(&drafts, &chosen, models);
        let unnamed = Arc::new(ArchiveMaterialRead::unobserved(MaterialStatus::Unnamed));
        let shared = (&libraries, &unnamed);
        let mut reported = HashSet::new();
        let mut characters = HashSet::new();
        let mut palettes = Palettes::new(palettes);
        let cut = BodyCut::of(tree, &drafts, &chosen, &mut issues);
        let mut parts: Vec<Part> = Vec::with_capacity(drafts.len());
        for (index, (draft, model)) in drafts.iter().zip(&chosen).enumerate() {
            let library = materials_of(draft, *model, shared, &mut reported, &mut issues);
            note_character(draft, *model, &mut characters, &mut issues);
            let attachments = attached(draft, *model, shared, &mut reported, &mut issues);
            let (palette, unresolved) = palettes.of_part(draft, *model, &library.materials.status);
            issues.extend(unresolved.into_iter().map(|kind| issue(draft, kind)));
            let rig = under_skin(draft, &parts)
                .then(|| creature_rig::anchor_skeleton(&parts, &issues, options.lod))
                .flatten();
            let prop = is_prop(draft, *model, options.lod);
            let placement = place(draft, &parts, *model, rig, prop, &mut issues);
            let world = world_of(draft, &parts, &placement.local);
            let geometry = rendered_geometry(cut.as_ref(), index, draft, *model, &mut issues);
            parts.push(Part {
                name: draft.name.clone(),
                path: draft.path.clone(),
                guid: draft.node.guid(),
                class_name: draft.class_name().to_owned(),
                landing_gear: placement.gear,
                parent: draft.parent,
                local: placement.local,
                world,
                placement: placement.kind,
                placement_source: placement.port,
                model_placement: placement.nodes,
                geometry: draft.geometry_path().map(str::to_owned),
                model: geometry.model,
                model_archive_sources: geometry.sources,
                model_definition: ModelDefinition::of(*model, library.from_model),
                geometry_selectors: geometry.selectors,
                materials: library.materials,
                palette,
                attachments,
                hidden: draft.hidden,
            });
        }
        creature_rig::note_fit(&parts, options.lod, &mut issues);
        creature_rig::note_loadout(tree, &mut issues);
        Ok(Self {
            parts,
            issues,
            lod: options.lod,
            interior: None,
            containers: containers_of(&drafts),
        })
    }
}

#[derive(Default)]
struct RenderedGeometry {
    model: Option<Arc<Model>>,
    sources: Arc<[ModelArchiveSource]>,
    selectors: GeometrySelectors,
}

fn rendered_geometry(
    cut: Option<&BodyCut>,
    index: usize,
    draft: &Draft<'_>,
    model: Option<&Loaded>,
    issues: &mut Vec<Issue>,
) -> RenderedGeometry {
    let Some(loaded) = model else {
        return RenderedGeometry::default();
    };
    let rendered = cut.map_or_else(
        || Arc::clone(&loaded.model),
        |cut| cut.model_of(index, draft, loaded, issues),
    );
    let sources = model_inputs(loaded, &rendered);
    let selectors = draft
        .geometry
        .map_or_else(GeometrySelectors::default, |geometry| {
            geometry.selectors(draft.library.is_none())
        });
    RenderedGeometry {
        model: Some(rendered),
        sources,
        selectors,
    }
}

fn model_inputs(loaded: &Loaded, rendered: &Model) -> Arc<[ModelArchiveSource]> {
    if std::ptr::eq(loaded.model.as_ref(), rendered) {
        Arc::clone(&loaded.archive_sources)
    } else {
        loaded
            .archive_sources
            .iter()
            .filter(|source| source.supplies(rendered))
            .cloned()
            .collect()
    }
}

/// Returns whether the part is a prop: an Entity that is no character item and whose model at the
/// level `level` of detail has no skinned mesh, so no weights that follow the skeleton.
fn is_prop(draft: &Draft<'_>, model: Option<&Loaded>, level: u32) -> bool {
    !is_character_item(draft.node.attach_type())
        && model.is_some_and(|loaded| is_unskinned(&loaded.model, level))
}

/// Returns the transform of a part to the Assembly's: its parent's world transform and its own.
fn world_of(draft: &Draft<'_>, parts: &[Part], local: &Transform) -> Transform {
    draft
        .parent
        .and_then(|parent| parts.get(parent))
        .map_or(*local, |parent| parent.world.then(local))
}

struct SelectedLibrary {
    materials: Library,
    from_model: bool,
}

/// Returns the materials a part draws with, noting a library that has a problem once.
fn materials_of(
    draft: &Draft<'_>,
    model: Option<&Loaded>,
    libraries: (&Libraries, &Library),
    reported: &mut HashSet<String>,
    issues: &mut Vec<Issue>,
) -> SelectedLibrary {
    let name = model
        .and_then(|loaded| build_library_name::choose(draft, loaded).map(|name| (loaded, name)));
    let from_model = matches!(name, Some((_, LibraryName::Model(_))));
    let key = name.map(|(loaded, name)| library_key_of(name.path(), &loaded.entry));
    let materials = materials_for(draft, key, libraries, reported, issues);
    SelectedLibrary {
        materials,
        from_model,
    }
}

/// Returns the materials of the library cached under `key`, noting a problem of it once.
pub(crate) fn materials_for(
    draft: &Draft<'_>,
    key: Option<String>,
    (libraries, unnamed): (&Libraries, &Library),
    reported: &mut HashSet<String>,
    issues: &mut Vec<Issue>,
) -> Library {
    let materials = key
        .as_ref()
        .and_then(|key| libraries.get(key))
        .map_or_else(|| Arc::clone(unnamed), Arc::clone);
    if let (Some(key), Some(reason)) = (
        key,
        materials
            .status
            .problem()
            .or_else(|| materials.status.layer_problem()),
    ) && reported.insert(key)
    {
        issues.push(issue(draft, IssueKind::MaterialLibrary { reason }));
    }
    materials
}

/// Returns the object containers each Entity of `drafts` names, shared with its node; a docked
/// vehicle that is not expanded and a hidden part name none, as their own ports are left out or
/// not drawn.
fn containers_of(drafts: &[Draft<'_>]) -> Vec<Arc<[ObjectContainerRef]>> {
    let none: Arc<[ObjectContainerRef]> = Arc::new([]);
    drafts
        .iter()
        .map(|draft| {
            if draft.names_containers() {
                Arc::clone(&draft.node.object_containers)
            } else {
                Arc::clone(&none)
            }
        })
        .collect()
}

/// Lists the attachments of a character definition that are not drawn and the problem with its
/// skeleton file, once per definition.
fn note_character(
    draft: &Draft<'_>,
    model: Option<&Loaded>,
    seen: &mut HashSet<String>,
    issues: &mut Vec<Issue>,
) {
    let (Some(loaded), Some(path)) = (model, draft.geometry_path()) else {
        return;
    };
    if (loaded.left_out.is_empty() && loaded.skeleton.is_none()) || !seen.insert(file_key(path)) {
        return;
    }
    if !loaded.left_out.is_empty() {
        let files = loaded.left_out.clone();
        issues.push(issue(draft, IssueKind::AttachmentsLeftOut { files }));
    }
    match &loaded.skeleton {
        Some(SkeletonProblem::Missing { path }) => {
            let path = path.clone();
            issues.push(issue(draft, IssueKind::SkeletonMissing { path }));
        }
        Some(SkeletonProblem::Unreadable { path, reason }) => {
            let (path, reason) = (path.clone(), reason.clone());
            issues.push(issue(draft, IssueKind::SkeletonUnreadable { path, reason }));
        }
        None => {}
    }
}

pub(crate) fn issue(draft: &Draft<'_>, kind: IssueKind) -> Issue {
    Issue {
        path: draft.path.clone(),
        name: draft.class_name().to_owned(),
        kind,
    }
}

/// Returns a key for a path that ignores case, the separator kind and a leading `Data\`.
pub(crate) fn file_key(path: &str) -> String {
    let key = path
        .trim_start_matches(['/', '\\'])
        .replace('/', "\\")
        .to_ascii_lowercase();
    key.strip_prefix("data\\")
        .map_or_else(|| key.clone(), str::to_owned)
}

/// Returns the key under which the library `name` of the model at `entry` is cached.
pub(crate) fn library_key_of(name: &str, entry: &str) -> String {
    if name.trim_start().starts_with(['/', '\\']) {
        return name.to_ascii_lowercase();
    }
    let key = file_key(name);
    let key = key.strip_suffix(".mtl").unwrap_or(&key).to_owned();
    if key.contains('\\') {
        key
    } else {
        let folder = entry
            .rsplit_once(['\\', '/'])
            .map_or("", |(folder, _)| folder);
        format!("{}\\{key}", folder.to_ascii_lowercase())
    }
}

/// Reads every distinct material library the parts draw with once, in parallel.
fn read_libraries(
    drafts: &[Draft<'_>],
    chosen: &[Option<&Loaded>],
    provider: &dyn ModelProvider,
) -> Libraries {
    let mut seen = HashSet::new();
    let wanted: Vec<(String, &str, &str)> = drafts
        .iter()
        .zip(chosen)
        .filter_map(|(draft, loaded)| Some((draft, (*loaded)?)))
        .flat_map(|(draft, loaded)| {
            let part = build_library_name::choose(draft, loaded).map(|selected| {
                let name = selected.path();
                (
                    library_key_of(name, &loaded.entry),
                    loaded.entry.as_str(),
                    name,
                )
            });
            let attached = loaded.attachments.iter().filter_map(|attached| {
                let name = attached.model.material_library.as_deref()?;
                Some((
                    library_key_of(name, &attached.entry),
                    attached.entry.as_str(),
                    name,
                ))
            });
            part.into_iter().chain(attached)
        })
        .filter(|(key, _, _)| seen.insert(key.clone()))
        .collect();
    wanted
        .par_iter()
        .map(|(key, entry, name)| {
            (
                key.clone(),
                Arc::new(provider.library_observed(entry, name)),
            )
        })
        .collect()
}
