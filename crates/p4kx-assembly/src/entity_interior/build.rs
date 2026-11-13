// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use p4kx_dataforge::Guid;
use p4kx_extraction::{Archive, Entry, PathIndex};

use super::EntityInterior;
use super::budget::Budget;
use super::lookup::{check_tree, find, node_path};
use crate::interior::tile_light::TileLightOwner;
use crate::{
    Assembly, AssemblyError, Attachment, ContainerSource, EntityNode, Interior, InteriorForge,
    InteriorOptions, InteriorProgress, InteriorWatch, IssueKind, ModelProvider, ObjectContainerRef,
};

impl EntityInterior {
    /// Builds the Interior of `assembly`, which was built from the Entity `tree`: each object
    /// container the Record of the root Entity or of an attached Entity names is looked up in
    /// `archive`, built like [`Interior::build`] and placed at its bone of that Entity's model,
    /// moved by the Record's offset.
    ///
    /// A container that cannot be found or opened, a bone the Entity's model lacks (the container
    /// then sits at the Entity's origin) and everything an Interior cannot resolve are listed in
    /// [`Self::issues`]. The limits of `options` hold for the Assembly as a whole: the
    /// containers, the parts and the vertices of every container are counted together, and the
    /// vertices of the hull too; when one is used up, the remaining containers are left out with
    /// `too_many_containers`, `too_many_parts` or `too_many_vertices`.
    ///
    /// # Errors
    ///
    /// Returns [`AssemblyError::Provider`] when a file cannot be read for every container, e.g.
    /// because the Archive key is missing or wrong, and [`AssemblyError::TreeMismatch`] when
    /// `tree` is not the Entity `assembly` was built from.
    pub fn build(
        archive: &Archive,
        assembly: &Assembly,
        tree: &EntityNode,
        options: &InteriorOptions,
    ) -> Result<Self, AssemblyError> {
        Self::build_watched(archive, assembly, tree, options, &InteriorWatch::none())
    }

    /// Builds the Interior like [`Self::build`], asking `watch` before each container and while
    /// it reads the models whether to stop, and telling it how far it has come.
    ///
    /// # Errors
    ///
    /// As [`Self::build`], and [`AssemblyError::Cancelled`] when `watch` asks to stop.
    pub fn build_watched(
        archive: &Archive,
        assembly: &Assembly,
        tree: &EntityNode,
        options: &InteriorOptions,
        watch: &InteriorWatch<'_>,
    ) -> Result<Self, AssemblyError> {
        Self::build_with(archive, assembly, tree, options, watch, None)
    }

    /// Builds the Interior like [`Self::build_watched`], with the class Records of the
    /// containers' entities looked up in `forge`, see [`Interior::build_with`].
    ///
    /// # Errors
    ///
    /// As [`Self::build_watched`].
    pub fn build_with(
        archive: &Archive,
        assembly: &Assembly,
        tree: &EntityNode,
        options: &InteriorOptions,
        watch: &InteriorWatch<'_>,
        forge: Option<&InteriorForge<'_>>,
    ) -> Result<Self, AssemblyError> {
        Self::build_with_models(archive, assembly, tree, options, watch, (forge, None))
    }

    /// Builds the Interior like [`Self::build_with`], reading the models of `archive` through
    /// `models` when given, e.g. a [`CachedModels`](crate::CachedModels), so that models read
    /// before are not read again, see [`Interior::build_with_models`].
    ///
    /// `models` must read from the same Archive as `archive`.
    ///
    /// # Errors
    ///
    /// As [`Self::build_watched`].
    pub fn build_with_models(
        archive: &Archive,
        assembly: &Assembly,
        tree: &EntityNode,
        options: &InteriorOptions,
        watch: &InteriorWatch<'_>,
        (forge, models): (Option<&InteriorForge<'_>>, Option<&dyn ModelProvider>),
    ) -> Result<Self, AssemblyError> {
        check_tree(assembly, tree)?;
        let index = PathIndex::new(archive);
        let mut built = Self::default();
        let budget = Budget::new(assembly, options);
        let owners = vehicle_owners(tree);
        let total = assembly.containers.iter().map(|list| list.len()).sum();
        let named = assembly
            .containers
            .iter()
            .enumerate()
            .flat_map(|(part, list)| list.iter().map(move |reference| (part, reference)));
        for (done, (part, reference)) in named.enumerate() {
            if watch.is_cancelled() {
                return Err(AssemblyError::Cancelled);
            }
            watch.report(InteriorProgress::Container { done, total });
            let owner = assembly.parts.get(part);
            let at = node_path(owner.map_or("", |owner| owner.path()));
            if let Some(kind) = budget.exhausted(&built) {
                built.note(&at, reference, kind);
                break;
            }
            let Some((entry, opened)) = built.open(archive, &index, &at, reference)? else {
                continue;
            };
            let model = owner.and_then(crate::Part::model);
            let owner_path = owner.map_or("", crate::Part::path);
            let placement = built.place(&at, reference, model);
            let limited = budget.options(&built);
            let tile_owner =
                (reference.source() == ContainerSource::Vehicle).then(|| TileLightOwner {
                    frame: placement.tile_owner,
                    model,
                    vehicle: owner.and_then(|part| {
                        owners
                            .get(owner_path)
                            .and_then(|by_guid| by_guid.get(&part.guid()))
                            .copied()
                            .flatten()
                    }),
                });
            let interior = Interior::build_owned(
                &[archive, &opened],
                entry.path(),
                &limited,
                watch,
                (forge, models),
                tile_owner,
            )?;
            built.add(part, owner_path, reference, placement.transform, interior);
        }
        Ok(built)
    }

    fn open<'a>(
        &mut self,
        archive: &Archive,
        index: &PathIndex<'a>,
        at: &str,
        reference: &ObjectContainerRef,
    ) -> Result<Option<(&'a Entry, Archive)>, AssemblyError> {
        let Some(entry) = find(index, reference.file()) else {
            self.note(
                at,
                reference,
                IssueKind::ContainerMissing {
                    name: reference.file().to_owned(),
                },
            );
            return Ok(None);
        };
        match archive.open_container(entry) {
            Ok(opened) => Ok(Some((entry, opened))),
            Err(error) if error.is_key_required() || error.is_wrong_key() => {
                Err(AssemblyError::Provider(Box::new(error)))
            }
            Err(error) => {
                self.note(
                    at,
                    reference,
                    IssueKind::ContainerUnreadable {
                        name: reference.file().to_owned(),
                        reason: error.to_string(),
                    },
                );
                Ok(None)
            }
        }
    }
}

type VehicleOwners<'a> = HashMap<String, HashMap<Guid, Option<&'a crate::VehicleImplementation>>>;
fn vehicle_owners(tree: &EntityNode) -> VehicleOwners<'_> {
    let mut owners = HashMap::new();
    if let Some(vehicle) = tree.vehicle() {
        owners
            .entry(String::new())
            .or_insert_with(HashMap::new)
            .insert(tree.guid(), Some(vehicle));
    }
    tree.visit_ports(|path, route| {
        let Some(node) = route.last().and_then(|port| match port.attachment() {
            Attachment::Entity(node) => Some(node.as_ref()),
            _ => None,
        }) else {
            return;
        };
        owners
            .entry(path.to_owned())
            .or_insert_with(HashMap::new)
            .entry(node.guid())
            .and_modify(|selected| *selected = None)
            .or_insert_with(|| node.vehicle());
    });
    owners
}
