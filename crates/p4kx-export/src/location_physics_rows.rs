// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceLocation, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use crate::catalog_read::authored_xml_tree::AuthoredXmlElement;
use crate::catalog_read::original::PhysicalEntryRef;
use crate::schema_fields::every_field_required;

/// One physically selected `ObjectContainer` with authored location physics.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct LocationPhysicsGrid {
    /// The physical container joined by the exact placement row's container reference.
    pub container: SourcedValue<PhysicalEntryRef>,
    /// The exact original descriptor member.
    pub descriptor: SourcedValue<PhysicalEntryRef>,
    /// The authored descriptor-wide grid radius.
    pub grid_radius: SourcedValue<f64>,
    /// Ordered physics declarations, with the physical XML coordinates of their owners.
    pub declarations: Vec<LocationPhysicsDeclaration>,
    /// Source elements under the declared physics roots, including unknown members.
    pub source_elements: Vec<AuthoredXmlElement>,
}

/// One typed declaration at an original position in an `ObjectContainer` descriptor.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct LocationPhysicsDeclaration {
    /// The actual source element's coordinate.
    pub position: SourcedValue<SourceLocation>,
    /// The original parent element's coordinate.
    pub owner: SourcedValue<SourceLocation>,
    /// Position among all original sibling elements.
    pub sibling_ordinal: SourcedValue<u64>,
    /// Typed original settings of the declaration.
    pub fields: LocationPhysicsFields,
}

/// The settings specific to each location-physics source declaration.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub enum LocationPhysicsFields {
    /// One declared grid with its original controls.
    Grid {
        /// The controls authored on this grid occurrence.
        controls: Box<LocationPhysicsGridControls>,
    },
    /// One original part.
    Part {
        /// Original shape label.
        shape: SourcedValue<String>,
    },
    /// One original voxel resource declaration.
    Voxel {
        /// Original filename, including an authored empty string.
        filename: SourcedValue<String>,
        /// Directory resolution in the descriptor's own container and root P4K.
        target: SourcedValue<LocationVoxelTarget>,
    },
    /// Original child-part group. It can have no children.
    ChildParts,
}

/// The exact authored controls on one physical location grid.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct LocationPhysicsGridControls {
    /// Selection of inherited gravity.
    pub inherit_gravity: SourcedValue<bool>,
    /// Three authored gravity components in source order.
    pub gravity: SourcedValue<[f64; 3]>,
    /// Original grid type label.
    pub grid_type: SourcedValue<String>,
    /// Original cell size.
    pub cell_size: SourcedValue<f64>,
    /// Author's part-presence switch.
    pub has_part: SourcedValue<bool>,
    /// Original numeric portal mode.
    pub portal_exclusive_mode: SourcedValue<i64>,
}

/// Directory lookup result for an original voxel filename.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub enum LocationVoxelTarget {
    /// The original filename is repeated, empty or cannot form a supported path.
    Invalid,
    /// The physical directories contain no matching path.
    Missing,
    /// The complete root directory could not be established.
    Unavailable,
    /// The winning directory tier supplies one exact physical target.
    Matched {
        /// The exact original target Entry.
        entry: PhysicalEntryRef,
    },
    /// The winning directory tier supplies several physical targets.
    Ambiguous {
        /// Original targets in physical order.
        candidates: Vec<PhysicalEntryRef>,
    },
}
