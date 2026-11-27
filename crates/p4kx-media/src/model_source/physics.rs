// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

#[path = "physics/evidence.rs"]
mod evidence;
#[path = "physics/frame_hierarchy.rs"]
mod frame_hierarchy;
#[path = "physics/frame_projection.rs"]
mod frame_projection;
#[path = "physics/geometry.rs"]
mod geometry;
#[path = "physics/payload.rs"]
mod payload;
#[path = "physics/schema.rs"]
mod schema;
#[path = "physics/slot.rs"]
mod slot;
#[path = "physics/table.rs"]
mod table;

use p4kx_extraction::SourceUseRole;
use p4kx_extraction::chunks::{ChunkDirectory, ivo};
use p4kx_extraction::model::StatObjPhysics;

use super::origins::ModelOrigins;
use crate::required_reader::RequiredReader;
use crate::{MediaError, MediaWriter};

use evidence::PhysicsOrigins;
use payload::PhysicsTableStatus;

use frame_projection::FrameProjection;
pub(super) use frame_projection::NodeFrames;
pub(super) use payload::{FRAME, SLOT, TABLE};
pub(super) use schema::bind;

pub(super) fn emit(
    file: &ChunkDirectory,
    index: usize,
    reader: &mut RequiredReader<'_>,
    frames: Option<&FrameProjection<'_>>,
    origins: &ModelOrigins<'_>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let missing = || MediaError::Contract("physics chunk lost its original table slot".into());
    let chunk = file.chunks().get(index).ok_or_else(missing)?;
    if chunk.chunk_type() != ivo::STAT_OBJ_PHYSICS {
        return Ok(());
    }
    let row = file.table_row(index).ok_or_else(missing)?;
    let extent =
        super::origins::chunk_extent(file.table_range(), row.offset, row.size, origins, writer)?;
    let original = PhysicsOrigins {
        model: origins,
        base: chunk.offset(),
        extent: &extent.end,
    };
    let version = origins.payload(
        row.version,
        "modelChunkVersion",
        SourceUseRole::Payload,
        writer,
    )?;
    if let Err(cause) = StatObjPhysics::require_layout(chunk, index) {
        return table::diagnose(
            index,
            PhysicsTableStatus::UnsupportedVersion,
            version,
            &cause.to_string(),
            &original,
            writer,
        );
    }
    let bytes = reader.read_chunk(chunk.offset()..chunk.offset() + chunk.size())?;
    match StatObjPhysics::read(&bytes) {
        Ok(physics) => {
            table::emit(index, &physics, &original, writer)?;
            if let Some(frames) = frames {
                for slot in physics.slots() {
                    writer.checkpoint()?;
                    frames.slot(index, &slot, &original, writer)?;
                }
            }
            Ok(())
        }
        Err(cause) => {
            let failure = original.use_bytes(0..chunk.size(), "physicsInvalidTable", writer)?;
            table::diagnose(
                index,
                PhysicsTableStatus::Malformed,
                failure,
                &cause.to_string(),
                &original,
                writer,
            )
        }
    }
}
