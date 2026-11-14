// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::model::{
    PhysicsBlob, PhysicsGeometry, PhysicsTrimesh, StatObjPhysicsSlot, physics_marker_range,
    physics_shape_range,
};

use super::evidence::{PhysicsOrigins, bind, merge};
use super::geometry;
use super::payload::{
    PhysicsPayload, PhysicsSlot, PhysicsSlotBlob, PhysicsTriangle, PhysicsVertex, SLOT, TRIANGLE,
    VERTEX,
};
use crate::origin_contributions::OriginContributions;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::{MediaError, MediaIssue, MediaWriter, Severity, SourceRange, Status, Subresource};

pub(super) fn emit(
    chunk: usize,
    slot: &StatObjPhysicsSlot<'_>,
    table_start: u64,
    count: &OriginContributions,
    original: &PhysicsOrigins<'_, '_>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let local = slot.declared_range();
    let blob = PhysicsOrigins {
        model: original.model,
        base: original.global(local).start,
        extent: original.extent,
    };
    let mut prepared = original.model.prepared()?;
    let (retained, end) = slot_origin(slot, table_start, count, original, &mut prepared, writer)?;
    let classification = slot.blob();
    let diagnosis = SlotDiagnostic::from(&classification);
    let data = match &classification {
        PhysicsBlob::Geometry(geometry) => {
            geometry::project(geometry, &blob, &mut prepared, writer)?
        }
        _ => non_geometry(&classification, slot, &blob, end, &mut prepared, writer)?,
    };
    if !diagnosis.issues.is_empty() {
        let proof = if slot.bytes().is_some() {
            blob.use_bytes(
                0..u64::from(slot.declared_length()),
                "physicsBlobFailure",
                writer,
            )?
        } else {
            original.use_bytes(slot.length_range(), "physicsDeclaredLength", writer)?
        };
        super::super::origins::at(&mut prepared, "/status", proof.clone(), writer)?;
        super::super::origins::at(&mut prepared, "/issues", proof, writer)?;
    }
    let indices = [chunk as u64, slot.ordinal() as u64];
    prepared.write(
        writer,
        &Subresource::new(SLOT, indices.to_vec()),
        diagnosis.status,
        &PhysicsPayload::Slot(PhysicsSlot {
            packed_key: slot.key(),
            part: slot.part(),
            node: slot.node(),
            declared_length: slot.declared_length(),
            declared_range: original.range(slot.declared_range()),
            retained_range: retained,
            blob: data,
        }),
        &diagnosis.issues,
    )?;
    if let PhysicsBlob::Geometry(PhysicsGeometry::Trimesh(mesh)) = classification {
        mesh_rows(&mesh, indices, &blob, writer)?;
    }
    Ok(())
}

fn slot_origin(
    slot: &StatObjPhysicsSlot<'_>,
    table_start: u64,
    count: &OriginContributions,
    original: &PhysicsOrigins<'_, '_>,
    prepared: &mut PreparedRecordOrigins,
    writer: &MediaWriter<'_>,
) -> Result<(Option<SourceRange>, OriginContributions), MediaError> {
    let key = original.use_bytes(slot.key_range(), "physicsPackedKey", writer)?;
    for field in ["packed_key", "part", "node"] {
        bind(prepared, field, key.clone(), writer)?;
    }
    let size = original.use_bytes(slot.length_range(), "physicsDeclaredLength", writer)?;
    bind(prepared, "declared_length", size.clone(), writer)?;
    let preceding = original.use_bytes(
        table_start..slot.length_range().start,
        "physicsPrecedingSlots",
        writer,
    )?;
    let start = merge(writer, [count, &preceding])?;
    let end = merge(writer, [&start, &size])?;
    original.bind_range(
        "declared_range",
        start.clone(),
        end.clone(),
        prepared,
        writer,
    )?;
    let retained = slot.bytes().map(|_| original.range(slot.declared_range()));
    if retained.is_some() {
        original.bind_range("retained_range", start, end.clone(), prepared, writer)?;
    } else {
        bind(
            prepared,
            "retained_range",
            merge(writer, [&end, original.extent])?,
            writer,
        )?;
    }
    Ok((retained, end))
}

fn non_geometry(
    classification: &PhysicsBlob<'_>,
    slot: &StatObjPhysicsSlot<'_>,
    original: &PhysicsOrigins<'_, '_>,
    extent: OriginContributions,
    prepared: &mut PreparedRecordOrigins,
    writer: &MediaWriter<'_>,
) -> Result<PhysicsSlotBlob, MediaError> {
    let (data, evidence) = match classification {
        PhysicsBlob::MissingPayload => (PhysicsSlotBlob::MissingPayload, extent),
        PhysicsBlob::DamageMesh => (PhysicsSlotBlob::DamageMesh, marker(original, writer)?),
        PhysicsBlob::SignedDistanceField => (
            PhysicsSlotBlob::SignedDistanceField,
            marker(original, writer)?,
        ),
        PhysicsBlob::UnknownMarker(marker_value) => (
            PhysicsSlotBlob::UnknownMarker {
                marker: *marker_value,
            },
            marker(original, writer)?,
        ),
        PhysicsBlob::UnknownKind(selector) => {
            let selector_source =
                original.use_bytes(physics_shape_range(), "physicsShapeSelector", writer)?;
            bind(prepared, "blob/selector", selector_source.clone(), writer)?;
            let version = marker(original, writer)?;
            (
                PhysicsSlotBlob::UnknownKind {
                    selector: *selector,
                },
                merge(writer, [&version, &selector_source])?,
            )
        }
        PhysicsBlob::Malformed(_) => (
            PhysicsSlotBlob::Malformed,
            original.use_bytes(
                0..u64::from(slot.declared_length()),
                "physicsMalformedBlob",
                writer,
            )?,
        ),
        _ => return Err(MediaError::Contract("unhandled opaque physics blob".into())),
    };
    bind(prepared, "blob", evidence, writer)?;
    Ok(data)
}

fn marker(
    original: &PhysicsOrigins<'_, '_>,
    writer: &MediaWriter<'_>,
) -> Result<OriginContributions, MediaError> {
    original.use_bytes(physics_marker_range(), "physicsBlobMarker", writer)
}

struct SlotDiagnostic {
    status: Status,
    issues: Vec<MediaIssue>,
}

impl From<&PhysicsBlob<'_>> for SlotDiagnostic {
    fn from(blob: &PhysicsBlob<'_>) -> Self {
        let issue = match blob {
            PhysicsBlob::MissingPayload => Some(MediaIssue::new(
                "model.physics-missing-blob",
                "stat-obj-physics",
                Severity::Warning,
                "declared collision payload extends beyond the retained chunk",
            )),
            PhysicsBlob::Malformed(cause) => Some(MediaIssue::new(
                "model.physics-malformed-blob",
                "stat-obj-physics",
                Severity::Warning,
                cause.to_string(),
            )),
            _ => None,
        };
        Self {
            status: if issue.is_some() {
                Status::Partial
            } else {
                Status::Complete
            },
            issues: issue.into_iter().collect(),
        }
    }
}

fn mesh_rows(
    mesh: &PhysicsTrimesh<'_>,
    slot: [u64; 2],
    original: &PhysicsOrigins<'_, '_>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    for (ordinal, position) in mesh.vertices().enumerate() {
        let mut prepared = original.model.prepared()?;
        original.vector("position", position, &mut prepared, writer)?;
        prepared.write(
            writer,
            &Subresource::new(VERTEX, vec![slot[0], slot[1], ordinal as u64]),
            Status::Complete,
            &PhysicsPayload::Vertex(PhysicsVertex {
                position: position.map(Into::into),
            }),
            &[],
        )?;
    }
    for (ordinal, (indices, &material_id)) in mesh.triangles().zip(mesh.material_ids()).enumerate()
    {
        let mut prepared = original.model.prepared()?;
        let at = mesh.index_range().start + ordinal as u64 * 3 * size_of::<u16>() as u64;
        let index_bytes = |ordinal| at + ordinal * size_of::<u16>() as u64;
        bind(
            &mut prepared,
            "indices",
            original.use_bytes(at..index_bytes(3), "physicsTriangleIndices", writer)?,
            writer,
        )?;
        for index in 0..3 {
            bind(
                &mut prepared,
                &format!("indices/{index}"),
                original.use_bytes(
                    index_bytes(index)..index_bytes(index + 1),
                    "physicsVertexIndex",
                    writer,
                )?,
                writer,
            )?;
        }
        let at = mesh.material_range().start + ordinal as u64;
        bind(
            &mut prepared,
            "material_id",
            original.use_bytes(at..at + 1, "physicsTriangleMaterial", writer)?,
            writer,
        )?;
        prepared.write(
            writer,
            &Subresource::new(TRIANGLE, vec![slot[0], slot[1], ordinal as u64]),
            Status::Complete,
            &PhysicsPayload::Triangle(PhysicsTriangle {
                indices,
                material_id,
            }),
            &[],
        )?;
    }
    Ok(())
}
