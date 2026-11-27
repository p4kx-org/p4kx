// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::model::StatObjPhysics;

use super::evidence::{PhysicsOrigins, bind, merge};
use super::payload::{PhysicsPayload, PhysicsTable, PhysicsTableStatus, TABLE};
use super::slot;
use crate::origin_contributions::OriginContributions;
use crate::{MediaError, MediaIssue, MediaWriter, Severity, Status, Subresource};

pub(super) fn emit(
    index: usize,
    physics: &StatObjPhysics<'_>,
    original: &PhysicsOrigins<'_, '_>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let mut prepared = original.model.prepared()?;
    let count = original.use_bytes(
        StatObjPhysics::slot_count_range(),
        "physicsSlotCount",
        writer,
    )?;
    for field in ["slot_count", "disposition"] {
        bind(&mut prepared, field, count.clone(), writer)?;
    }
    original.bind_range(
        "slot_table",
        original.extent.clone(),
        count.clone(),
        &mut prepared,
        writer,
    )?;
    let lengths = original.use_bytes(physics.table_range(), "physicsSlotTable", writer)?;
    let tail = physics.trailing_range();
    let tail_origin = merge(writer, [&count, &lengths, original.extent])?;
    if tail.is_some() {
        original.bind_range(
            "trailing",
            tail_origin,
            original.extent.clone(),
            &mut prepared,
            writer,
        )?;
    } else {
        bind(&mut prepared, "trailing", tail_origin, writer)?;
    }
    prepared.write(
        writer,
        &Subresource::new(TABLE, vec![index as u64]),
        Status::Complete,
        &PhysicsPayload::Table(PhysicsTable {
            disposition: PhysicsTableStatus::Read,
            slot_count: Some(physics.slots().len()),
            slot_table: Some(original.range(physics.table_range())),
            trailing: tail.map(|range| original.range(range)),
        }),
        &[],
    )?;
    for slot in physics.slots() {
        writer.checkpoint()?;
        slot::emit(
            index,
            &slot,
            physics.table_range().start,
            &count,
            original,
            writer,
        )?;
    }
    Ok(())
}

pub(super) fn diagnose(
    index: usize,
    disposition: PhysicsTableStatus,
    source: OriginContributions,
    detail: &str,
    original: &PhysicsOrigins<'_, '_>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let mut prepared = original.model.prepared()?;
    for field in ["disposition", "slot_count", "slot_table", "trailing"] {
        bind(&mut prepared, field, source.clone(), writer)?;
    }
    let malformed = matches!(disposition, PhysicsTableStatus::Malformed);
    let (code, severity, status) = if malformed {
        ("model.physics-table", Severity::Warning, Status::Partial)
    } else {
        ("model.physics-version", Severity::Info, Status::Complete)
    };
    super::super::origins::at(&mut prepared, "/issues", source.clone(), writer)?;
    super::super::origins::at(&mut prepared, "/status", source, writer)?;
    prepared.write(
        writer,
        &Subresource::new(TABLE, vec![index as u64]),
        status,
        &PhysicsPayload::Table(PhysicsTable {
            disposition,
            slot_count: None,
            slot_table: None,
            trailing: None,
        }),
        &[MediaIssue::new(code, "stat-obj-physics", severity, detail)],
    )
}
