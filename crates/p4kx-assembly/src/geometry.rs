// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_dataforge::{Guid, StructValue, Value, ValueError};
use p4kx_extraction::SourceLocation;

use crate::character_path::is_rigged_path_outside_human_rigs;
use crate::read::{indexed_struct_items, struct_field, text};

#[path = "geometry_origins.rs"]
mod origins;

pub(crate) use self::origins::{GeometryComponent, GeometrySelectors};

/// The tint palette an Entity's geometry names (`Palette.TintPaletteRef`): the `TintPaletteTree`
/// Record and the child of its tree.
#[derive(Debug, Clone)]
pub struct PaletteRef {
    pub(crate) guid: Guid,
    pub(crate) child_path: String,
    pub(crate) selector: PaletteReferenceSource,
}

#[derive(Debug, Clone)]
pub(crate) enum PaletteReferenceSource {
    Named(Option<SourceLocation>),
    IncludedPath(Option<crate::PaletteRecordSelection>),
    SocEntity,
}

impl PartialEq for PaletteRef {
    fn eq(&self, other: &Self) -> bool {
        self.guid == other.guid && self.child_path == other.child_path
    }
}

impl Eq for PaletteRef {}

impl PaletteRef {
    /// Returns the GUID of the `TintPaletteTree` Record.
    #[must_use]
    pub const fn guid(&self) -> Guid {
        self.guid
    }

    /// Returns the path of the child palette below the Record's root, or an empty string for the
    /// root.
    #[must_use]
    pub fn child_path(&self) -> &str {
        &self.child_path
    }
}

/// The geometry an Entity Record names (`SGeometryResourceParams`): the model file, the material
/// library that replaces the library the model names itself and the tint palette.
#[derive(Debug, Clone)]
pub struct Geometry {
    pub(crate) path: String,
    pub(crate) material: Option<String>,
    pub(crate) palette: Option<PaletteRef>,
    origins: GeometrySelectors,
}

impl PartialEq for Geometry {
    fn eq(&self, other: &Self) -> bool {
        self.path == other.path && self.material == other.material && self.palette == other.palette
    }
}

impl Eq for Geometry {}

/// A geometry an Entity Record names for the tags of a sub-node (`SubGeometry`), e.g. `Male` or
/// `Female` for the body a piece of armour is worn on.
#[derive(Debug, Clone)]
pub struct GeometryVariant {
    pub(crate) tags: Vec<String>,
    pub(crate) geometry: Geometry,
}

impl PartialEq for GeometryVariant {
    fn eq(&self, other: &Self) -> bool {
        self.tags == other.tags && self.geometry == other.geometry && self.slots() == other.slots()
    }
}

impl Eq for GeometryVariant {}

impl GeometryVariant {
    fn slots(&self) -> &[usize] {
        self.geometry
            .origins
            .model
            .as_ref()
            .map_or(&[], |source| source.slots.as_ref())
    }

    /// Returns the tags of the sub-node, split at white space.
    #[must_use]
    pub fn tags(&self) -> &[String] {
        &self.tags
    }

    /// Returns the geometry of the sub-node.
    #[must_use]
    pub const fn geometry(&self) -> &Geometry {
        &self.geometry
    }

    /// Returns whether the sub-node carries the tag `tag`, ignoring ASCII case.
    #[must_use]
    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|own| own.eq_ignore_ascii_case(tag))
    }
}

impl Geometry {
    /// Returns the model file as the Record spells it, a path below `Data`, e.g.
    /// `Objects\Spaceships\Ships\AEGS\Gladius\AEGS_Gladius.cga`.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Returns the material library the Entity chooses, a path below `Data`, or `None` when the
    /// Record names none and the model's own library applies.
    #[must_use]
    pub fn material(&self) -> Option<&str> {
        self.material.as_deref()
    }

    /// Returns the tint palette the Entity chooses, or `None` when the Record names none.
    #[must_use]
    pub const fn palette(&self) -> Option<&PaletteRef> {
        self.palette.as_ref()
    }

    /// Returns whether the model file is a `.cdf`, `.chr` or `.skin` file outside
    /// `Objects/Characters/Human`, the folder of the human body rigs.
    #[must_use]
    pub fn is_rigged_file_outside_human_rigs(&self) -> bool {
        is_rigged_path_outside_human_rigs(&self.path)
    }

    /// Reads the main geometry of a `SGeometryResourceParams` component.
    pub(crate) fn read(component: &StructValue<'_>, owner: GeometryComponent) -> Option<Self> {
        Self::of_node(struct_field(component, "Geometry")?, owner, &[])
    }

    /// Reads the geometry of the sub-nodes of the main node of a `SGeometryResourceParams`
    /// component that name a model, with their tags: the children of the main node first, then
    /// the sub-nodes nested in them, which carry the tags of the nodes they are nested in as well
    /// as their own.
    pub(crate) fn read_variants(
        component: &StructValue<'_>,
        owner: GeometryComponent,
    ) -> Vec<GeometryVariant> {
        let Some(node) = struct_field(component, "Geometry") else {
            return Vec::new();
        };
        let mut variants = Vec::new();
        let mut level = vec![(node, Vec::<String>::new(), Vec::<usize>::new())];
        while !level.is_empty() {
            let mut next = Vec::new();
            for (parent, inherited, source) in &level {
                for (slot, sub) in indexed_struct_items(parent, "SubGeometry") {
                    let slots: Vec<_> = source.iter().copied().chain([slot]).collect();
                    let mut tags = inherited.clone();
                    tags.extend(text(sub, "Tags").split_whitespace().map(str::to_owned));
                    if let Some(geometry) = Self::of_node(sub, owner, &slots) {
                        variants.push(GeometryVariant {
                            tags: tags.clone(),
                            geometry,
                        });
                    }
                    next.push((sub, tags, slots));
                }
            }
            level = next;
        }
        variants
    }

    /// Returns the physical Record field that names this geometry.
    #[must_use]
    pub fn model_field(&self) -> Option<SourceLocation> {
        self.origins.model_field()
    }

    pub(crate) fn material_field(&self) -> Option<SourceLocation> {
        self.origins.material_field()
    }

    pub(crate) fn selectors(&self, material_from_geometry: bool) -> GeometrySelectors {
        GeometrySelectors {
            model: self.origins.model.clone(),
            material: material_from_geometry
                .then(|| self.origins.material.clone())
                .flatten(),
        }
    }

    pub(crate) fn inherit_material(&mut self, other: &Self) {
        self.material.clone_from(&other.material);
        self.origins.material.clone_from(&other.origins.material);
    }

    pub(crate) fn override_model(&mut self, path: String) {
        self.path = path;
        self.origins.model = None;
    }

    #[cfg(test)]
    pub(crate) fn fixture(path: &str) -> Self {
        Self {
            path: path.to_owned(),
            material: None,
            palette: None,
            origins: GeometrySelectors::default(),
        }
    }

    fn of_node(node: &StructValue<'_>, owner: GeometryComponent, slots: &[usize]) -> Option<Self> {
        let data = struct_field(node, "Geometry")?;
        let path = struct_field(data, "Geometry").map(|resource| text(resource, "path"))?;
        if path.is_empty() {
            return None;
        }
        let material = struct_field(data, "Material")
            .map(|resource| text(resource, "path"))
            .filter(|path| !path.is_empty());
        let origins = GeometrySelectors::of(owner, slots, material.is_some());
        let mut palette = read_palette(data);
        if let Some(palette) = &mut palette {
            palette.selector = PaletteReferenceSource::Named(
                origins
                    .model
                    .as_ref()
                    .map(|position| position.at(".Geometry.Palette.RootRecord")),
            );
        }
        Some(Self {
            path: path.to_owned(),
            material: material.map(str::to_owned),
            palette,
            origins,
        })
    }
}

fn read_palette(data: &StructValue<'_>) -> Option<PaletteRef> {
    let reference = struct_field(data, "Palette")?;
    let guid = match reference.field("RootRecord")? {
        Value::Reference(link) => link.guid(),
        Value::Error(ValueError::DanglingReference { guid }) => *guid,
        _ => return None,
    };
    Some(PaletteRef {
        guid,
        child_path: text(reference, "ChildPath").trim().to_owned(),
        selector: PaletteReferenceSource::Named(None),
    })
}

#[cfg(test)]
mod tests {
    use super::Geometry;
    use crate::BodyVariant;

    #[test]
    fn the_human_skeleton_and_skin_are_no_rigged_files_outside_the_human_rigs() {
        for body in BodyVariant::ALL {
            assert!(!Geometry::fixture(body.skeleton()).is_rigged_file_outside_human_rigs());
            assert!(!Geometry::fixture(body.skin()).is_rigged_file_outside_human_rigs());
        }
    }

    #[test]
    fn character_files_outside_the_human_rigs_are_told_by_extension_and_folder() {
        for path in [
            "Objects/Characters/Creatures/x/x_skel.chr",
            "objects\\characters\\creatures\\x\\x.CDF",
            "/Objects/Characters/Vanduul/drone/body.skin",
            "Objects/Spaceships/Thrusters/nozzle.cdf",
        ] {
            assert!(
                Geometry::fixture(path).is_rigged_file_outside_human_rigs(),
                "{path}"
            );
        }
        for path in [
            "Data/Objects/Characters/Human/male_v7/export/bhm_skeleton_v7.chr",
            "Objects/Characters/Human/heads/male/head.skin",
            "Objects/Spaceships/Ships/AEGS/Gladius/gladius.cga",
            "Objects/Characters/Creatures/x/x.cgf",
            "Objects/Characters/Creatures/x/skin",
        ] {
            assert!(
                !Geometry::fixture(path).is_rigged_file_outside_human_rigs(),
                "{path}"
            );
        }
    }
}

#[cfg(test)]
#[path = "geometry_source_variants_tests.rs"]
mod source_variants_tests;
