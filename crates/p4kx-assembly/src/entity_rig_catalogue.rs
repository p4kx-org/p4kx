// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::convert::Infallible;
use std::sync::Arc;

use p4kx_dataforge::Guid;
use p4kx_extraction::{DataForgeOrigin, SourceLocation};

use crate::gear_params::catalogue_parameters;
use crate::{ArchiveModels, Attachment, EntityNode, GearPlacementSource, LandingGear};

#[path = "entity_rig_files.rs"]
mod files;
pub(crate) use files::{ArchiveRigInputs, RigEntry, read_rig_entry};
pub use files::{RigDefinition, RigFile, RigFileRead, RigInput, RigInputs};

pub use crate::gear_params::RigParameter;

#[cfg(test)]
#[path = "entity_rig_catalogue_tests.rs"]
mod tests;

/// One position in the resolved Entity Loadout that names a potential animation rig.
#[derive(Debug)]
#[non_exhaustive]
pub enum RigPlacement {
    /// The root Entity of the resolved Loadout.
    Root,
    /// An Entity selected by a Loadout port.
    Attachment {
        /// Exact port path, in the resolved Loadout's authored order.
        port_path: String,
    },
    /// One landing gear declaration below its owning Entity.
    LandingGear {
        /// Port path of the owning Entity, empty for the root.
        owner_path: String,
        /// The stored gear ordinal within the owning landing-system Record.
        ordinal: usize,
        /// Authored placement bone name when supplied.
        bone: Option<String>,
    },
}

/// Metadata discovery for one Entity or landing-gear position.
#[derive(Debug)]
#[non_exhaustive]
pub struct EntityRig {
    /// Position in the supplied Loadout.
    pub placement: RigPlacement,
    /// Physical DataForge owner index when known.
    pub record_index: Option<u32>,
    /// GUID of the owning Entity, independent of its Record position.
    pub owner: Guid,
    /// Observed DataForge of the owner when it belongs to the same Archive.
    pub owner_source: Option<Arc<DataForgeOrigin>>,
    /// Geometry selected by the resolved Loadout, before mesh loading or fallback.
    pub geometry: Option<String>,
    /// Original selectors that supplied the geometry path.
    pub geometry_selectors: Vec<SourceLocation>,
    /// Character definition read on the path to the skeleton, if applicable.
    pub definition: Option<RigFile<RigDefinition>>,
    /// Skeleton read separately from the model and clips.
    pub skeleton: Option<RigFile<Option<p4kx_model::SkeletonSource>>>,
    /// Authored parameter rows associated with the declared skeleton.
    pub parameters: Option<RigFile<Vec<RigParameter>>>,
}

/// Discovers original rig inputs for the root, attachments and landing gear in Loadout order.
///
/// Missing or unreadable inputs stay with their positions. Clip payloads and mesh companions are
/// outside this discovery pass.
#[must_use]
pub fn discover_entity_rigs(models: &ArchiveModels<'_>, root: &EntityNode) -> Vec<EntityRig> {
    let mut inputs = ArchiveRigInputs {
        models,
        check: || Ok::<_, Infallible>(()),
    };
    match discover_entity_rigs_using(models, root, &mut inputs) {
        Ok(rigs) => rigs,
        Err(never) => match never {},
    }
}

pub(crate) fn discover_entity_rigs_using<I: RigInputs>(
    models: &ArchiveModels<'_>,
    root: &EntityNode,
    inputs: &mut I,
) -> Result<Vec<EntityRig>, I::Error> {
    inputs.check()?;
    let mut catalogue = Catalogue {
        models,
        owner_source: root
            .forge_origin
            .as_ref()
            .filter(|source| source.matches_archive(models.archive)),
        rigs: Vec::new(),
        files: files::RigFiles::default(),
    };
    catalogue.append(root, "", RigPlacement::Root, inputs)?;
    crate::tree_walk::try_visit(root.ports(), |path, route| {
        inputs.check()?;
        if let Some(port) = route.last()
            && let Attachment::Entity(child) = port.attachment()
        {
            catalogue.append(
                child,
                path,
                RigPlacement::Attachment {
                    port_path: path.to_owned(),
                },
                inputs,
            )?;
        }
        Ok(())
    })?;
    Ok(catalogue.rigs)
}

struct Catalogue<'model, 'archive> {
    models: &'model ArchiveModels<'archive>,
    owner_source: Option<&'model Arc<DataForgeOrigin>>,
    rigs: Vec<EntityRig>,
    files: files::RigFiles,
}

impl Catalogue<'_, '_> {
    fn append<I: RigInputs>(
        &mut self,
        node: &EntityNode,
        path: &str,
        placement: RigPlacement,
        inputs: &mut I,
    ) -> Result<(), I::Error> {
        let geometry = if matches!(placement, RigPlacement::Root) {
            node.geometry()
        } else {
            node.own_skin
                .as_ref()
                .map(|skin| &skin.geometry)
                .or_else(|| node.geometry())
        };
        self.append_rig(
            node,
            placement,
            geometry.map(crate::Geometry::path),
            geometry
                .and_then(crate::Geometry::model_field)
                .into_iter()
                .collect(),
            inputs,
        )?;
        if node.is_docked_vehicle() && !node.is_expanded() {
            return Ok(());
        }
        for gear in node.landing_gear() {
            inputs.check()?;
            self.append_gear(node, path, gear, inputs)?;
        }
        Ok(())
    }

    fn append_gear<I: RigInputs>(
        &mut self,
        node: &EntityNode,
        path: &str,
        gear: &LandingGear,
        inputs: &mut I,
    ) -> Result<(), I::Error> {
        let geometry = Some(gear.geometry()).filter(|path| !path.is_empty());
        let source = GearPlacementSource::of(node, gear, false);
        self.append_rig(
            node,
            RigPlacement::LandingGear {
                owner_path: path.to_owned(),
                ordinal: gear.source.ordinal,
                bone: gear.source.authored_bone.then(|| gear.bone().to_owned()),
            },
            geometry,
            geometry
                .into_iter()
                .flat_map(|_| source.model_selectors())
                .collect(),
            inputs,
        )
    }

    fn append_rig<I: RigInputs>(
        &mut self,
        node: &EntityNode,
        placement: RigPlacement,
        geometry: Option<&str>,
        geometry_selectors: Vec<SourceLocation>,
        inputs: &mut I,
    ) -> Result<(), I::Error> {
        let rig = self.read(node, placement, geometry, geometry_selectors, inputs)?;
        self.rigs.push(rig);
        Ok(())
    }

    fn read<I: RigInputs>(
        &mut self,
        node: &EntityNode,
        placement: RigPlacement,
        geometry: Option<&str>,
        geometry_selectors: Vec<SourceLocation>,
        inputs: &mut I,
    ) -> Result<EntityRig, I::Error> {
        let definition = geometry
            .filter(|path| crate::cdf::is_character_definition(path))
            .map(|path| {
                files::read(
                    self.models,
                    path,
                    &mut self.files.definitions,
                    files::definition,
                    inputs,
                )
            })
            .transpose()?;
        let skeleton_path = definition.as_ref().map_or(geometry, |definition| {
            definition
                .value()
                .and_then(|value| value.skeleton.as_deref())
        });
        let skeleton = skeleton_path
            .map(|path| {
                files::read(
                    self.models,
                    path,
                    &mut self.files.skeletons,
                    files::skeleton,
                    inputs,
                )
            })
            .transpose()?;
        let parameters = skeleton_path
            .map(|path| {
                files::read(
                    self.models,
                    &crate::gear_clip::params_path(path),
                    &mut self.files.parameters,
                    catalogue_parameters,
                    inputs,
                )
            })
            .transpose()?;
        Ok(EntityRig {
            placement,
            record_index: node.record_index(),
            owner: node.guid(),
            owner_source: self
                .owner_source
                .filter(|_| node.record_index().is_some())
                .cloned(),
            geometry: geometry.map(str::to_owned),
            geometry_selectors,
            definition,
            skeleton,
            parameters,
        })
    }
}
