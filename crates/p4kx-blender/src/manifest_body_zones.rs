// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The body and worn-part addresses in the Character's rigged GLB.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[non_exhaustive]
pub struct BodyZoneMap {
    /// SHA-256 of the exact GLB whose mesh and primitive indices this map addresses.
    #[schemars(regex(pattern = "^[0-9a-f]{64}$"))]
    pub glb_sha256: String,
    /// The geometry addresses and source evidence.
    #[serde(flatten)]
    pub data: BodyZoneData,
}

/// The geometry and provenance of a Character body-zone map.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[non_exhaustive]
pub struct BodyZoneData {
    /// The body's original zone partition and its visible primitives.
    pub body: BodyZoneGeometry,
    /// Geometry and clothing coverage for the emitted worn parts.
    pub parts: Vec<PartBodyZones>,
    /// The original input Entries and their uses in the map.
    #[schemars(with = "p4kx_extraction::SourceInventory")]
    pub source_inventory: Value,
    /// Immediate provenance at source-derived map fields, in map-relative JSON Pointers.
    #[schemars(with = "p4kx_extraction::FieldOrigins")]
    pub field_origins: Value,
}

/// Whether the body's geometry could be mapped to the variant's original zones.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "state", rename_all = "snake_case")]
#[non_exhaustive]
pub enum BodyZoneGeometry {
    /// The original zone layout and every emitted body primitive were identified.
    Verified {
        /// Exact emitted primitive locations, in writer order.
        primitives: Vec<BodyZonePrimitive>,
        /// Zones already omitted by clothing when building this Character.
        omitted: Vec<String>,
    },
    /// The complete body-zone partition could not be verified.
    Unavailable {
        /// The layout or output-address failure.
        reason: String,
    },
}

/// One named clothing zone surface addressed in the rigged GLB.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[non_exhaustive]
pub struct BodyZonePrimitive {
    /// The canonical zone name.
    pub zone: String,
    /// The zero-based GLB mesh index.
    pub mesh: usize,
    /// The zero-based primitive in that mesh.
    pub primitive: usize,
    /// The actual geometry level used by this mesh.
    pub model_lod: u32,
}

/// A worn part and its own body coverage on the shared rig.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[non_exhaustive]
pub struct PartBodyZones {
    /// The zero-based index within the owning manifest rig's parts.
    pub part: usize,
    /// The zero-based GLB mesh indices produced by this part.
    pub meshes: Vec<usize>,
    /// Emitted primitives with a unique zone match in the worn part's Record.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub primitives: Vec<BodyZonePrimitive>,
    /// Authored clothing coverage when a physical item Record supplied it.
    pub coverage: Option<ClothingBodyCoverage>,
}

/// The retained body-layer coverage of an original worn item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[non_exhaustive]
pub struct ClothingBodyCoverage {
    /// The item's original DataForge Record GUID.
    pub guid: String,
    /// The original physical Record table index when available.
    pub record_index: Option<u32>,
    /// The item's original DataForge Record path.
    pub record_path: String,
    /// Whether the admitted authored chunk list is complete.
    pub chunks_complete: bool,
    /// Body zones unconditionally covered by this item.
    pub covers: Vec<String>,
    /// Body zones with unevaluated conditions and no unconditional cover from this item.
    pub conditional: Vec<String>,
}
