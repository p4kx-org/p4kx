// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_foundation::{ByteReader, ParseError};

use crate::physics_trimesh::PhysicsTrimesh;

const GEOMETRY_VERSION: u32 = 2;
const DAMAGE_MARKER: u32 = 0xCAFE_BABE;
const SDF_MARKER: u32 = 0xDEAD_BEB5;
const GEOMETRY_KIND_OFFSET: usize = 68;

/// A version-qualified disposition for an original `StatObjPhysics` blob.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum PhysicsBlob<'source> {
    /// The original slot declares a payload that is unavailable.
    MissingPayload,
    /// The identified damage-mesh format is retained in the slot's original bytes.
    DamageMesh,
    /// The identified signed-distance field is retained in the slot's original bytes.
    SignedDistanceField,
    /// The original marker is outside the supported geometry format.
    UnknownMarker(u32),
    /// The original version-2 shape selector is unsupported.
    UnknownKind(u32),
    /// A recognized layout has truncated or invalid fields.
    Malformed(PhysicsBlobError),
    /// A checked version-2 geometry projection.
    Geometry(PhysicsGeometry<'source>),
}

/// A failure to decode a recognized collision geometry layout.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum PhysicsBlobError {
    /// An original field extends past the blob.
    #[error(transparent)]
    Read(#[from] ParseError),
    /// A field required a finite float.
    #[error("non-finite collision geometry word at blob offset {offset}")]
    NonFinite {
        /// Original word offset relative to this blob.
        offset: u64,
    },
    /// A declared count is invalid for the layout.
    #[error("invalid collision geometry count at blob offset {offset}")]
    Count {
        /// Original count offset relative to this blob.
        offset: u64,
    },
    /// An index names a vertex outside the original mesh.
    #[error("triangle index {index} at blob offset {offset} is outside {vertices} vertices")]
    Index {
        /// Offset of the original index word.
        offset: u64,
        /// The original index.
        index: u16,
        /// The declared vertex count.
        vertices: usize,
    },
}

/// An original finite scalar, positioned within a collision blob.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhysicsFloat {
    bits: u32,
    offset: u64,
}

impl PhysicsFloat {
    pub(crate) const fn from_validated_bits(bits: u32, offset: u64) -> Self {
        Self { bits, offset }
    }

    pub(crate) fn read(reader: &mut ByteReader<'_>) -> Result<Self, PhysicsBlobError> {
        let offset = reader.offset();
        let bits = reader.read_u32()?;
        if f32::from_bits(bits).is_finite() {
            Ok(Self { bits, offset })
        } else {
            Err(PhysicsBlobError::NonFinite { offset })
        }
    }

    /// Returns the exact source word.
    #[must_use]
    pub const fn bits(self) -> u32 {
        self.bits
    }

    /// Interprets the source word as a finite binary32 value.
    #[must_use]
    pub const fn value(self) -> f32 {
        f32::from_bits(self.bits)
    }

    /// Returns the original half-open byte range relative to this blob.
    #[must_use]
    pub const fn range(self) -> Range<u64> {
        self.offset..self.offset + size_of::<u32>() as u64
    }
}

pub(crate) fn read_vector(
    reader: &mut ByteReader<'_>,
) -> Result<[PhysicsFloat; 3], PhysicsBlobError> {
    Ok([
        PhysicsFloat::read(reader)?,
        PhysicsFloat::read(reader)?,
        PhysicsFloat::read(reader)?,
    ])
}

/// A version-2 collision shape in the source node's Z-up coordinate frame, in metres.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum PhysicsGeometry<'source> {
    /// An oriented box.
    Box(Box<PhysicsBox>),
    /// An indexed triangle mesh with original vertex, index and material-ID ranges.
    Trimesh(PhysicsTrimesh<'source>),
    /// A sphere.
    Sphere(PhysicsSphere),
    /// A cylinder.
    Cylinder(PhysicsAxial),
    /// A capsule.
    Capsule(PhysicsAxial),
}

/// Box source fields in node coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct PhysicsBox {
    /// Original basis rows.
    pub axes: [[PhysicsFloat; 3]; 3],
    /// Original orientation word.
    pub oriented: u32,
    /// Original centre position.
    pub centre: [PhysicsFloat; 3],
    /// Original half extents along the box axes.
    pub half_size: [PhysicsFloat; 3],
}

impl PhysicsBox {
    /// Returns the original orientation word's byte range within the blob.
    #[must_use]
    pub const fn orientation_range(self) -> Range<u64> {
        let start = self.axes[2][2].range().end;
        start..start + size_of::<u32>() as u64
    }
}

/// Sphere source fields in node coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct PhysicsSphere {
    /// Original centre position.
    pub centre: [PhysicsFloat; 3],
    /// Original radius.
    pub radius: PhysicsFloat,
}

/// Source fields common to the cylinder and capsule layouts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct PhysicsAxial {
    /// Original centre position.
    pub centre: [PhysicsFloat; 3],
    /// Original direction of the axis.
    pub axis: [PhysicsFloat; 3],
    /// Original radius.
    pub radius: PhysicsFloat,
    /// Half axis length for a cylinder or half the centre-to-centre segment of a capsule.
    pub half_height: PhysicsFloat,
}

/// Returns the original blob marker word's byte range.
#[must_use]
pub const fn physics_marker_range() -> Range<u64> {
    0..size_of::<u32>() as u64
}

/// Returns the shape selector's byte range within version-2 geometry blobs.
#[must_use]
pub const fn physics_shape_range() -> Range<u64> {
    GEOMETRY_KIND_OFFSET as u64..(GEOMETRY_KIND_OFFSET + size_of::<u32>()) as u64
}

pub(crate) fn read(bytes: &[u8]) -> PhysicsBlob<'_> {
    classify(bytes).unwrap_or_else(PhysicsBlob::Malformed)
}

fn classify(bytes: &[u8]) -> Result<PhysicsBlob<'_>, PhysicsBlobError> {
    let mut reader = ByteReader::new(bytes);
    match reader.read_u32()? {
        DAMAGE_MARKER => return Ok(PhysicsBlob::DamageMesh),
        SDF_MARKER => return Ok(PhysicsBlob::SignedDistanceField),
        GEOMETRY_VERSION => (),
        marker => return Ok(PhysicsBlob::UnknownMarker(marker)),
    }
    reader.skip(GEOMETRY_KIND_OFFSET - size_of::<u32>())?;
    let geometry = match reader.read_u32()? {
        0 => PhysicsGeometry::Box(Box::new(PhysicsBox {
            axes: [
                read_vector(&mut reader)?,
                read_vector(&mut reader)?,
                read_vector(&mut reader)?,
            ],
            oriented: reader.read_u32()?,
            centre: read_vector(&mut reader)?,
            half_size: read_vector(&mut reader)?,
        })),
        1 => PhysicsGeometry::Trimesh(PhysicsTrimesh::read(&mut reader)?),
        4 => PhysicsGeometry::Sphere(PhysicsSphere {
            centre: read_vector(&mut reader)?,
            radius: PhysicsFloat::read(&mut reader)?,
        }),
        5 => PhysicsGeometry::Cylinder(read_axial(&mut reader)?),
        6 => PhysicsGeometry::Capsule(read_axial(&mut reader)?),
        kind => return Ok(PhysicsBlob::UnknownKind(kind)),
    };
    Ok(PhysicsBlob::Geometry(geometry))
}

fn read_axial(reader: &mut ByteReader<'_>) -> Result<PhysicsAxial, PhysicsBlobError> {
    Ok(PhysicsAxial {
        centre: read_vector(reader)?,
        axis: read_vector(reader)?,
        radius: PhysicsFloat::read(reader)?,
        half_height: PhysicsFloat::read(reader)?,
    })
}
