// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Archive;

use super::classes::ClassLookup;
use super::models::attach;
use super::tile_light::TileLightOwner;
use super::walk::walk;
use super::{Interior, InteriorForge, InteriorOptions, InteriorWatch};
use crate::{AssemblyError, ModelProvider};

impl Interior {
    /// Builds the Interior of the socpak container `layers.last()`, which was opened from the
    /// Entry `entry` of the Archive or container before it in `layers`; `layers` runs from the
    /// Archive that holds the game's files to the container.
    ///
    /// The container's description (`<name>.xml`) names the containers nested in it, each with
    /// its position and rotation in the parent's space, and the `.soc` files that place models;
    /// nested containers are entered recursively and their transforms composed, so a child is
    /// moved by the parent's rotation as well as its position. A container that is nested in
    /// itself is not entered again. A name is looked for in the container that names it, then in
    /// the containers around it, then below `Data` in the first Archive.
    ///
    /// The models are looked for inside the container of the `.soc` file first, then in the
    /// containers around it and last in the Archive, and are read at the level of detail in
    /// `options`, each distinct file once and in parallel; the library a model names is read
    /// once. Whatever cannot be resolved is left out and listed as an [`Issue`](crate::Issue).
    /// The parts, their nesting, the containers and the vertices are limited by
    /// [`MAX_INTERIOR_PARTS`](crate::MAX_INTERIOR_PARTS),
    /// [`MAX_INTERIOR_DEPTH`](crate::MAX_INTERIOR_DEPTH),
    /// [`MAX_INTERIOR_CONTAINERS`](crate::MAX_INTERIOR_CONTAINERS) and
    /// [`InteriorOptions::max_vertices`].
    ///
    /// # Errors
    ///
    /// Returns [`AssemblyError::Provider`] when a file cannot be read for every container, e.g.
    /// because an Archive key is missing.
    pub fn build(
        layers: &[&Archive],
        entry: &str,
        options: &InteriorOptions,
    ) -> Result<Self, AssemblyError> {
        Self::build_watched(layers, entry, options, &InteriorWatch::none())
    }

    /// Builds the Interior like [`Self::build`], asking `watch` between containers, for every
    /// model file and library it reads and between the steps that follow whether to stop, and
    /// telling it how far it has come.
    ///
    /// # Errors
    ///
    /// As [`Self::build`], and [`AssemblyError::Cancelled`] when `watch` asks to stop.
    pub fn build_watched(
        layers: &[&Archive],
        entry: &str,
        options: &InteriorOptions,
        watch: &InteriorWatch<'_>,
    ) -> Result<Self, AssemblyError> {
        Self::build_with(layers, entry, options, watch, None)
    }

    /// Builds the Interior like [`Self::build_watched`], looking the class Records of the `.soc`
    /// files' entities up in `forge`.
    ///
    /// An entity that names only a class Record (`EntityClassGUID`), such as a door, a panel or a
    /// cabinet, is placed as a prop with the model, the material library and the tint palette of
    /// the Record's `SGeometryResourceParams`; an entity of a class without geometry is left out
    /// as before. The tint palettes the `.soc` file chooses for its models and props, and the
    /// material library a prop names, are applied; a part without a palette of its own takes the
    /// default palette of its material library. Without `forge` none of this happens.
    ///
    /// # Errors
    ///
    /// As [`Self::build_watched`].
    pub fn build_with(
        layers: &[&Archive],
        entry: &str,
        options: &InteriorOptions,
        watch: &InteriorWatch<'_>,
        forge: Option<&InteriorForge<'_>>,
    ) -> Result<Self, AssemblyError> {
        Self::build_with_models(layers, entry, options, watch, (forge, None))
    }

    /// Builds the Interior like [`Self::build_with`], reading the models that `layers[0]`, the
    /// game's Archive, holds through `models` when given, e.g. a
    /// [`CachedModels`](crate::CachedModels), so that models read before are not read again.
    /// Models inside a container are always read from the container. `models` must read from the
    /// same Archive as `layers[0]`.
    ///
    /// # Errors
    ///
    /// As [`Self::build_watched`].
    pub fn build_with_models(
        layers: &[&Archive],
        entry: &str,
        options: &InteriorOptions,
        watch: &InteriorWatch<'_>,
        (forge, models): (Option<&InteriorForge<'_>>, Option<&dyn ModelProvider>),
    ) -> Result<Self, AssemblyError> {
        Self::build_owned(layers, entry, options, watch, (forge, models), None)
    }

    pub(crate) fn build_owned(
        layers: &[&Archive],
        entry: &str,
        options: &InteriorOptions,
        watch: &InteriorWatch<'_>,
        (forge, models): (Option<&InteriorForge<'_>>, Option<&dyn ModelProvider>),
        owner: Option<TileLightOwner<'_>>,
    ) -> Result<Self, AssemblyError> {
        let classes = forge.map(|forge| forge as &dyn ClassLookup);
        let mut walked = walk(layers, entry, options, watch, classes, owner)?;
        let default_material = attach(&mut walked, options, watch, classes, models)?;
        Ok(Self {
            parts: walked.parts,
            issues: walked.issues,
            lod: options.lod,
            containers: walked.containers,
            default_material,
        })
    }
}
