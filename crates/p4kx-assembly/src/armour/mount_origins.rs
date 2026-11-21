// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};

use p4kx_extraction::{
    DataForgeOrigin, FieldOrigin, FieldOrigins, FieldOriginsError, FieldPath, FieldPathError,
    InputScope, OriginArtifactBinding, OriginOutputAddress, SourceEvidenceError, SourceInput,
    SourceInventory, SourceUse, SourceUseRole, SourceUses, write_origin_companion,
};
use p4kx_gltf::{RiggedGlbEmission, RiggedJsonAddress};
use thiserror::Error;

use super::{Armour, MountModelSource, WornMountSource};
use crate::PortPlacementField;

/// An origin projection could not represent the observed GLB fields.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum WornMountOriginError {
    /// The source registry rejected the evidence.
    #[error(transparent)]
    Evidence(#[from] SourceEvidenceError),
    /// An emitted selector could not be represented as a field path.
    #[error(transparent)]
    FieldPath(#[from] FieldPathError),
}

/// Provenance for the fields written for rigid worn mounts.
#[derive(Debug)]
pub struct WornMountOrigins {
    inventory: SourceInventory,
    fields: Vec<(FieldPath, FieldOrigin)>,
}

impl WornMountOrigins {
    /// Returns whether any emitted mount field was observed.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }

    /// Returns the source inventory and package-addressed fields.
    ///
    /// # Errors
    /// Returns an error if multiple observations claim the same GLB field.
    pub fn into_field_origins(self) -> Result<(SourceInventory, FieldOrigins), FieldOriginsError> {
        let mut fields = FieldOrigins::new();
        for (path, origin) in self.fields {
            fields.insert(path, origin)?;
        }
        Ok((self.inventory, fields))
    }

    /// Writes a GLB provenance companion in emission order.
    ///
    /// # Errors
    /// Returns an error if the companion cannot be serialized or written.
    pub fn write_companion(
        &self,
        writer: impl Write,
        binding: &OriginArtifactBinding,
    ) -> io::Result<()> {
        let fields: Vec<_> = self
            .fields
            .iter()
            .map(|(path, origin)| {
                (
                    OriginOutputAddress::GlbJsonField { path: path.clone() },
                    origin,
                )
            })
            .collect();
        write_origin_companion(
            writer,
            binding,
            &self.inventory,
            fields.iter().map(|(address, origin)| (address, *origin)),
        )
    }
}

impl Armour {
    /// Projects worn-mount provenance using the GLB emission observed for this armour.
    ///
    /// # Errors
    /// Returns an error if the source evidence or GLB field paths cannot be represented.
    pub fn worn_mount_origins(
        &self,
        emission: &RiggedGlbEmission,
    ) -> Result<WornMountOrigins, WornMountOriginError> {
        let uses = SourceUses::new();
        let mut fields = Vec::new();
        for &address in emission.json_addresses() {
            let RiggedJsonAddress::MountedNode {
                index,
                part,
                bone,
                child,
                trs,
            } = address
            else {
                continue;
            };
            let Some(source) = self.parts().get(part).and_then(|part| part.mount_source()) else {
                continue;
            };
            let origins = mount_origins(&uses, source)?;
            fields.push((node_path(bone, "children", Some(child))?, origins.parent));
            for (field, written) in ["translation", "rotation", "scale"].into_iter().zip(trs) {
                if written {
                    fields.push((node_path(index, field, None)?, origins.pose.clone()));
                }
            }
        }
        Ok(WornMountOrigins {
            inventory: uses.freeze()?,
            fields,
        })
    }
}

struct MountOrigins {
    parent: FieldOrigin,
    pose: FieldOrigin,
}

fn mount_origins(
    uses: &SourceUses,
    source: &WornMountSource,
) -> Result<MountOrigins, SourceEvidenceError> {
    let forge = uses.observe(forge_input(source.forge_origin()), InputScope::Payload)?;
    let mut parent = Vec::new();
    let mut pose = Vec::new();
    for (field, location) in source.port().locations() {
        let (target, role) = match field {
            PortPlacementField::Helper => (&mut parent, SourceUseRole::Selector),
            PortPlacementField::ItemOffsetHelper => (&mut pose, SourceUseRole::Selector),
            _ => (&mut pose, SourceUseRole::Conversion),
        };
        target.push(forge.at(location, role)?);
    }
    for model in [source.holder_bone(), source.skeleton_bone()] {
        if let Some(model) = model_input(uses, model)? {
            parent.extend(model.selector);
            pose.push(model.transform);
        }
    }
    if let Some(locator) = model_input(uses, source.item_locator())? {
        pose.extend(locator.selector);
        pose.push(locator.transform);
    }
    let parent_origin = uses.origin(&parent, false);
    pose.extend(parent);
    Ok(MountOrigins {
        parent: parent_origin,
        pose: uses.origin(&pose, false),
    })
}

const fn forge_input(forge: Option<&DataForgeOrigin>) -> SourceInput<'_> {
    match forge {
        Some(DataForgeOrigin::Archive(entry)) => SourceInput::Archive(entry),
        Some(DataForgeOrigin::File(file)) => SourceInput::File(file),
        _ => SourceInput::Unknown,
    }
}

fn node_path(node: usize, field: &str, child: Option<usize>) -> Result<FieldPath, FieldPathError> {
    let mut path = FieldPath::root();
    path.push_field("nodes")?;
    path.push_index(node)?;
    path.push_field(field)?;
    if let Some(child) = child {
        path.push_index(child)?;
    }
    Ok(path)
}

struct ModelUses {
    selector: Option<SourceUse>,
    transform: SourceUse,
}

fn model_input(
    uses: &SourceUses,
    model: Option<&MountModelSource>,
) -> Result<Option<ModelUses>, SourceEvidenceError> {
    let Some(model) = model else { return Ok(None) };
    let scope = uses.observe(SourceInput::Archive(model.origin()), InputScope::Payload)?;
    let selector = model
        .name()
        .map(|location| scope.at(location, SourceUseRole::Selector))
        .transpose()?;
    let transform = scope.at(model.transform(), SourceUseRole::Conversion)?;
    Ok(Some(ModelUses {
        selector,
        transform,
    }))
}
