// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::{FieldOrigin, PhysicalSourceRef, SourceInventory, SourcedValue};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::package_receipt::{self, PackageFile};
use crate::{PackageResult, PackageWriteContext};

pub(crate) const FILE: &str = "collision-helpers.json";
const VERSION: u8 = 1;

pub(crate) type Affine = [[f32; 4]; 3];

/// Reference to a versioned collision-helper companion in a package.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CollisionHelperReference {
    /// The package-relative file carrying the source facts and helper placements.
    #[schemars(extend("const" = FILE))]
    pub file: String,
    /// The collision-helper document's schema version.
    #[schemars(extend("const" = VERSION))]
    pub schema_version: u8,
}

impl CollisionHelperReference {
    pub(crate) fn new() -> Self {
        Self {
            file: FILE.to_owned(),
            schema_version: VERSION,
        }
    }

    pub(crate) fn is_valid(&self) -> bool {
        self.file == FILE && self.schema_version == VERSION
    }
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct CollisionHelpers {
    #[schemars(extend("const" = VERSION))]
    pub(crate) schema_version: u8,
    pub(crate) models: Vec<ModelCollision>,
    pub(crate) instances: Vec<ModelInstance>,
    #[serde(rename = "sourceInventory")]
    pub(crate) inventory: SourceInventory,
}

/// Returns the JSON Schema of the optional Blender collision companion.
#[must_use]
pub fn collision_helpers_schema() -> serde_json::Value {
    schemars::schema_for!(CollisionHelpers).to_value()
}

impl CollisionHelpers {
    pub(crate) const fn new(
        models: Vec<ModelCollision>,
        instances: Vec<ModelInstance>,
        inventory: SourceInventory,
    ) -> Self {
        Self {
            schema_version: VERSION,
            models,
            instances,
            inventory,
        }
    }

    pub(crate) fn write(
        &self,
        staging: &Path,
        context: &PackageWriteContext<'_>,
    ) -> PackageResult<PackageFile> {
        let digest = package_receipt::write_json(&staging.join(FILE), self, context, false)?;
        Ok(PackageFile::new(FILE.to_owned(), &digest))
    }
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ModelCollision {
    pub(crate) source: PhysicalSourceRef,
    pub(crate) slots: Vec<CollisionSlot>,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct CollisionSlot {
    pub(crate) chunk: usize,
    pub(crate) slot: usize,
    pub(crate) packed_key: SourcedValue<u32>,
    pub(crate) frame: SourcedValue<Affine>,
    pub(crate) geometry: SourcedValue<TriangleMesh>,
    pub(crate) disposition: CollisionDisposition,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CollisionDisposition {
    Ready,
    WholeObject,
    FrameUnavailable,
    EmptyMesh,
    NonMeshShape,
    UnsupportedBlob,
    MalformedBlob,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct TriangleMesh {
    pub(crate) positions_z_up_metres: Vec<[f32; 3]>,
    pub(crate) indices: Vec<[u16; 3]>,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ModelInstance {
    pub(crate) node: usize,
    pub(crate) model: usize,
    pub(crate) cdf: Option<CdfPlacement>,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct CdfPlacement {
    pub(crate) definition: PhysicalSourceRef,
    pub(crate) attachment_bone: SourcedValue<String>,
    pub(crate) declared_skeleton_file: SourcedValue<String>,
    pub(crate) selected_rig: Option<SelectedRig>,
    pub(crate) owner_from_attachment: SourcedValue<Affine>,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct SelectedRig {
    pub(crate) source: PhysicalSourceRef,
    pub(crate) chunk: usize,
    pub(crate) bone: usize,
    pub(crate) lod: Option<u32>,
    pub(crate) origin: FieldOrigin,
}
