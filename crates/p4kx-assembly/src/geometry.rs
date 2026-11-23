// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_dataforge::{Guid, StructValue, Value, ValueError};
use p4kx_extraction::SourceLocation;

use crate::character_path::is_rigged_path_outside_human_rigs;
use crate::read::{struct_field, text};

#[path = "geometry_source.rs"]
mod source;

pub use source::{GeometrySource, visit_geometry_sources};

#[path = "geometry_origins.rs"]
mod origins;

use self::origins::MaterialPosition;
pub(crate) use self::origins::{GeometryComponent, GeometrySelectors};
use crate::MaterialVariantSelection;

#[path = "geometry_variants.rs"]
mod variants;

pub(crate) use self::variants::AppearanceOnlyVariant;

/// The authored tint palette selection, with geometry fallback when the material node names one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaletteRef {
    preferred: PaletteCandidate,
    fallback: Option<Box<PaletteCandidate>>,
}

#[derive(Debug, Clone)]
pub(crate) struct PaletteCandidate {
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

impl PartialEq for PaletteCandidate {
    fn eq(&self, other: &Self) -> bool {
        self.guid == other.guid && self.child_path == other.child_path
    }
}

impl Eq for PaletteCandidate {}

impl PaletteRef {
    /// Returns the GUID of the `TintPaletteTree` Record.
    #[must_use]
    pub const fn guid(&self) -> Guid {
        self.preferred.guid
    }

    /// Returns the preferred palette's child path, or an empty string for the root.
    #[must_use]
    pub fn child_path(&self) -> &str {
        &self.preferred.child_path
    }

    pub(crate) const fn new(
        guid: Guid,
        child_path: String,
        selector: PaletteReferenceSource,
    ) -> Self {
        Self {
            preferred: PaletteCandidate {
                guid,
                child_path,
                selector,
            },
            fallback: None,
        }
    }

    pub(crate) fn with_selector(mut self, selector: PaletteReferenceSource) -> Self {
        self.preferred.selector = selector;
        self
    }

    fn with_geometry_fallback(mut self, geometry: Option<Self>) -> Self {
        self.fallback = geometry
            .map(|palette| palette.preferred)
            .filter(|candidate| *candidate != self.preferred)
            .map(Box::new);
        self
    }

    pub(crate) fn candidates(&self) -> impl Iterator<Item = &PaletteCandidate> {
        std::iter::once(&self.preferred).chain(self.fallback.as_deref())
    }

    pub(crate) fn read(data: &StructValue<'_>) -> Option<Self> {
        let reference = struct_field(data, "Palette")?;
        let guid = match reference.field("RootRecord")? {
            Value::Reference(link) => link.guid(),
            Value::Error(ValueError::DanglingReference { guid }) => *guid,
            _ => return None,
        };
        Some(Self::new(
            guid,
            text(reference, "ChildPath").trim().to_owned(),
            PaletteReferenceSource::Named(None),
        ))
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
    pub(crate) tag_locations: Box<[SourceLocation]>,
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

    /// Returns the geometry's own palette or the one named by the same item's main geometry.
    pub(crate) fn palette_with_main<'a>(
        &'a self,
        main: Option<&'a Self>,
    ) -> Option<&'a PaletteRef> {
        self.palette().or_else(|| main.and_then(Self::palette))
    }

    pub(crate) fn inherit_palette(&mut self, main: Option<&Self>) {
        if self.palette.is_none() {
            self.palette = self.palette_with_main(main).cloned();
        }
    }

    /// Returns whether the model file is a `.cdf`, `.chr` or `.skin` file outside
    /// `Objects/Characters/Human`, the folder of the human body rigs.
    #[must_use]
    pub fn is_rigged_file_outside_human_rigs(&self) -> bool {
        is_rigged_path_outside_human_rigs(&self.path)
    }

    /// Reads the main geometry of a `SGeometryResourceParams` component.
    pub(crate) fn read(component: &StructValue<'_>, owner: GeometryComponent) -> Option<Self> {
        let data = struct_field(struct_field(component, "Geometry")?, "Geometry")?;
        let path = model_path(data)?;
        let material_node = struct_field(component, "Material");
        let component_material = material_node.and_then(material_path);
        let mut geometry = Self::from_data(data, owner, &[], path, component_material);
        if let Some(palette) = material_node.and_then(PaletteRef::read) {
            let source =
                PaletteReferenceSource::Named(Some(owner.at(".Material.Palette.RootRecord")));
            geometry.palette = Some(
                palette
                    .with_selector(source)
                    .with_geometry_fallback(geometry.palette.take()),
            );
        }
        Some(geometry)
    }

    /// Returns the physical Record field that names this geometry.
    #[must_use]
    pub fn model_field(&self) -> Option<SourceLocation> {
        self.origins.model_field()
    }

    pub(crate) fn variant_position(&self) -> Option<SourceLocation> {
        self.origins.model.as_ref().map(|position| position.at(""))
    }

    /// Returns the physical Record field that names this geometry's selected material.
    #[must_use]
    pub fn material_field(&self) -> Option<SourceLocation> {
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

    pub(crate) fn apply_material_variant(&mut self, variant: &MaterialVariantSelection<'_>) {
        if let Some(material) = variant.material() {
            self.material = Some(material.to_owned());
            self.origins.material = variant
                .material_location()
                .map(|location| MaterialPosition::Authored(std::sync::Arc::new(location)));
        }
        if let Some(palette) = variant.palette() {
            self.palette = Some(palette);
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

    fn from_data(
        data: &StructValue<'_>,
        owner: GeometryComponent,
        slots: &[usize],
        path: &str,
        component_material: Option<&str>,
    ) -> Self {
        let material = component_material.or_else(|| material_path(data));
        let mut origins = GeometrySelectors::of(
            owner,
            slots,
            material.is_some() && component_material.is_none(),
        );
        if component_material.is_some() {
            origins.material = Some(MaterialPosition::Component(owner));
        }
        let palette = PaletteRef::read(data).map(|palette| {
            let location = origins
                .model
                .as_ref()
                .map(|position| position.at(".Geometry.Palette.RootRecord"));
            palette.with_selector(PaletteReferenceSource::Named(location))
        });
        Self {
            path: path.to_owned(),
            material: material.map(str::to_owned),
            palette,
            origins,
        }
    }
}

/// Returns the nonempty material path named by this original node or geometry.
pub(crate) fn material_path<'source>(data: &StructValue<'source>) -> Option<&'source str> {
    let path = text(struct_field(data, "Material")?, "path");
    (!path.is_empty()).then_some(path)
}

fn model_path<'source>(data: &StructValue<'source>) -> Option<&'source str> {
    let path = text(struct_field(data, "Geometry")?, "path");
    (!path.is_empty()).then_some(path)
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

#[cfg(test)]
#[path = "geometry_primary_material_tests.rs"]
mod primary_material_tests;
