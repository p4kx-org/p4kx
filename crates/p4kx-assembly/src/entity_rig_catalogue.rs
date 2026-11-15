// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::convert::Infallible;
use std::sync::Arc;

use p4kx_dataforge::Guid;
use p4kx_extraction::{DataForgeOrigin, SourceLocation};

use crate::{
    ArchiveModels, Attachment, EntityNode, GearPlacementSource, LandingGear, PortNode, PortSource,
};
use files::RigPath;

#[path = "entity_rig_files.rs"]
mod files;
pub(crate) use files::{ArchiveRigInputs, RigEntry, read_rig_entry};
pub use files::{ModelRig, RigDefinition, RigFile, RigFileRead, RigInput, RigInputs};

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
    port_selectors: Arc<[Option<SourceLocation>]>,
    gear_source: Option<GearPlacementSource>,
}

impl EntityRig {
    /// Returns one observed port-name source per segment of the owning route.
    ///
    /// A missing source means the segment's original name has no retained coordinate.
    #[must_use]
    pub fn port_selectors(&self) -> &[Option<SourceLocation>] {
        &self.port_selectors
    }

    /// Returns the observed declaration of a landing gear placement.
    #[must_use]
    pub const fn gear_source(&self) -> Option<&GearPlacementSource> {
        self.gear_source.as_ref()
    }
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

/// Discovers original Entity rig inputs with caller-controlled admission and reads.
///
/// # Errors
/// Returns the caller's terminal input or checkpoint error.
pub fn discover_entity_rigs_using<I: RigInputs>(
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
    catalogue.append(root, "", Placement::root(), inputs)?;
    crate::tree_walk::try_visit(root.ports(), |path, route| {
        inputs.check()?;
        if let Some(port) = route.last()
            && let Attachment::Entity(child) = port.attachment()
        {
            catalogue.append(child, path, Placement::attachment(path, route), inputs)?;
        }
        Ok(())
    })?;
    Ok(catalogue.rigs)
}

/// Discovers definition, skeleton and parameters for a model in its owning Archive.
///
/// The model path selects an exact physical candidate. A CDF's Model File uses the
/// Archive provider's authored path forms.
///
/// # Errors
/// Returns the caller's terminal input or checkpoint error.
pub fn discover_model_rig_using<I: RigInputs>(
    models: &ArchiveModels<'_>,
    path: &str,
    inputs: &mut I,
) -> Result<ModelRig, I::Error> {
    files::RigFiles::default().model(models, RigPath::Exact(path), inputs)
}

struct Catalogue<'model, 'archive> {
    models: &'model ArchiveModels<'archive>,
    owner_source: Option<&'model Arc<DataForgeOrigin>>,
    rigs: Vec<EntityRig>,
    files: files::RigFiles,
}

struct Placement {
    value: RigPlacement,
    port_selectors: Arc<[Option<SourceLocation>]>,
    gear_source: Option<GearPlacementSource>,
}

impl Placement {
    fn root() -> Self {
        Self {
            value: RigPlacement::Root,
            port_selectors: Arc::from([]),
            gear_source: None,
        }
    }

    fn attachment(path: &str, route: &[&PortNode]) -> Self {
        Self {
            value: RigPlacement::Attachment {
                port_path: path.to_owned(),
            },
            port_selectors: route.iter().map(|port| port_selector(port)).collect(),
            gear_source: None,
        }
    }
}

fn port_selector(port: &PortNode) -> Option<SourceLocation> {
    if port.source() != PortSource::PortDefault || port.name().is_empty() {
        return None;
    }
    let position = port.definition()?.position();
    Some(SourceLocation::DataForge {
        record_index: position.record_index(),
        field: format!(
            "Components[{}].Ports[{}].Name",
            position.component_ordinal(),
            position.port_ordinal()
        ),
    })
}

impl Catalogue<'_, '_> {
    fn append<I: RigInputs>(
        &mut self,
        node: &EntityNode,
        path: &str,
        placement: Placement,
        inputs: &mut I,
    ) -> Result<(), I::Error> {
        let geometry = if matches!(placement.value, RigPlacement::Root) {
            node.geometry()
        } else {
            node.own_skin
                .as_ref()
                .map(|skin| &skin.geometry)
                .or_else(|| node.geometry())
        };
        let port_selectors = Arc::clone(&placement.port_selectors);
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
            self.append_gear(node, path, &port_selectors, gear, inputs)?;
        }
        Ok(())
    }

    fn append_gear<I: RigInputs>(
        &mut self,
        node: &EntityNode,
        path: &str,
        port_selectors: &Arc<[Option<SourceLocation>]>,
        gear: &LandingGear,
        inputs: &mut I,
    ) -> Result<(), I::Error> {
        let geometry = Some(gear.geometry()).filter(|path| !path.is_empty());
        let source = GearPlacementSource::of(node, gear, false);
        self.append_rig(
            node,
            Placement {
                value: RigPlacement::LandingGear {
                    owner_path: path.to_owned(),
                    ordinal: gear.source.ordinal,
                    bone: gear.source.authored_bone.then(|| gear.bone().to_owned()),
                },
                port_selectors: Arc::clone(port_selectors),
                gear_source: Some(source),
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
        placement: Placement,
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
        placement: Placement,
        geometry: Option<&str>,
        geometry_selectors: Vec<SourceLocation>,
        inputs: &mut I,
    ) -> Result<EntityRig, I::Error> {
        let rig = geometry
            .map(|path| {
                self.files
                    .model(self.models, RigPath::Authored(path), inputs)
            })
            .transpose()?;
        let ModelRig {
            definition,
            skeleton,
            parameters,
        } = rig.unwrap_or_default();
        Ok(EntityRig {
            placement: placement.value,
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
            port_selectors: placement.port_selectors,
            gear_source: placement.gear_source,
        })
    }
}
