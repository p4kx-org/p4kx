// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::model::{
    PhysicsAxial, PhysicsBox, PhysicsGeometry, PhysicsSphere, PhysicsTrimesh, physics_marker_range,
    physics_shape_range,
};

use super::evidence::{PhysicsOrigins, bind, merge};
use super::payload::{GeometryFrame, PhysicsShape, PhysicsSlotBlob};
use crate::origin_prepared::PreparedRecordOrigins;
use crate::{MediaError, MediaWriter};

const GEOMETRY: &str = "blob/geometry";

pub(super) fn project(
    geometry: &PhysicsGeometry<'_>,
    original: &PhysicsOrigins<'_, '_>,
    prepared: &mut PreparedRecordOrigins,
    writer: &MediaWriter<'_>,
) -> Result<PhysicsSlotBlob, MediaError> {
    let version = original.use_bytes(physics_marker_range(), "physicsBlobMarker", writer)?;
    let shape = original.use_bytes(physics_shape_range(), "physicsShapeSelector", writer)?;
    let layout = merge(writer, [&version, &shape])?;
    for field in ["blob", "blob/disposition", "blob/frame", GEOMETRY] {
        bind(prepared, field, layout.clone(), writer)?;
    }
    bind(prepared, "blob/geometry/shape", shape, writer)?;
    let geometry = match geometry {
        PhysicsGeometry::Box(shape) => project_box(shape, original, prepared, writer)?,
        PhysicsGeometry::Sphere(shape) => sphere(shape, original, prepared, writer)?,
        PhysicsGeometry::Cylinder(shape) => axial(shape, false, original, prepared, writer)?,
        PhysicsGeometry::Capsule(shape) => axial(shape, true, original, prepared, writer)?,
        PhysicsGeometry::Trimesh(mesh) => trimesh(mesh, original, prepared, writer)?,
        _ => return Err(MediaError::Contract("unhandled collision shape".into())),
    };
    Ok(PhysicsSlotBlob::Geometry {
        frame: GeometryFrame::SourceNodeZUpMetres,
        geometry: Box::new(geometry),
    })
}

fn project_box(
    shape: &PhysicsBox,
    original: &PhysicsOrigins<'_, '_>,
    prepared: &mut PreparedRecordOrigins,
    writer: &MediaWriter<'_>,
) -> Result<PhysicsShape, MediaError> {
    let axes = shape.axes[0][0].range().start..shape.axes[2][2].range().end;
    bind(
        prepared,
        "blob/geometry/axes",
        original.use_bytes(axes, "physicsBoxBasis", writer)?,
        writer,
    )?;
    for (index, axis) in shape.axes.into_iter().enumerate() {
        original.vector(
            &format!("blob/geometry/axes/{index}"),
            axis,
            prepared,
            writer,
        )?;
    }
    bind(
        prepared,
        "blob/geometry/orientation_word",
        original.use_bytes(shape.orientation_range(), "physicsBoxOrientation", writer)?,
        writer,
    )?;
    original.vector("blob/geometry/centre", shape.centre, prepared, writer)?;
    original.vector("blob/geometry/half_size", shape.half_size, prepared, writer)?;
    Ok(PhysicsShape::Box {
        axes: shape.axes.map(|axis| axis.map(Into::into)),
        orientation_word: shape.oriented,
        centre: shape.centre.map(Into::into),
        half_size: shape.half_size.map(Into::into),
    })
}

fn sphere(
    shape: &PhysicsSphere,
    original: &PhysicsOrigins<'_, '_>,
    prepared: &mut PreparedRecordOrigins,
    writer: &MediaWriter<'_>,
) -> Result<PhysicsShape, MediaError> {
    original.vector("blob/geometry/centre", shape.centre, prepared, writer)?;
    original.scalar("blob/geometry/radius", shape.radius, prepared, writer)?;
    Ok(PhysicsShape::Sphere {
        centre: shape.centre.map(Into::into),
        radius: shape.radius.into(),
    })
}

fn axial(
    shape: &PhysicsAxial,
    capsule: bool,
    original: &PhysicsOrigins<'_, '_>,
    prepared: &mut PreparedRecordOrigins,
    writer: &MediaWriter<'_>,
) -> Result<PhysicsShape, MediaError> {
    original.vector("blob/geometry/centre", shape.centre, prepared, writer)?;
    original.vector("blob/geometry/axis", shape.axis, prepared, writer)?;
    original.scalar("blob/geometry/radius", shape.radius, prepared, writer)?;
    original.scalar(
        "blob/geometry/half_height",
        shape.half_height,
        prepared,
        writer,
    )?;
    let centre = shape.centre.map(Into::into);
    let axis = shape.axis.map(Into::into);
    let radius = shape.radius.into();
    let half_height = shape.half_height.into();
    Ok(if capsule {
        PhysicsShape::Capsule {
            centre,
            axis,
            radius,
            half_height,
        }
    } else {
        PhysicsShape::Cylinder {
            centre,
            axis,
            radius,
            half_height,
        }
    })
}

fn trimesh(
    mesh: &PhysicsTrimesh<'_>,
    original: &PhysicsOrigins<'_, '_>,
    prepared: &mut PreparedRecordOrigins,
    writer: &MediaWriter<'_>,
) -> Result<PhysicsShape, MediaError> {
    for (name, range) in [
        ("vertex_count", mesh.vertex_count_range()),
        ("triangle_count", mesh.triangle_count_range()),
    ] {
        bind(
            prepared,
            &format!("blob/geometry/{name}"),
            original.use_bytes(range, name, writer)?,
            writer,
        )?;
    }
    let layout = original.use_bytes(
        physics_shape_range().end..mesh.material_range().start,
        "physicsTrimeshLayout",
        writer,
    )?;
    for name in ["vertex_range", "index_range", "material_range"] {
        original.bind_range(
            &format!("blob/geometry/{name}"),
            layout.clone(),
            layout.clone(),
            prepared,
            writer,
        )?;
    }
    Ok(PhysicsShape::Trimesh {
        vertex_count: mesh.vertices().len(),
        triangle_count: mesh.triangles().len(),
        vertex_range: original.range(mesh.vertex_range()),
        index_range: original.range(mesh.index_range()),
        material_range: original.range(mesh.material_range()),
    })
}
